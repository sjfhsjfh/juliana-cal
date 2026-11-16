use jiff::{Timestamp, Zoned};

use crate::{
    error::Error,
    model::{ResolvedEvent, ResolvedTiming},
};

use super::{
    format::{escape_text, format_date, format_utc, format_zoned, trigger},
    writer::Writer,
};

pub(super) fn bounds(timing: &ResolvedTiming) -> Option<(Timestamp, Timestamp)> {
    match timing {
        ResolvedTiming::Interval { start, end } => Some((start.timestamp(), end.timestamp())),
        ResolvedTiming::Deadline { start, due } => Some((start.timestamp(), due.timestamp())),
        ResolvedTiming::AllDay { .. } => None,
    }
}

pub(super) fn write_event(
    writer: &mut Writer,
    event: &ResolvedEvent,
    timezone: impl AsRef<str>,
    uid_domain: impl AsRef<str>,
    stamp: Timestamp,
) -> Result<(), Error> {
    let timezone = timezone.as_ref();
    let uid_domain = uid_domain.as_ref();

    writer.begin("VEVENT");
    writer.property("SUMMARY", escape_text(event.summary.as_str()));

    match &event.timing {
        ResolvedTiming::Interval { start, end } => {
            write_timed(writer, "DTSTART", timezone, start);
            write_timed(writer, "DTEND", timezone, end);
        }
        ResolvedTiming::Deadline { start, due } => {
            write_timed(writer, "DTSTART", timezone, start);
            write_timed(writer, "DTEND", timezone, due);
        }
        ResolvedTiming::AllDay { date } => {
            writer.line(format!("DTSTART;VALUE=DATE:{}", format_date(date)));
            let next = (*date).tomorrow()?;
            writer.line(format!("DTEND;VALUE=DATE:{}", format_date(&next)));
        }
    }

    writer.property("DTSTAMP", format_utc(stamp));
    writer.property("UID", escape_text(format!("{}@{uid_domain}", event.id)));
    writer.property("SEQUENCE", event.sequence.to_string());

    if let Some(description) = &event.description
        && !description.is_empty()
    {
        writer.property("DESCRIPTION", escape_text(description.as_str()));
    }
    if let Some(location) = &event.location {
        writer.property("LOCATION", escape_text(location.as_str()));
    }
    if let Some(url) = &event.url {
        writer.line(format!("URL:{url}"));
    }
    if let Some(status) = event.status {
        writer.property("STATUS", status.as_str());
    }

    let related_end = matches!(&event.timing, ResolvedTiming::Deadline { .. });
    for minutes in &event.alerts_before_minutes {
        write_alarm(writer, event.summary.as_str(), *minutes, related_end);
    }

    writer.end("VEVENT");
    Ok(())
}

fn write_timed(
    writer: &mut Writer,
    property: impl AsRef<str>,
    timezone: impl AsRef<str>,
    value: &Zoned,
) {
    writer.line(format!(
        "{};TZID={}:{}",
        property.as_ref(),
        timezone.as_ref(),
        format_zoned(value)
    ));
}

fn write_alarm(writer: &mut Writer, summary: impl AsRef<str>, minutes: u32, related_end: bool) {
    let summary = summary.as_ref();

    writer.begin("VALARM");
    writer.property("ACTION", "DISPLAY");
    // TRIGGER already carries the relative offset. Repeating it as localized
    // prose would duplicate timing and hard-code a language the feed does not
    // declare; keep DESCRIPTION to the user-provided summary instead.
    writer.property("DESCRIPTION", escape_text(summary));
    if related_end {
        writer.line(format!("TRIGGER;RELATED=END:{}", trigger(minutes)));
    } else {
        writer.property("TRIGGER", trigger(minutes));
    }
    writer.end("VALARM");
}
