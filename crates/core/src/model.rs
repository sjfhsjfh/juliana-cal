use std::collections::HashSet;
use std::path::Path;

use jiff::{
    SignedDuration, Timestamp, Zoned,
    civil::Date,
    tz::{TimeZone, TimeZoneDatabase},
};
use serde::{Deserialize, Deserializer};

use crate::error::Error;

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub timezone: String,
    pub calendar_name: String,
    pub calendar_uid: String,
    pub uid_domain: String,

    /// Stable `DTSTAMP` source. Keep this fixed unless the manifest changes.
    pub updated: toml::value::Datetime,

    #[serde(default)]
    pub default_alerts_before_minutes: Vec<u32>,

    /// Display interval for `ddl = <datetime>` entries. The interval ends at
    /// the deadline and starts this many minutes earlier.
    #[serde(default = "default_ddl_display_minutes")]
    pub ddl_display_minutes: u32,

    #[serde(default, rename = "event")]
    pub events: Vec<Event>,
}

#[derive(Debug, Deserialize)]
pub struct Event {
    pub id: String,
    pub summary: String,

    #[serde(flatten)]
    pub timing: Timing,

    pub description: Option<String>,
    pub location: Option<String>,
    pub url: Option<String>,
    pub status: Option<EventStatus>,

    pub alerts_before_minutes: Option<Vec<u32>>,

    #[serde(default)]
    pub sequence: u32,
}

#[derive(Debug)]
pub enum Timing {
    Interval {
        start: toml::value::Datetime,
        end: toml::value::Datetime,
    },
    Date {
        date: toml::value::Date,
    },
    DdlDateTime {
        ddl: toml::value::Datetime,
    },
    DdlDate {
        ddl: toml::value::Date,
    },
}

#[derive(Deserialize)]
struct TimingFields {
    start: Option<toml::value::Datetime>,
    end: Option<toml::value::Datetime>,
    date: Option<toml::value::Date>,
    ddl: Option<toml::value::Datetime>,
}

impl<'de> Deserialize<'de> for Timing {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let TimingFields {
            start,
            end,
            date,
            ddl,
        } = TimingFields::deserialize(deserializer)?;

        let has_interval = start.is_some() || end.is_some();
        let has_date = date.is_some();
        let has_ddl = ddl.is_some();
        let timing_count = usize::from(has_interval) + usize::from(has_date) + usize::from(has_ddl);

        if timing_count == 0 {
            return Err(<D::Error as serde::de::Error>::custom(
                "event timing must specify exactly one of `start`/`end`, `date`, or `ddl`",
            ));
        }
        if timing_count > 1 {
            return Err(<D::Error as serde::de::Error>::custom(
                "event timing `start`/`end`, `date`, and `ddl` are mutually exclusive",
            ));
        }

        if has_interval {
            let (Some(start), Some(end)) = (start, end) else {
                return Err(<D::Error as serde::de::Error>::custom(
                    "event timing `start` and `end` must be provided together",
                ));
            };
            return Ok(Timing::Interval { start, end });
        }

        if let Some(date) = date {
            return Ok(Timing::Date { date });
        }

        if let Some(ddl) = ddl {
            if ddl.time.is_none() {
                let Some(date) = ddl.date else {
                    return Err(<D::Error as serde::de::Error>::custom(
                        "event timing `ddl` must include a date",
                    ));
                };
                return Ok(Timing::DdlDate { ddl: date });
            }
            return Ok(Timing::DdlDateTime { ddl });
        }

        Err(<D::Error as serde::de::Error>::custom(
            "event timing is invalid",
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum EventStatus {
    Tentative,
    Confirmed,
    Cancelled,
}

impl EventStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tentative => "TENTATIVE",
            Self::Confirmed => "CONFIRMED",
            Self::Cancelled => "CANCELLED",
        }
    }
}

#[derive(Debug)]
pub struct ResolvedManifest {
    pub calendar_name: String,
    pub calendar_uid: String,
    pub uid_domain: String,
    pub stamp: Timestamp,
    pub timezone: TimeZone,
    pub events: Vec<ResolvedEvent>,
}

#[derive(Debug)]
pub struct ResolvedEvent {
    pub id: String,
    pub summary: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub url: Option<String>,
    pub status: Option<EventStatus>,
    pub timing: ResolvedTiming,
    pub alerts_before_minutes: Vec<u32>,
    pub sequence: u32,
}

#[derive(Debug)]
pub enum ResolvedTiming {
    Interval { start: Zoned, end: Zoned },
    Deadline { start: Zoned, due: Zoned },
    AllDay { date: Date },
}

const fn default_ddl_display_minutes() -> u32 {
    15
}

