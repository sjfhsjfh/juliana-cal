use jiff::{
    Timestamp, Zoned,
    tz::{Dst, Offset, TimeZone, TimeZoneDatabase},
};

use crate::{
    error::Error,
    model::{Manifest, ResolvedEvent, ResolvedManifest, ResolvedTiming},
};

const FOLD_AT: usize = 75;

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
        if let Some((start, end)) = event.timing.bounds() {
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

    let mut out = String::new();
    write_line(&mut out, "BEGIN:VCALENDAR");
    write_property(&mut out, "VERSION", "2.0");
    write_property(&mut out, "PRODID", "-//sjfhsjfh//juliana-cal//EN");
    write_property(&mut out, "CALSCALE", "GREGORIAN");
    write_property(&mut out, "METHOD", "PUBLISH");
    write_property(&mut out, "NAME", &escape_text(&manifest.calendar_name));
    write_property(
        &mut out,
        "X-WR-CALNAME",
        &escape_text(&manifest.calendar_name),
    );
    write_property(&mut out, "UID", &escape_text(&manifest.calendar_uid));

    if let (Some(min_start), Some(max_end)) = (min_start, max_end) {
        let window_start = Timestamp::from_second(min_start.as_second() - 365 * 24 * 60 * 60)?
            .to_zoned(manifest.timezone.clone())
            .start_of_day()?
            .timestamp();
        let window_end = Timestamp::from_second(max_end.as_second() + 365 * 24 * 60 * 60)?;
        write_timezone(&mut out, &manifest.timezone, window_start, window_end);
    }

    for event in &manifest.events {
        write_event(
            &mut out,
            event,
            timezone,
            &manifest.uid_domain,
            manifest.stamp,
        )?;
    }

    write_line(&mut out, "END:VCALENDAR");
    Ok(out)
}

impl ResolvedTiming {
    fn bounds(&self) -> Option<(Timestamp, Timestamp)> {
        match self {
            Self::Interval { start, end } => Some((start.timestamp(), end.timestamp())),
            Self::Deadline { start, due } => Some((start.timestamp(), due.timestamp())),
            Self::AllDay { .. } => None,
        }
    }
}

fn write_event(
    out: &mut String,
    event: &ResolvedEvent,
    timezone: &str,
    uid_domain: &str,
    stamp: Timestamp,
) -> Result<(), Error> {
    write_line(out, "BEGIN:VEVENT");
    write_property(out, "SUMMARY", &escape_text(&event.summary));

    match &event.timing {
        ResolvedTiming::Interval { start, end } => {
            write_timed(out, "DTSTART", timezone, start);
            write_timed(out, "DTEND", timezone, end);
        }
        ResolvedTiming::Deadline { start, due } => {
            write_timed(out, "DTSTART", timezone, start);
            write_timed(out, "DTEND", timezone, due);
        }
        ResolvedTiming::AllDay { date } => {
            write_line(out, &format!("DTSTART;VALUE=DATE:{}", format_date(date)));
            let next = (*date).tomorrow()?;
            write_line(out, &format!("DTEND;VALUE=DATE:{}", format_date(&next)));
        }
    }

    write_property(out, "DTSTAMP", &format_utc(stamp));
    write_property(
        out,
        "UID",
        &escape_text(&format!("{}@{uid_domain}", event.id)),
    );
    write_property(out, "SEQUENCE", &event.sequence.to_string());

    if let Some(description) = &event.description
        && !description.is_empty()
    {
        write_property(out, "DESCRIPTION", &escape_text(description));
    }
    if let Some(location) = &event.location {
        write_property(out, "LOCATION", &escape_text(location));
    }
    if let Some(url) = &event.url {
        write_line(out, &format!("URL:{url}"));
    }
    if let Some(status) = event.status {
        write_property(out, "STATUS", status.as_str());
    }

    let related_end = matches!(&event.timing, ResolvedTiming::Deadline { .. });
    for minutes in &event.alerts_before_minutes {
        write_alarm(out, &event.summary, *minutes, related_end);
    }

    write_line(out, "END:VEVENT");
    Ok(())
}

fn write_timed(out: &mut String, property: &str, timezone: &str, value: &Zoned) {
    write_line(
        out,
        &format!("{property};TZID={timezone}:{}", format_zoned(value)),
    );
}

fn write_alarm(out: &mut String, summary: &str, minutes: u32, related_end: bool) {
    write_line(out, "BEGIN:VALARM");
    write_property(out, "ACTION", "DISPLAY");
    write_property(
        out,
        "DESCRIPTION",
        &escape_text(&format!("{}后：{summary}", human_delta(minutes))),
    );
    if related_end {
        write_line(out, &format!("TRIGGER;RELATED=END:{}", trigger(minutes)));
    } else {
        write_property(out, "TRIGGER", &trigger(minutes));
    }
    write_line(out, "END:VALARM");
}

