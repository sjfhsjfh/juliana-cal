use jiff::{
    Timestamp, Zoned,
    tz::{Dst, Offset, TimeZone, TimeZoneDatabase},
};

use crate::{
    error::Error,
    model::{Manifest, ResolvedDeadline, ResolvedManifest},
};

const FOLD_AT: usize = 75;

pub fn render(manifest: &Manifest) -> Result<String, Error> {
    let db = TimeZoneDatabase::bundled();
    let resolved = manifest.resolve(&db)?;
    render_resolved(&resolved)
}

fn render_resolved(manifest: &ResolvedManifest) -> Result<String, Error> {
    let timezone = manifest.timezone.iana_name().unwrap_or("Etc/UTC");

    let Some(min_start) = manifest
        .deadlines
        .iter()
        .map(|deadline| deadline.start.timestamp())
        .min()
    else {
        return Err(Error::Empty);
    };
    let Some(max_end) = manifest
        .deadlines
        .iter()
        .map(|deadline| deadline.end.timestamp())
        .max()
    else {
        return Err(Error::Empty);
    };

    let window_start = Timestamp::from_second(min_start.as_second() - 365 * 24 * 60 * 60)?
        .to_zoned(manifest.timezone.clone())
        .start_of_day()?
        .timestamp();
    let window_end = Timestamp::from_second(max_end.as_second() + 365 * 24 * 60 * 60)?;

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

    write_timezone(&mut out, &manifest.timezone, window_start, window_end);

    for deadline in &manifest.deadlines {
        write_event(
            &mut out,
            deadline,
            timezone,
            &manifest.uid_domain,
            manifest.stamp,
        );
    }

    write_line(&mut out, "END:VCALENDAR");
    Ok(out)
}

fn write_event(
    out: &mut String,
    deadline: &ResolvedDeadline,
    timezone: &str,
    uid_domain: &str,
    stamp: Timestamp,
) {
    let summary = format!("[{}] {}", deadline.course, deadline.title);

    write_line(out, "BEGIN:VEVENT");
    write_property(out, "SUMMARY", &escape_text(&summary));
    write_line(
        out,
        &format!("DTSTART;TZID={timezone}:{}", format_zoned(&deadline.start)),
    );
    write_line(
        out,
        &format!("DTEND;TZID={timezone}:{}", format_zoned(&deadline.end)),
    );
    write_property(out, "DTSTAMP", &format_utc(stamp));
    write_property(
        out,
        "UID",
        &escape_text(&format!("{}@{uid_domain}", deadline.id)),
    );
    write_property(out, "SEQUENCE", &deadline.sequence.to_string());
    write_property(out, "DESCRIPTION", &escape_text(&description(deadline)));

    if let Some(location) = &deadline.location {
        write_property(out, "LOCATION", &escape_text(location));
    }
    if let Some(url) = &deadline.url {
        write_line(out, &format!("URL:{url}"));
    }
    if let Some(status) = deadline.status {
        write_property(out, "STATUS", status.as_str());
    }

    for minutes in &deadline.alerts_before_minutes {
        write_line(out, "BEGIN:VALARM");
        write_property(out, "ACTION", "DISPLAY");
        write_property(
            out,
            "DESCRIPTION",
            &escape_text(&format!("{}后：{summary}", human_delta(*minutes))),
        );
        write_property(out, "TRIGGER", &trigger(*minutes));
        write_line(out, "END:VALARM");
    }

    write_line(out, "END:VEVENT");
}

fn description(deadline: &ResolvedDeadline) -> String {
    let mut parts = Vec::with_capacity(3);

    if let Some(kind) = &deadline.kind
        && !kind.is_empty()
    {
        parts.push(format!("类型：{kind}"));
    }
    parts.push(format!("课程：{}", deadline.course));
    if let Some(note) = &deadline.note
        && !note.is_empty()
    {
        parts.push(note.clone());
    }

    parts.join("\n")
}

fn human_delta(minutes: u32) -> String {
    if minutes.is_multiple_of(1440) {
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
