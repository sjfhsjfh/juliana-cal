//! TOML manifest -> RFC 5545 iCalendar rendering.

mod error;
mod ics;
mod model;
mod schema;

pub use error::Error;
pub use ics::render;
pub use model::{
    Event, EventStatus, Manifest, ResolvedEvent, ResolvedManifest, ResolvedTiming, Timing,
};
pub use schema::schema;
