use crate::{Error, Manifest};

const HEADER: &str = r#"
timezone = "Asia/Shanghai"
calendar_name = "Juliana's Suspicious Deadlines"
calendar_uid = "suspicious-calendar@example.invalid"
uid_domain = "example.invalid"
updated = 2031-01-01T00:00:00Z
"#;

fn manifest(events: &str) -> Manifest {
    Manifest::parse(&format!("{HEADER}\n{events}")).expect("fixture manifest should parse")
}

fn render(events: &str) -> String {
    super::render(&manifest(events)).expect("fixture calendar should render")
}

fn unfold(rendered: &str) -> String {
    icalendar::parser::unfold(rendered)
}

fn parse(unfolded: &str) -> icalendar::parser::Calendar<'_> {
    icalendar::parser::read_calendar(unfolded).expect("rendered calendar should parse")
}

fn events<'a>(
    calendar: &'a icalendar::parser::Calendar<'a>,
) -> Vec<&'a icalendar::parser::Component<'a>> {
    calendar
        .components
        .iter()
        .filter(|component| component.name.as_str() == "VEVENT")
        .collect()
}

fn property<'a>(component: &'a icalendar::parser::Component<'a>, name: &str) -> &'a str {
    component
        .find_prop(name)
        .unwrap_or_else(|| panic!("missing property `{name}`"))
        .val
        .as_str()
}

#[test]
fn renders_calendar_that_icalendar_parses() {
    let output = render(
        r#"
[[event]]
id = "suspicious-deadline"
summary = "Juliana's Suspicious Deadlines"
start = 2031-02-03T09:00:00
end = 2031-02-03T10:00:00
alerts_before_minutes = [10]
"#,
    );

    let unfolded = unfold(&output);
    let calendar = parse(&unfolded);
    let events = events(&calendar);

    assert_eq!(events.len(), 1);
    assert_eq!(
        property(events[0], "SUMMARY"),
        "Juliana's Suspicious Deadlines"
    );
    assert_eq!(
        property(events[0], "UID"),
        "suspicious-deadline@example.invalid"
    );
    assert_eq!(
        events[0]
            .components
            .iter()
            .filter(|component| component.name.as_str() == "VALARM")
            .count(),
        1
    );
}

#[test]
fn folds_long_utf8_summary_within_75_octets() {
    let summary = "Juliana's 可疑截止日期: Room 404, \
                   Department of Questionable Scheduling. "
        .repeat(3);
    let output = render(&format!(
        "[[event]]\nid = \"long-summary\"\nsummary = {summary:?}\n\
         start = 2031-04-01T09:00:00\nend = 2031-04-01T09:30:00\n"
    ));

    for line in output.split("\r\n") {
        assert!(
            line.len() <= 75,
            "physical line exceeds 75 octets: {line:?}"
        );
    }
    assert!(output.contains("\r\n "), "long line should be folded");

    let unfolded = unfold(&output);
    let calendar = parse(&unfolded);
    assert_eq!(property(events(&calendar)[0], "SUMMARY"), summary);
}

#[test]
fn escapes_text_special_characters() {
    let summary = "Room 404, Building 7; bring \\ snacks\nand regrets";
    let output = render(&format!(
        "[[event]]\nid = \"escaped-summary\"\nsummary = {summary:?}\n\
         start = 2031-04-02T09:00:00\nend = 2031-04-02T09:30:00\n"
    ));

    assert!(output.contains("SUMMARY:Room 404\\, Building 7\\; bring \\\\ snacks\\nand regrets"));

    let unfolded = unfold(&output);
    let calendar = parse(&unfolded);
    let parsed = events(&calendar)[0]
        .find_prop("SUMMARY")
        .expect("missing SUMMARY")
        .val
        .clone()
        .unescape_text();
    assert_eq!(parsed.as_str(), summary);
}

#[test]
fn emits_vtimezone_for_non_utc_event() {
    let output = render(
        r#"
[[event]]
id = "shanghai-event"
summary = "Room 404"
start = 2031-02-03T09:00:00
end = 2031-02-03T10:00:00
"#,
    );

    assert!(output.contains("BEGIN:VTIMEZONE"));
    assert!(output.contains("DTSTART;TZID=Asia/Shanghai:20310203T090000"));

    let unfolded = unfold(&output);
    let calendar = parse(&unfolded);
    assert!(
        calendar
            .components
            .iter()
            .any(|component| component.name.as_str() == "VTIMEZONE")
    );
}

#[test]
fn all_day_event_uses_exclusive_end_date() {
    let output = render(
        r#"
[[event]]
id = "all-day-event"
summary = "Department of Questionable Scheduling"
date = 2031-03-05
"#,
    );

    assert!(output.contains("DTSTART;VALUE=DATE:20310305"));
    assert!(output.contains("DTEND;VALUE=DATE:20310306"));
    let unfolded = unfold(&output);
    parse(&unfolded);
}

#[test]
fn rejects_invalid_timing_and_duplicate_ids() {
    let backwards = super::render(&manifest(
        r#"
[[event]]
id = "backwards"
summary = "Time flows the other way"
start = 2031-05-01T10:00:00
end = 2031-05-01T09:00:00
"#,
    ));
    assert!(matches!(backwards, Err(Error::EndNotAfterStart(id)) if id == "backwards"));

    let duplicate = super::render(&manifest(
        r#"
[[event]]
id = "twice"
summary = "Room 404"
start = 2031-05-02T09:00:00
end = 2031-05-02T10:00:00

[[event]]
id = "twice"
summary = "Room 405"
start = 2031-05-03T09:00:00
end = 2031-05-03T10:00:00
"#,
    ));
    assert!(matches!(duplicate, Err(Error::DuplicateId(id)) if id == "twice"));
}
