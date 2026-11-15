use std::collections::HashSet;
use std::path::Path;

use jiff::{
    Timestamp, Zoned,
    tz::{Offset, TimeZone, TimeZoneDatabase},
};
use serde::Deserialize;

use crate::error::Error;

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub timezone: String,
    pub calendar_name: String,
    pub calendar_uid: String,
    pub uid_domain: String,

    /// Stable `DTSTAMP` source. Keep this fixed unless the manifest changes.
    pub updated: toml::value::Datetime,

    pub default_alerts_before_minutes: Vec<u32>,

    #[serde(rename = "deadline")]
    pub deadlines: Vec<Deadline>,
}

#[derive(Debug, Deserialize)]
pub struct Deadline {
    pub id: String,
    pub course: String,
    pub title: String,
    pub kind: Option<String>,
    pub location: Option<String>,
    pub note: Option<String>,
    pub url: Option<String>,
    pub status: Option<EventStatus>,
    pub start: toml::value::Datetime,
    pub end: toml::value::Datetime,
    pub alerts_before_minutes: Option<Vec<u32>>,

    #[serde(default)]
    pub sequence: u32,
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
    pub deadlines: Vec<ResolvedDeadline>,
}

#[derive(Debug)]
pub struct ResolvedDeadline {
    pub id: String,
    pub course: String,
    pub title: String,
    pub kind: Option<String>,
    pub location: Option<String>,
    pub note: Option<String>,
    pub url: Option<String>,
    pub status: Option<EventStatus>,
    pub start: Zoned,
    pub end: Zoned,
    pub alerts_before_minutes: Vec<u32>,
    pub sequence: u32,
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
        if self.deadlines.is_empty() {
            return Err(Error::Empty);
        }

        let timezone = db
            .get(&self.timezone)
            .map_err(|_| Error::UnknownZone(self.timezone.clone()))?;

        let mut seen = HashSet::with_capacity(self.deadlines.len());
        let mut deadlines = Vec::with_capacity(self.deadlines.len());

        for deadline in &self.deadlines {
            if !seen.insert(deadline.id.as_str()) {
                return Err(Error::DuplicateId(deadline.id.clone()));
            }

            let start = to_zoned(&deadline.start, &timezone, &deadline.id)?;
            let end = to_zoned(&deadline.end, &timezone, &deadline.id)?;

            if end.timestamp() < start.timestamp() {
                return Err(Error::EndBeforeStart(deadline.id.clone()));
            }

            deadlines.push(ResolvedDeadline {
                id: deadline.id.clone(),
                course: deadline.course.clone(),
                title: deadline.title.clone(),
                kind: deadline.kind.clone(),
                location: deadline.location.clone(),
                note: deadline.note.clone(),
                url: deadline.url.clone(),
                status: deadline.status,
                start,
                end,
                alerts_before_minutes: deadline
                    .alerts_before_minutes
                    .clone()
                    .unwrap_or_else(|| self.default_alerts_before_minutes.clone()),
                sequence: deadline.sequence,
            });
        }

        let stamp = to_zoned(&self.updated, &timezone, "updated")?.timestamp();

        Ok(ResolvedManifest {
            calendar_name: self.calendar_name.clone(),
            calendar_uid: self.calendar_uid.clone(),
            uid_domain: self.uid_domain.clone(),
            stamp,
            timezone,
            deadlines,
        })
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
            Offset::from_seconds(i32::from(minutes) * 60)
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
