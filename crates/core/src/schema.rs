//! JSON Schema generation for TOML manifests.

use schemars::generate::SchemaSettings;
use serde_json::Value;

use crate::Manifest;

pub fn schema() -> Value {
    let generator = SchemaSettings::draft07().into_generator();
    let root = generator.into_root_schema_for::<Manifest>();
    serde_json::to_value(root).expect("JSON Schema is serializable")
}
