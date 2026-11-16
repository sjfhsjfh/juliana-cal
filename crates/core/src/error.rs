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

    #[error("invalid TOML")]
    Toml(#[from] toml::de::Error),

    #[error("unknown time zone `{0}`")]
    UnknownZone(String),

    #[error("no [[event]] entries in manifest")]
    Empty,

    #[error("duplicate event id `{0}`")]
    DuplicateId(String),

    #[error("event `{0}`: `start`/`end` must include a time; use `date` for an all-day event")]
    DateOnlyInterval(String),

    #[error("event `{0}`: end must be after start")]
    EndNotAfterStart(String),

    #[error("`ddl_display_minutes` must be greater than zero")]
    InvalidDdlDisplayMinutes,

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

    #[error("`{label}`: invalid date: {source}")]
    InvalidDate {
        label: String,
        #[source]
        source: jiff::Error,
    },

    #[error(transparent)]
    Jiff(#[from] jiff::Error),
}
