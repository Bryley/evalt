use serde::Serialize;

use crate::{
    harness::{HarnessAdaptor, pi},
    sandbox::{Sandbox, cage::CageSandbox},
    types::{config::stack::DEFAULT_CONCURRENT_TESTS, yaml_spec::config::Harness},
};

pub mod stack;

pub struct ResolvedConfig {
    pub profile: Profile,
    pub reviewer: ReviewerConfig,
    pub run: RunConfig,
}

impl ResolvedConfig {
    pub fn reviewer_harness_adapter(&self) -> impl HarnessAdaptor {
        match self.reviewer.profile.harness {
            Harness::Pi => pi::Adaptor,
        }
    }

    pub fn harness_adapter(&self) -> impl HarnessAdaptor {
        match self.profile.harness {
            Harness::Pi => pi::Adaptor,
        }
    }

    pub fn sandbox(&self) -> impl Sandbox {
        CageSandbox
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct Profile {
    pub name: String,
    pub harness: Harness,
    pub extra_args: Vec<String>,
}

pub struct ReviewerConfig {
    pub profile: Profile,
    pub default_pass_threshold: f64,
    pub system_prompt: String,
}

pub struct RunConfig {
    pub concurrent_tests: u64,
    pub timeout_ms: Option<u64>,
    pub max_turns: Option<u64>,
    pub budgets: Budgets,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            concurrent_tests: DEFAULT_CONCURRENT_TESTS,
            timeout_ms: Default::default(),
            max_turns: Default::default(),
            budgets: Default::default(),
        }
    }
}

#[derive(Default)]
pub struct Budgets {
    pub max_tokens: Option<u64>,
    pub max_cost_usd: Option<f64>,
}
