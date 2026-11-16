use std::path::PathBuf;

#[cfg(feature = "server")]
use std::net::SocketAddr;

use clap::{Parser, Subcommand};

use crate::commands;

#[derive(Debug, Parser)]
#[command(
    name = "juliana-cal",
    version,
    about = "Build and serve course-deadline ICS feeds"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Render a TOML manifest into an .ics file.
    Build {
        /// Path to the TOML manifest.
        manifest: PathBuf,

        /// Output file; defaults to stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Parse and render a manifest without writing anything.
    Check {
        /// Path to the TOML manifest.
        manifest: PathBuf,
    },

    /// Create a starter TOML manifest.
    New {
        /// Path to write.
        path: PathBuf,

        /// Overwrite an existing file.
        #[arg(long)]
        force: bool,
    },

    /// Serve a directory, preferring .ics files over .toml fallbacks.
    #[cfg(feature = "server")]
    Serve {
        /// Directory containing .ics and/or .toml files.
        dir: PathBuf,

        /// Listen address.
        #[arg(long, default_value = "127.0.0.1:8787")]
        bind: SocketAddr,
    },
}

impl Command {
    pub fn run(self) -> anyhow::Result<()> {
        match self {
            Command::Build { manifest, output } => {
                commands::build::run(&manifest, output.as_deref())
            }
            Command::Check { manifest } => commands::check::run(&manifest),
            Command::New { path, force } => commands::new::run(&path, force),
            #[cfg(feature = "server")]
            Command::Serve { dir, bind } => commands::serve::run(dir, bind),
        }
    }
}
