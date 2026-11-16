use jiff::{
    Timestamp,
    tz::{Dst, Offset, TimeZone},
};

use super::{format::escape_text, writer::Writer};

pub(super) fn write_timezone(
    writer: &mut Writer,
    timezone: &TimeZone,
    start: Timestamp,
    end: Timestamp,
) {
    let tzid = timezone.iana_name().unwrap_or("Etc/UTC");
    let initial = timezone.to_offset_info(start);

    writer.begin("VTIMEZONE");
    writer.property("TZID", escape_text(tzid));
    write_observance(
        writer,
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
            writer,
            observance_kind(transition.dst()),
            transition.timestamp(),
            previous,
            transition.offset(),
            transition.abbreviation(),
        );
        previous = transition.offset();
    }

    writer.end("VTIMEZONE");
}

fn observance_kind(dst: Dst) -> &'static str {
    if dst == Dst::Yes {
        "DAYLIGHT"
    } else {
        "STANDARD"
    }
}

fn write_observance(
    writer: &mut Writer,
    kind: impl AsRef<str>,
    at: Timestamp,
    from: Offset,
    to: Offset,
    name: impl AsRef<str>,
) {
    let kind = kind.as_ref();
    let local = at.to_zoned(TimeZone::fixed(from)).datetime();

    writer.begin(kind);
    writer.property("DTSTART", local.strftime("%Y%m%dT%H%M%S").to_string());
    writer.property("TZNAME", escape_text(name));
    writer.property("TZOFFSETFROM", ics_offset(from));
    writer.property("TZOFFSETTO", ics_offset(to));
    writer.end(kind);
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
