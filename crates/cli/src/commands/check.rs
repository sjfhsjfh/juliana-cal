use std::path::Path;

use anyhow::Result;
use juliana_cal_core::{Manifest, render};

pub fn run(manifest: &Path) -> Result<()> {
    let manifest = Manifest::from_path(manifest)?;
    let ics = render(&manifest)?;
    let deadlines = ics.matches("\r\nBEGIN:VEVENT\r\n").count();
    let alarms = ics.matches("\r\nBEGIN:VALARM\r\n").count();

    println!("{deadlines} deadlines, {alarms} alarms");
    Ok(())
}
