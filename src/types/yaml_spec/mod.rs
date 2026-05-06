use regex::Regex;
use schemars::schema_for;
use serde::{Deserialize as _, Deserializer};

#[allow(unused_imports)]
pub use assertions::Assertion;
#[allow(unused_imports)]
pub use testfile::{Input, TestCase, TestFile};

#[allow(unused_imports)]
pub use config::Config;

pub mod assertions;
pub mod config;
pub mod operations;
pub mod testfile;

pub fn testfile_schema_json() -> String {
    let schema = schema_for!(TestFile);
    serde_json::to_string_pretty(&schema).expect("failed to serialize `TestFile` JSON schema")
}

pub fn config_schema_json() -> String {
    let schema = schema_for!(Config);
    serde_json::to_string_pretty(&schema).expect("failed to serialize `Config` JSON schema")
}

fn deserialize_optional_regex<'de, D>(deserializer: D) -> Result<Option<Regex>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    match raw {
        Some(pattern) => Regex::new(&pattern)
            .map(Some)
            .map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}

fn deserialize_regex<'de, D>(deserializer: D) -> Result<Regex, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    Regex::new(&raw).map_err(serde::de::Error::custom)
}