impl Manifest {
    pub fn parse(input: &str) -> Result<Self, Error> {
        Ok(toml::from_str(input)?)
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let input = std::fs::read_to_string(path).map_err(|source| Error::ReadManifest {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse(&input)
    }

    pub fn resolve(&self, db: &TimeZoneDatabase) -> Result<ResolvedManifest, Error> {
        if self.events.is_empty() {
            return Err(Error::Empty);
        }

        let timezone = db
            .get(&self.timezone)
            .map_err(|_| Error::UnknownZone(self.timezone.clone()))?;

        let mut seen = HashSet::with_capacity(self.events.len());
        let mut events = Vec::with_capacity(self.events.len());

        for event in &self.events {
            if !seen.insert(event.id.as_str()) {
                return Err(Error::DuplicateId(event.id.clone()));
            }

            events.push(ResolvedEvent {
                id: event.id.clone(),
                summary: event.summary.clone(),
                description: event.description.clone(),
                location: event.location.clone(),
                url: event.url.clone(),
                status: event.status,
                timing: resolve_timing(event, &timezone, self.ddl_display_minutes)?,
                alerts_before_minutes: event
                    .alerts_before_minutes
                    .clone()
                    .unwrap_or_else(|| self.default_alerts_before_minutes.clone()),
                sequence: event.sequence,
            });
        }

        let stamp = to_zoned(&self.updated, &timezone, "updated")?.timestamp();

        Ok(ResolvedManifest {
            calendar_name: self.calendar_name.clone(),
            calendar_uid: self.calendar_uid.clone(),
            uid_domain: self.uid_domain.clone(),
            stamp,
            timezone,
            events,
        })
    }
}

fn resolve_timing(
    event: &Event,
    timezone: &TimeZone,
    ddl_display_minutes: u32,
) -> Result<ResolvedTiming, Error> {
    match &event.timing {
        Timing::Interval { start, end } => {
            if start.time.is_none() || end.time.is_none() {
                return Err(Error::DateOnlyInterval(event.id.clone()));
            }

            let start = to_zoned(start, timezone, &format!("{} start", event.id))?;
            let end = to_zoned(end, timezone, &format!("{} end", event.id))?;

            if end.timestamp() <= start.timestamp() {
                return Err(Error::EndNotAfterStart(event.id.clone()));
            }

            Ok(ResolvedTiming::Interval { start, end })
        }
        Timing::Date { date } => Ok(ResolvedTiming::AllDay {
            date: to_date(date, &format!("{} date", event.id))?,
        }),
        Timing::DdlDateTime { ddl } => {
            if ddl_display_minutes == 0 {
                return Err(Error::InvalidDdlDisplayMinutes);
            }

            let label = format!("{} ddl", event.id);
            let due = to_zoned(ddl, timezone, &label)?;
            let start =
                due.checked_sub(SignedDuration::from_mins(i64::from(ddl_display_minutes)))?;
            Ok(ResolvedTiming::Deadline { start, due })
        }
        Timing::DdlDate { ddl } => Ok(ResolvedTiming::AllDay {
            date: to_date(ddl, &format!("{} ddl", event.id))?,
        }),
    }
}

fn to_zoned(
    value: &toml::value::Datetime,
    timezone: &TimeZone,
    label: &str,
) -> Result<Zoned, Error> {
    let Some(date) = value.date else {
        return Err(Error::TimeWithoutDate {
            label: label.to_owned(),
        });
    };
    let time = value.time.unwrap_or(toml::value::Time {
        hour: 0,
        minute: 0,
        second: None,
        nanosecond: None,
    });

    let civil = jiff::civil::DateTime::new(
        i16::try_from(date.year).map_err(|_| Error::OutOfRange {
            label: label.to_owned(),
        })?,
        i8::try_from(date.month).map_err(|_| Error::OutOfRange {
            label: label.to_owned(),
        })?,
        i8::try_from(date.day).map_err(|_| Error::OutOfRange {
            label: label.to_owned(),
        })?,
        i8::try_from(time.hour).map_err(|_| Error::OutOfRange {
            label: label.to_owned(),
        })?,
        i8::try_from(time.minute).map_err(|_| Error::OutOfRange {
            label: label.to_owned(),
        })?,
        i8::try_from(time.second.unwrap_or(0)).map_err(|_| Error::OutOfRange {
            label: label.to_owned(),
        })?,
        i32::try_from(time.nanosecond.unwrap_or(0)).map_err(|_| Error::OutOfRange {
            label: label.to_owned(),
        })?,
    )
    .map_err(|source| Error::InvalidDatetime {
        label: label.to_owned(),
        source,
    })?;

    let zoned = match value.offset {
        None => civil.to_zoned(timezone.clone()),
        Some(toml::value::Offset::Z) => civil
            .to_zoned(TimeZone::UTC)
            .map(|zoned| zoned.with_time_zone(timezone.clone())),
        Some(toml::value::Offset::Custom { minutes }) => {
            jiff::tz::Offset::from_seconds(i32::from(minutes) * 60)
                .map(TimeZone::fixed)
                .and_then(|fixed| civil.to_zoned(fixed))
                .map(|zoned| zoned.with_time_zone(timezone.clone()))
        }
    };

    zoned.map_err(|source| Error::InvalidDatetime {
        label: label.to_owned(),
        source,
    })
}

fn to_date(value: &toml::value::Date, label: &str) -> Result<Date, Error> {
    Date::new(
        i16::try_from(value.year).map_err(|_| Error::OutOfRange {
            label: label.to_owned(),
        })?,
        i8::try_from(value.month).map_err(|_| Error::OutOfRange {
            label: label.to_owned(),
        })?,
        i8::try_from(value.day).map_err(|_| Error::OutOfRange {
            label: label.to_owned(),
        })?,
    )
    .map_err(|source| Error::InvalidDate {
        label: label.to_owned(),
        source,
    })
}
