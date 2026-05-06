use std::fmt::Display;

use crate::{
    types::yaml_spec::{
        assertions::{Assertion, ComparisonAssertion, StatusOptions},
        operations::{BoolOp, NumericOp, StringOp},
    }, utils::clip,
};

impl Display for Assertion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Assertion::Not { not } => {
                write!(f, "not({not})")?;
            }
            Assertion::All { all } => {
                write!(f, "all(")?;
                let mut iter = all.iter();
                let Some(item) = iter.next() else {
                    write!(f, ")")?;
                    return Ok(());
                };
                write!(f, "{item}")?;
                for item in iter {
                    write!(f, ", {item}")?;
                }
            }
            Assertion::Any { any } => {
                write!(f, "any(")?;
                let mut iter = any.iter();
                let Some(item) = iter.next() else {
                    write!(f, ")")?;
                    return Ok(());
                };
                write!(f, "{item}")?;
                for item in iter {
                    write!(f, ", {item}")?;
                }
            }
            Assertion::Comparison { comparison } => {
                write!(f, "{comparison}")?;
            }
            Assertion::Review { review } => {
                let prompt = clip(&review.prompt, 100);
                write!(f, "{prompt}")?;
            }
        }
        Ok(())
    }
}

impl Display for ComparisonAssertion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComparisonAssertion::DurationSecs { op } => {
                write!(f, "duration-secs {op}")?;
            }
            ComparisonAssertion::Turns { op } => {
                write!(f, "turns {op}")?;
            }
            ComparisonAssertion::TokensInput { op } => {
                write!(f, "tokens.input {op}")?;
            }
            ComparisonAssertion::TokensOutput { op } => {
                write!(f, "tokens.output {op}")?;
            }
            ComparisonAssertion::TokensThinking { op } => {
                write!(f, "tokens.thinking {op}")?;
            }
            ComparisonAssertion::TokensTotal { op } => {
                write!(f, "tokens.total {op}")?;
            }
            ComparisonAssertion::TokensCost { op } => {
                write!(f, "tokens.cost-usd {op}")?;
            }
            ComparisonAssertion::OutputThinking { op } => {
                write!(f, "output.thinking {op}")?;
            }
            ComparisonAssertion::OutputAssistant { op } => {
                write!(f, "output {op}")?;
            }
            ComparisonAssertion::ToolCallCount {
                tool_name,
                tool_params,
                op,
            } => {
                write!(f, "tool.calls")?;

                match (tool_name, tool_params) {
                    (Some(name), Some(params)) => {
                        write!(f, "({name}, {params})")?;
                    }
                    (Some(name), None) => {
                        write!(f, "({name})")?;
                    }
                    (None, Some(params)) => {
                        write!(f, "(*, {params})")?;
                    }
                    (None, None) => {}
                }

                write!(f, " {op}")?;
            }
            ComparisonAssertion::ToolCalled {
                tool_name,
                tool_params,
                op,
            } => {
                write!(f, "tool.called")?;

                match (tool_name, tool_params) {
                    (Some(name), Some(params)) => {
                        write!(f, "({name}, {params})")?;
                    }
                    (Some(name), None) => {
                        write!(f, "({name})")?;
                    }
                    (None, Some(params)) => {
                        write!(f, "(*, {params})")?;
                    }
                    (None, None) => {}
                }

                write!(f, " {op}")?;
            }
            ComparisonAssertion::FileExists { path, op } => {
                write!(f, "file.exists({}) {op}", path.display())?;
            }
            ComparisonAssertion::FileContent { path, op } => {
                write!(f, "file.content({}) {op}", path.display())?;
            }
        }

        Ok(())
    }
}

impl Display for StatusOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl<T: Display> Display for NumericOp<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NumericOp::Eq(t) => write!(f, "== {t}"),
            NumericOp::Ne(t) => write!(f, "!= {t}"),
            NumericOp::Lt(t) => write!(f, "< {t}"),
            NumericOp::Lte(t) => write!(f, "<= {t}"),
            NumericOp::Gt(t) => write!(f, "> {t}"),
            NumericOp::Gte(t) => write!(f, ">= {t}"),
        }
    }
}

impl Display for StringOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let val_display = |text: &str, case_sensitive: bool| {
            format!("{}'{text}'", if case_sensitive { "" } else { "~" })
        };

        match self {
            StringOp::Eq { right } => write!(f, "== '{right}'"),
            StringOp::Ne { right } => write!(f, "!= '{right}'"),
            StringOp::Contains {
                right,
                case_sensitive,
            } => write!(f, "contains {}", val_display(right, *case_sensitive)),
            StringOp::NotContains {
                right,
                case_sensitive,
            } => write!(f, "not contains {}", val_display(right, *case_sensitive)),
            StringOp::MatchesRegex { right } => write!(f, "regex '{right}'"),
            StringOp::StartsWith {
                right,
                case_sensitive,
            } => write!(f, "starts-with {}", val_display(right, *case_sensitive)),
            StringOp::EndsWith {
                right,
                case_sensitive,
            } => write!(f, "ends-with {}", val_display(right, *case_sensitive)),
        }
    }
}

impl Display for BoolOp<bool> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BoolOp::Eq(v) => {
                write!(f, "== {v}")
            }
            BoolOp::Ne(v) => write!(f, "!= {v}"),
        }
    }
}
