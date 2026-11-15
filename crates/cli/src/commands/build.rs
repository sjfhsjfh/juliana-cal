use std::path::Path;

use anyhow::{Context, Result};
use juliana_cal_core::{Manifest, render};

pub fn run(manifest: &Path, output: Option<&Path>) -> Result<()> {
    let manifest = Manifest::from_path(manifest)?;
    let ics = render(&manifest)?;

    match output {
        Some(path) => std::fs::write(path, ics.as_bytes())
            .with_context(|| format!("writing {}", path.display())),
        None => {
            print!("{ics}");
            Ok(())
        }
    }
}
