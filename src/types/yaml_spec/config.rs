use std::{env, path::PathBuf};

use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Config {
    /// List of profiles
    pub profiles: Option<IndexMap<String, Profile>>,
    /// Selected profile
    pub profile: Option<String>,
    /// Reviewer config options
    pub reviewer: Option<ReviewerConfig>,
    /// Run options such as budgets and more
    pub run: Option<RunConfig>,
}
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub struct Profile {
    /// Harness to use
    pub harness: Harness,
    /// Any extra args to pass into the harness outside the nessessary
    #[serde(default)]
    pub extra_args: Vec<String>,
}

#[derive(Debug, Default, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub struct ReviewerConfig {
    /// The profile to use for reviewing
    pub profile: Option<String>,
    /// The default pass threshold to use if not specified
    pub default_pass_threshold: Option<f64>,
    /// Override the system prompt used for reviewing to this
    pub system_prompt: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub struct RunConfig {
    /// The max number of concurrent tests to run at a time (defaults to 10).
    ///
    /// Note that this option is a global only option and cannot be set on the testfile or
    /// individual test level.
    pub concurrent_tests: Option<u64>,
    /// If the test case runs for longer than this many miliseconds then interupt
    pub timeout_ms: Option<u64>,
    /// If the test case runs for more turns than this then interupt
    pub max_turns: Option<u64>,
    /// Extra budget settings
    pub budgets: Option<Budgets>,
}

#[derive(Debug, Default, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub struct Budgets {
    /// Max number of tokens allowed before interupt
    pub max_tokens: Option<u64>,
    /// Max cost (if measured) in USD allowed before interupt
    pub max_cost_usd: Option<f64>,
}

#[derive(Debug, Serialize, Clone, Copy, Deserialize, JsonSchema, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum Harness {
    Pi,
}

impl Harness {
    pub fn extra_allowed_paths(&self) -> anyhow::Result<Vec<PathBuf>> {
        match self {
            Harness::Pi => {
                let pi_settings = env::var_os("HOME")
                    .map(PathBuf::from)
                    .ok_or_else(|| anyhow::anyhow!("HOME env var not set"))?
                    .join(".pi/agent");

                Ok(vec![pi_settings])
            }
        }
    }
}
