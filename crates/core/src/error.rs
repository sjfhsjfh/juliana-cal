use std::io;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to read manifest `{path}`: {source}")]
    ReadManifest {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("invalid TOML: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("unknown time zone `{0}`")]
    UnknownZone(String),

    #[error("no [[deadline]] entries in manifest")]
    Empty,

    #[error("duplicate deadline id `{0}`")]
    DuplicateId(String),

    #[error("`{label}`: local time without a date is not supported")]
    TimeWithoutDate { label: String },

    #[error("`{label}`: datetime component out of range")]
    OutOfRange { label: String },

    #[error("`{label}`: invalid datetime: {source}")]
    InvalidDatetime {
        label: String,
        #[source]
        source: jiff::Error,
    },

    #[error("deadline `{0}`: end is before start")]
    EndBeforeStart(String),

    #[error(transparent)]
    Jiff(#[from] jiff::Error),
}
