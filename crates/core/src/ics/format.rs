use jiff::{Timestamp, Zoned, civil::Date, tz::TimeZone};

pub(super) fn trigger(minutes: u32) -> String {
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

pub(super) fn format_zoned(zoned: &Zoned) -> String {
    zoned.datetime().strftime("%Y%m%dT%H%M%S").to_string()
}

pub(super) fn format_utc(stamp: Timestamp) -> String {
    let datetime = stamp.to_zoned(TimeZone::UTC).datetime();
    format!("{}Z", datetime.strftime("%Y%m%dT%H%M%S"))
}

pub(super) fn format_date(date: &Date) -> String {
    date.strftime("%Y%m%d").to_string()
}

pub(super) fn escape_text(value: impl AsRef<str>) -> String {
    let normalized = value.as_ref().replace("\r\n", "\n").replace('\r', "\n");
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
