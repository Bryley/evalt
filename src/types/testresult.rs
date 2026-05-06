use std::fmt::{Debug, Display};

use crossterm::style::Stylize;
use owo_colors::OwoColorize;
use serde::Serialize;

use crate::{
    types::yaml_spec::assertions::ReviewAssertion, utils::{fail, pass, render_rail_text},
};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum AssertionResult {
    Single {
        /// Human-readable label: the named assertion's explicit name, or a derived label.
        label: String,

        /// Whether this assertion passed.
        passed: bool,

        /// Human-readable description of what was expected (e.g. `contains "hello"`).
        expected_display: String,

        /// Human-readable description of what the run actually produced.
        actual_display: String,
    },
    All {
        /// All asserted children results.
        children: Vec<AssertionResult>,
    },
    Any {
        /// All asserted children results.
        children: Vec<AssertionResult>,
    },
    Not {
        /// The child of the not.
        child: Box<AssertionResult>,
    },
    Review {
        /// Human-readable label: the named assertion's explicit name, or a derived label.
        label: String,
        /// The output that was reviewed
        output: String,
        /// The score the reviewer came up with
        score: f64,
        /// The passing score that it needed to be above
        passing_score: f64,
        /// The comment/reason the reviewer gave with the score
        comment: String,
    },
    Error {
        /// The error that the assertion faced.
        error: String,
    },
}

impl AssertionResult {
    pub fn passed(&self) -> bool {
        match self {
            AssertionResult::Single { passed, .. } => *passed,
            AssertionResult::All { children } => {
                children.iter().all(|assertion| assertion.passed())
            }
            AssertionResult::Any { children } => {
                children.iter().any(|assertion| assertion.passed())
            }
            AssertionResult::Not { child } => !child.passed(),
            AssertionResult::Review {
                score,
                passing_score,
                ..
            } => score >= passing_score,
            AssertionResult::Error { .. } => false,
        }
    }

    pub fn label(&self) -> String {
        match self {
            AssertionResult::Single { label, .. } => label.clone(),
            AssertionResult::All { children } => {
                let inner = children
                    .iter()
                    .map(|res| res.label())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("all({inner})")
            }
            AssertionResult::Any { children } => {
                let inner = children
                    .iter()
                    .map(|res| res.label())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("any({inner})")
            }
            AssertionResult::Not { child } => {
                format!("not({})", child.label())
            }
            AssertionResult::Review { label, .. } => label.clone(),
            AssertionResult::Error { error } => error.clone(),
        }
    }
}

/// Result from an AI reviewer invocation.
#[derive(Debug, Clone)]
pub struct ReviewAssertionResult {
    /// The review assertion evaluated.
    pub assertion: ReviewAssertion,

    /// The score the AI reviewer gave (0.0 – 1.0).
    pub score: f64,

    /// Whether the score met the pass threshold.
    pub passed: bool,

    /// Reviewer rationale parsed from the response.
    pub rationale: String,

    /// Raw reviewer response for audit/debugging.
    pub raw_response: String,
}

// ── Display ───────────────────────────────────────────────────────────────────

pub struct PrettyDisplay<'a, T> {
    inner: &'a T,
    indent: usize,
    verbose: bool,
}

impl<'a, T> PrettyDisplay<'a, T> {
    pub fn new(data: &'a T) -> Self {
        Self {
            inner: data,
            indent: 0,
            // TODO add ability to change this
            verbose: false,
        }
    }

    pub fn with_indent(data: &'a T, indent: usize) -> Self {
        Self {
            inner: data,
            indent,
            verbose: false,
        }
    }
}

impl<'a> Display for PrettyDisplay<'a, AssertionResult> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pass_icon = if self.inner.passed() { pass() } else { fail() };
        let indent = "  ".repeat(self.indent);

        match &self.inner {
            AssertionResult::Single {
                label,
                expected_display,
                actual_display,
                ..
            } => {
                write!(f, "{indent}{pass_icon} {}", label.bold())?;
                if !self.verbose && self.inner.passed() {
                    return Ok(());
                }
                writeln!(f)?;
                write!(f, "{indent}  {}", "expected:".bold())?;

                if expected_display.contains('\n') {
                    writeln!(f)?;
                    render_rail_text(f, expected_display, self.indent)?;
                    writeln!(f)?;
                } else {
                    writeln!(f, " {expected_display}")?;
                }

                write!(f, "{indent}  {}", "actual:".bold())?;

                if actual_display.contains('\n') {
                    writeln!(f)?;
                    render_rail_text(f, actual_display, self.indent)?;
                } else {
                    writeln!(f, " {actual_display}")?;
                }
            }
            AssertionResult::All { children } => {
                writeln!(f, "{indent}{pass_icon} ALL:")?;
                for (index, child) in children.iter().enumerate() {
                    if index > 0 {
                        writeln!(f)?;
                    }
                    write!(f, "{}", PrettyDisplay::with_indent(child, self.indent + 1))?;
                }
            }
            AssertionResult::Any { children } => {
                writeln!(f, "{indent}{pass_icon} ANY:")?;
                for (index, child) in children.iter().enumerate() {
                    if index > 0 {
                        writeln!(f)?;
                    }
                    write!(f, "{}", PrettyDisplay::with_indent(child, self.indent + 1))?;
                }
            }
            AssertionResult::Not { child } => {
                writeln!(f, "{indent}{pass_icon} NOT:")?;
                write!(
                    f,
                    "{}",
                    PrettyDisplay::with_indent(&**child, self.indent + 1)
                )?;
            }
            AssertionResult::Review {
                label,
                output,
                score,
                passing_score,
                comment,
            } => {
                write!(f, "{indent}{pass_icon} {}", label.bold())?;
                if !self.verbose && self.inner.passed() {
                    return Ok(());
                }
                writeln!(f)?;
                writeln!(f, "{indent}  {}", "output:".bold())?;
                render_rail_text(f, output, self.indent)?;
                writeln!(f)?;

                writeln!(f, "{indent}  {} {score:.2}", "score:".bold())?;
                writeln!(
                    f,
                    "{indent}  {} {passing_score:.2}",
                    "passing_score:".bold()
                )?;
                writeln!(f, "{indent}  {}", "comment:".bold())?;
                render_rail_text(f, comment, self.indent)?;
            }
            AssertionResult::Error { error } => {
                writeln!(f, "{indent}{pass_icon} {}", "ERROR:".bold())?;
                write!(f, "{indent}  {error}")?;
            }
        }

        Ok(())
    }
}
