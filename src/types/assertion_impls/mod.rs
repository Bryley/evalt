pub mod display;

use std::fs;

use anyhow::anyhow;
use futures::future::{FutureExt, LocalBoxFuture};
use regex::Regex;

use crate::{
    engine::Engine,
    types::{
        config::stack::Stack,
        test::{Test, TestState, ToolCall},
        testresult::AssertionResult,
        yaml_spec::assertions::{Assertion, ComparisonAssertion, ReviewAssertion},
    },
};

impl Assertion {
    pub fn to_assertion_results<'a>(
        &'a self,
        stack: &'a Stack,
        engine: &'a Engine,
        test: &'a Test,
    ) -> LocalBoxFuture<'a, miette::Result<AssertionResult>> {
        async move {
            let res = match self {
                Assertion::Comparison { comparison } => {
                    match comparison.to_assertion_result(test) {
                        Ok(x) => x,
                        Err(err) => AssertionResult::Error {
                            error: err.to_string(),
                        },
                    }
                }
                Assertion::Not { not } => {
                    let assertion_result = not.to_assertion_results(stack, engine, test).await?;
                    AssertionResult::Not {
                        child: Box::new(assertion_result),
                    }
                }
                Assertion::All { all } => {
                    let children = all
                        .iter()
                        .map(|assertion| assertion.to_assertion_results(stack, engine, test))
                        .collect::<Vec<_>>();

                    let children = futures::future::try_join_all(children).await?;
                    AssertionResult::All { children }
                }
                Assertion::Any { any } => {
                    let children = any
                        .iter()
                        .map(|assertion| assertion.to_assertion_results(stack, engine, test))
                        .collect::<Vec<_>>();
                    let children = futures::future::try_join_all(children).await?;
                    AssertionResult::Any { children }
                }
                Assertion::Review { review } => {
                    review.to_assertion_result(stack, engine, test).await?
                }
            };

            Ok(res)
        }
        .boxed_local()
    }
}

macro_rules! create_assertion_result {
    ($self:expr, $op:expr, $value:expr) => {{
        AssertionResult::Single {
            label: $self.to_string(),
            passed: $op.compare($value),
            expected_display: $op.value_display(),
            actual_display: $value.to_string(),
        }
    }};
}

impl ComparisonAssertion {
    pub fn to_assertion_result(&self, test: &Test) -> anyhow::Result<AssertionResult> {
        let result = match self {
            ComparisonAssertion::DurationSecs { op } => {
                let value = if let TestState::Reviewing { run_duration, .. } = test.state {
                    run_duration.as_secs_f64()
                } else {
                    return Err(anyhow!("duration not available"));
                };
                create_assertion_result!(self, op, value)
            }
            ComparisonAssertion::Turns { op } => {
                let value = test.run.turns;
                create_assertion_result!(self, op, value)
            }
            ComparisonAssertion::TokensInput { op } => {
                let value = test.run.input_tokens;
                create_assertion_result!(self, op, value)
            }
            ComparisonAssertion::TokensOutput { op } => {
                let value = test.run.output_tokens;
                create_assertion_result!(self, op, value)
            }
            ComparisonAssertion::TokensThinking { op } => {
                let value = test.run.thinking_tokens;
                create_assertion_result!(self, op, value)
            }
            ComparisonAssertion::TokensTotal { op } => {
                let value = test.run.total_tokens;
                create_assertion_result!(self, op, value)
            }
            ComparisonAssertion::TokensCost { op } => {
                let value = test.run.total_cost_micros as f64 / 1_000_000.0;
                create_assertion_result!(self, op, value)
            }
            ComparisonAssertion::OutputThinking { op } => {
                let value = &test.run.thinking_response;
                create_assertion_result!(self, op, value)
            }
            ComparisonAssertion::OutputAssistant { op } => {
                let value = &test.run.assistant_response;
                create_assertion_result!(self, op, value)
            }
            ComparisonAssertion::ToolCallCount {
                tool_name,
                tool_params,
                op,
            } => {
                let value = test
                    .run
                    .tool_calls
                    .iter()
                    .filter(|tool| {
                        tool_call_matches(tool, tool_name.as_ref(), tool_params.as_ref())
                    })
                    .count() as u64;
                create_assertion_result!(self, op, value)
            }
            ComparisonAssertion::ToolCalled {
                tool_name,
                tool_params,
                op,
            } => {
                let value =
                    test.run.tool_calls.iter().any(|tool| {
                        tool_call_matches(tool, tool_name.as_ref(), tool_params.as_ref())
                    });
                create_assertion_result!(self, op, value)
            }
            ComparisonAssertion::FileExists { path, op } => {
                let value = test
                    .workspace
                    .as_ref()
                    .map(|workspace| workspace.join(path).exists())
                    .ok_or(anyhow!("workspace does not exist"))?;
                create_assertion_result!(self, op, value)
            }
            ComparisonAssertion::FileContent { path, op } => {
                let Some(workspace) = test.workspace.as_ref() else {
                    return Err(anyhow!("workspace does not exist"));
                };
                let full_path = workspace.join(path);
                if !full_path.is_file() {
                    return Ok(AssertionResult::Single {
                        label: self.to_string(),
                        passed: false,
                        expected_display: format!("file {} to exist", path.display()),
                        actual_display: format!("file {} does not exist", path.display()),
                    });
                }

                let value = fs::read_to_string(full_path)?;
                create_assertion_result!(self, op, value.as_str())
            }
        };

        Ok(result)
    }
}

impl ReviewAssertion {
    pub async fn to_assertion_result(
        &self,
        stack: &Stack,
        engine: &Engine,
        test: &Test,
    ) -> miette::Result<AssertionResult> {
        engine.review(stack, self, test).await
    }
}

fn tool_call_matches(
    tool_call: &ToolCall,
    tool_name: Option<&Regex>,
    tool_params: Option<&Regex>,
) -> bool {
    let tool_name_matches = tool_name.map_or_else(|| true, |regex| regex.is_match(&tool_call.name));
    let raw_args = serde_json::to_string(&tool_call.args).expect("args must be JSON serializable");
    let tool_params_matches = tool_params.map_or_else(|| true, |regex| regex.is_match(&raw_args));
    tool_name_matches && tool_params_matches
}
