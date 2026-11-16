//! RFC 5545 rendering.

mod event;
mod format;
mod timezone;
mod writer;

#[cfg(test)]
mod tests;

use jiff::{Timestamp, tz::TimeZoneDatabase};

use crate::{
    error::Error,
    model::{Manifest, ResolvedManifest},
};

use event::{bounds, write_event};
use timezone::write_timezone;
use writer::Writer;

pub const PRODID_OWNER: &str = "sjfhsjfh";
pub const PRODID_PRODUCT: &str = "juliana-cal";

pub fn render(manifest: &Manifest) -> Result<String, Error> {
    let db = TimeZoneDatabase::bundled();
    let resolved = manifest.resolve(&db)?;
    render_resolved(&resolved)
}

fn render_resolved(manifest: &ResolvedManifest) -> Result<String, Error> {
    let timezone = manifest.timezone.iana_name().unwrap_or("Etc/UTC");

    let mut min_start: Option<Timestamp> = None;
    let mut max_end: Option<Timestamp> = None;

    for event in &manifest.events {
        if let Some((start, end)) = bounds(&event.timing) {
            min_start = Some(match min_start {
                Some(current) if current < start => current,
                _ => start,
            });
            max_end = Some(match max_end {
                Some(current) if current > end => current,
                _ => end,
            });
        }
    }

    let mut writer = Writer::new();
    writer.begin("VCALENDAR");
    writer.property("VERSION", "2.0");
    writer.property(
        "PRODID",
        format!("-//{}//{}//EN", PRODID_OWNER, PRODID_PRODUCT),
    );
    writer.property("CALSCALE", "GREGORIAN");
    writer.property("NAME", format::escape_text(manifest.calendar_name.as_str()));
    writer.property(
        "X-WR-CALNAME",
        format::escape_text(manifest.calendar_name.as_str()),
    );
    writer.property("UID", format::escape_text(manifest.calendar_uid.as_str()));

    if let (Some(min_start), Some(max_end)) = (min_start, max_end) {
        let window_start = Timestamp::from_second(min_start.as_second() - 365 * 24 * 60 * 60)?
            .to_zoned(manifest.timezone.clone())
            .start_of_day()?
            .timestamp();
        let window_end = Timestamp::from_second(max_end.as_second() + 365 * 24 * 60 * 60)?;
        write_timezone(&mut writer, &manifest.timezone, window_start, window_end);
    }

    for event in &manifest.events {
        write_event(
            &mut writer,
            event,
            timezone,
            manifest.uid_domain.as_str(),
            manifest.stamp,
        )?;
    }

    writer.end("VCALENDAR");
    Ok(writer.finish())
}