fn human_delta(minutes: u32) -> String {
    if minutes == 0 {
        "0 分钟".to_owned()
    } else if minutes.is_multiple_of(1440) {
        format!("{} 天", minutes / 1440)
    } else if minutes.is_multiple_of(60) {
        format!("{} 小时", minutes / 60)
    } else {
        format!("{minutes} 分钟")
    }
}

fn trigger(minutes: u32) -> String {
    if minutes == 0 {
        "PT0S".to_owned()
    } else if minutes.is_multiple_of(1440) {
        format!("-P{}D", minutes / 1440)
    } else if minutes.is_multiple_of(60) {
        format!("-PT{}H", minutes / 60)
    } else {
        format!("-PT{minutes}M")
    }
}

fn format_zoned(zoned: &Zoned) -> String {
    zoned.datetime().strftime("%Y%m%dT%H%M%S").to_string()
}

fn format_utc(stamp: Timestamp) -> String {
    let datetime = stamp.to_zoned(TimeZone::UTC).datetime();
    format!("{}Z", datetime.strftime("%Y%m%dT%H%M%S"))
}

fn format_date(date: &jiff::civil::Date) -> String {
    date.strftime("%Y%m%d").to_string()
}

fn write_timezone(out: &mut String, timezone: &TimeZone, start: Timestamp, end: Timestamp) {
    let tzid = timezone.iana_name().unwrap_or("Etc/UTC");
    let initial = timezone.to_offset_info(start);

    write_line(out, "BEGIN:VTIMEZONE");
    write_property(out, "TZID", &escape_text(tzid));
    write_observance(
        out,
        observance_kind(initial.dst()),
        start,
        initial.offset(),
        initial.offset(),
        initial.abbreviation(),
    );

    let mut previous = initial.offset();
    for transition in timezone
        .following(start)
        .take_while(|t| t.timestamp() <= end)
    {
        write_observance(
            out,
            observance_kind(transition.dst()),
            transition.timestamp(),
            previous,
            transition.offset(),
            transition.abbreviation(),
        );
        previous = transition.offset();
    }

    write_line(out, "END:VTIMEZONE");
}

fn observance_kind(dst: Dst) -> &'static str {
    if dst == Dst::Yes {
        "DAYLIGHT"
    } else {
        "STANDARD"
    }
}

fn write_observance(
    out: &mut String,
    kind: &str,
    at: Timestamp,
    from: Offset,
    to: Offset,
    name: &str,
) {
    let local = at.to_zoned(TimeZone::fixed(from)).datetime();

    write_line(out, &format!("BEGIN:{kind}"));
    write_property(out, "DTSTART", &local.strftime("%Y%m%dT%H%M%S").to_string());
    write_property(out, "TZNAME", &escape_text(name));
    write_property(out, "TZOFFSETFROM", &ics_offset(from));
    write_property(out, "TZOFFSETTO", &ics_offset(to));
    write_line(out, &format!("END:{kind}"));
}

fn ics_offset(offset: Offset) -> String {
    let seconds = offset.seconds();
    let sign = if seconds < 0 { '-' } else { '+' };
    let seconds = seconds.unsigned_abs();
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let seconds = seconds % 60;

    if seconds == 0 {
        format!("{sign}{hours:02}{minutes:02}")
    } else {
        format!("{sign}{hours:02}{minutes:02}{seconds:02}")
    }
}

fn write_property(out: &mut String, name: &str, value: &str) {
    write_line(out, &format!("{name}:{value}"));
}

fn write_line(out: &mut String, line: &str) {
    let mut current = String::new();
    let mut current_len = 0usize;

    for ch in line.chars() {
        let ch_len = ch.len_utf8();
        if current_len + ch_len > FOLD_AT {
            // Do not split an escape sequence across a folded line.
            let trailing_backslashes = current
                .chars()
                .rev()
                .take_while(|&character| character == '\\')
                .count();
            if trailing_backslashes % 2 == 1 {
                current.pop();
                out.push_str(&current);
                out.push_str("\r\n ");
                current.clear();
                current.push('\\');
                current_len = 2;
            } else {
                out.push_str(&current);
                out.push_str("\r\n ");
                current.clear();
                current_len = 1;
            }
        }

        current.push(ch);
        current_len += ch_len;
    }

    out.push_str(&current);
    out.push_str("\r\n");
}

fn escape_text(value: &str) -> String {
    let normalized = value.replace("\r\n", "\n").replace('\r', "\n");
    let mut escaped = String::with_capacity(normalized.len());

    for ch in normalized.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            ';' => escaped.push_str("\\;"),
            ',' => escaped.push_str("\\,"),
            '\n' => escaped.push_str("\\n"),
            _ => escaped.push(ch),
        }
    }

    escaped
}
