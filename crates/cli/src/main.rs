mod cli;
mod commands;

use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    if let Err(error) = crate::cli::Cli::parse().command.run() {
        eprintln!("error: {error:#}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
