use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};

pub fn run(path: &Path, force: bool) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true);
    if force {
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }

    let mut file = options.open(path).with_context(|| {
        if force {
            format!("writing {}", path.display())
        } else {
            format!("creating {} (use --force to overwrite)", path.display())
        }
    })?;
    file.write_all(template().as_bytes())
        .with_context(|| format!("writing {}", path.display()))?;

    println!("created {}", path.display());
    Ok(())
}

fn template() -> String {
    format!(
        r#"#:schema https://raw.githubusercontent.com/sjfhsjfh/juliana-cal/v{version}/schema.json
# Starter manifest for juliana-cal.
# Replace the example values, then run:
#   juliana-cal check deadlines.toml
#   juliana-cal build deadlines.toml -o deadlines.ics
#
# `updated` is the stable DTSTAMP source. Keep it unchanged unless the
# manifest changes.

timezone = "Asia/Shanghai" # IANA timezone for datetimes without an offset
calendar_name = "My deadlines"
calendar_uid = "my-deadlines@example.invalid" # keep stable; changing it creates a new feed
uid_domain = "example.invalid" # appended to event ids
updated = 2077-01-01T00:00:00Z
default_alerts_before_minutes = [1440, 120]

# Timing forms, one per event:
#   start/end       explicit timed interval
#   date            all-day event
#   ddl datetime    deadline block, ending at the deadline
#   ddl date        all-day deadline

[[event]]
id = "assignment-1"
summary = "Submit assignment 1"
description = "Course: Example 101\nType: homework"
url = "https://example.invalid/assignments/1"
ddl = 2077-02-03T23:59:00
alerts_before_minutes = [1440, 120]

[[event]]
id = "presentation-1"
summary = "Group presentation"
description = "Course: Example 101\nType: presentation"
location = "Room 404"
start = 2077-02-10T14:00:00
end = 2077-02-10T15:30:00
status = "TENTATIVE"
alerts_before_minutes = [60]

[[event]]
id = "exam-1"
summary = "Final exam"
description = "Course: Example 101\nType: exam"
location = "Room 405"
date = 2077-03-05

[[event]]
id = "reading-response-1"
summary = "Reading response due"
ddl = 2077-03-12
"#,
        version = env!("CARGO_PKG_VERSION"),
    )
}

#[cfg(test)]
mod tests {
    use juliana_cal_core::{Manifest, render};

    #[test]
    fn template_is_a_valid_manifest() {
        let manifest = Manifest::parse(&super::template()).expect("template should parse");
        let ics = render(&manifest).expect("template should render");

        assert!(ics.contains("\r\nBEGIN:VEVENT\r\n"));
        assert!(ics.contains("\r\nBEGIN:VALARM\r\n"));
    }
}
