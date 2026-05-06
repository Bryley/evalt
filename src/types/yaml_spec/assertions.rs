use std::path::PathBuf;

use miette::bail;
use regex::Regex;
use schemars::JsonSchema;
use serde::Deserialize;

use super::operations::{BoolOp, NumericOp, StringOp};

use super::deserialize_optional_regex;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Assertion {
    Comparison {
        #[serde(flatten)]
        comparison: ComparisonAssertion,
    },
    Not {
        not: Box<Assertion>,
    },
    All {
        all: Vec<Assertion>,
    },
    Any {
        any: Vec<Assertion>,
    },
    Review {
        review: ReviewAssertion,
    },
}

/// A typed comparison assertion over a known normalized run value.
///
/// The `left` tag selects which value from the run is inspected. Each variant fixes the type of
/// `right` and the set of valid operators, so schema generation can provide useful autocomplete
/// and validation.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(tag = "left")]
pub enum ComparisonAssertion {
    /// The duration of the prompt then response in seconds
    #[serde(rename = "duration-secs")]
    DurationSecs {
        #[serde(flatten)]
        op: NumericOp<f64>,
    },

    /// The number of turns the AI took
    #[serde(rename = "turns")]
    Turns {
        #[serde(flatten)]
        op: NumericOp<u64>,
    },

    /// Total number of input tokens used
    #[serde(rename = "tokens.input")]
    TokensInput {
        #[serde(flatten)]
        op: NumericOp<u64>,
    },

    /// Total number of output tokens used (assistant's response)
    #[serde(rename = "tokens.output")]
    TokensOutput {
        #[serde(flatten)]
        op: NumericOp<u64>,
    },

    /// Total number of thinking tokens generated
    #[serde(rename = "tokens.thinking")]
    TokensThinking {
        #[serde(flatten)]
        op: NumericOp<u64>,
    },

    /// Total number of tokens used together
    #[serde(rename = "tokens.total")]
    TokensTotal {
        #[serde(flatten)]
        op: NumericOp<u64>,
    },

    /// The total cost of the response (if applicable) in USD
    #[serde(rename = "tokens.cost-usd")]
    TokensCost {
        #[serde(flatten)]
        op: NumericOp<f64>,
    },

    /// The thinking response
    #[serde(rename = "output.thinking")]
    OutputThinking {
        #[serde(flatten)]
        op: StringOp,
    },

    /// The assistant's response
    #[serde(rename = "output")]
    OutputAssistant {
        #[serde(flatten)]
        op: StringOp,
    },

    /// Compare the number of times certain tool calls occurred.
    #[serde(rename = "tool.calls")]
    ToolCallCount {
        /// Optional regex name of the tool to match.
        #[serde(default, deserialize_with = "deserialize_optional_regex")]
        #[schemars(with = "Option<String>")]
        tool_name: Option<Regex>,
        /// Optional regex of compacted JSON tool parameters passed through.
        #[serde(default, deserialize_with = "deserialize_optional_regex")]
        #[schemars(with = "Option<String>")]
        tool_params: Option<Regex>,
        /// Numeric operator used to compare tool-call count.
        #[serde(flatten)]
        op: NumericOp<u64>,
    },

    /// Check whether a tool with the given name was called at least once.
    #[serde(rename = "tool.called")]
    ToolCalled {
        /// Optional regex name of the tool to match.
        #[serde(default, deserialize_with = "deserialize_optional_regex")]
        #[schemars(with = "Option<String>")]
        tool_name: Option<Regex>,
        /// Optional regex of compacted JSON tool parameters passed through.
        #[serde(default, deserialize_with = "deserialize_optional_regex")]
        #[schemars(with = "Option<String>")]
        tool_params: Option<Regex>,
        /// Boolean operator used to compare whether the tool was called.
        #[serde(flatten)]
        op: BoolOp<bool>,
    },

    /// Check whether a file exists in the final isolated workspace.
    #[serde(rename = "file.exists")]
    FileExists {
        /// Workspace-relative filepath to inspect.
        path: PathBuf,
        /// Boolean operator used to compare file existence.
        #[serde(flatten)]
        op: BoolOp<bool>,
    },

    /// Compare the final contents of a file in the isolated workspace.
    #[serde(rename = "file.content")]
    FileContent {
        /// Workspace-relative filepath to read.
        path: PathBuf,
        /// String operator used to compare file contents.
        #[serde(flatten)]
        op: StringOp,
    },
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum StatusOptions {
    #[default]
    Pending,
    Passed,
    Failed,
    Error,
    Timeout,
}

/// AI-reviewer assertion for semantic or qualitative checks.
///
/// The reviewer receives the configured prompt plus selected run context, then returns a score.
/// The assertion passes when the reviewer score is greater than or equal to `pass_threshold`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub struct ReviewAssertion {
    /// Reviewer instruction or rubric question.
    ///
    /// This should ask for a specific judgement, for example: "Did the agent satisfy the user's
    /// request without introducing unrelated changes?"
    pub prompt: String,

    /// Minimum reviewer score required to pass, usually between `0.0` and `1.0`.
    #[serde(default)]
    pub pass_threshold: Option<f64>,
}

impl Assertion {
    pub fn validate(&self) -> miette::Result<()> {
        match self {
            Assertion::Comparison { comparison: _ } => Ok(()),
            Assertion::Not { not } => not.validate(),
            Assertion::All { all } => all.iter().try_for_each(|x| x.validate()),
            Assertion::Any { any } => any.iter().try_for_each(|x| x.validate()),
            Assertion::Review { review } => {
                if let Some(threshold) = &review.pass_threshold
                    && (*threshold > 1.0 || *threshold < 0.0)
                {
                    bail!("review `pass_threshold` must be between 0 and 1");
                }
                Ok(())
            }
        }
    }
}
