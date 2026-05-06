use std::{collections::BTreeMap, path::Path};

use regex::Regex;

use crate::types::yaml_spec::assertions::StatusOptions;

#[derive(Debug, Clone, Default)]
pub struct RunResults {
    /// Normalized run status, such as `passed`, `failed`, `error`, or `timeout`.
    pub status: StatusOptions,

    /// Exit code from the harness process.
    pub exit_code: i32,

    /// Final normalized assistant response text.
    pub output_text: String,

    /// Captured standard output from the harness process.
    pub stdout: String,

    /// Captured standard error from the harness process.
    pub stderr: String,

    /// Wall-clock duration of the harness process in milliseconds (if set yet).
    pub duration_ms: Option<u64>,

    /// Number of agent turns observed in the normalized transcript, when available.
    pub turns: Option<u64>,

    /// Input token count reported by the harness, when available.
    pub input_tokens: Option<u64>,

    /// Output token count reported by the harness, when available.
    pub output_tokens: Option<u64>,

    /// Thinking/reasoning token count reported by the harness, when available.
    pub thinking_tokens: Option<u64>,

    /// Total token count reported by the harness, when available.
    pub total_tokens: Option<u64>,

    /// Total cost in millionths of a US dollar, when available.
    pub total_cost_micros: Option<u64>,

    /// Normalized tool calls observed in the transcript, in call order.
    pub tool_calls: Vec<ToolCallResult>,

    /// Final file contents keyed by workspace-relative path.
    pub file_contents: BTreeMap<String, String>,

    /// Final file existence keyed by workspace-relative path.
    pub file_exists: BTreeMap<String, bool>,

    /// File changed state keyed by workspace-relative path.
    pub file_changed: BTreeMap<String, bool>,

    /// Workspace-relative paths changed by the run.
    pub changed_files: Vec<String>,

    /// Unified diff for all workspace changes, when collected.
    pub diff: Option<String>,

    /// Raw harness transcript or event log, when available.
    pub raw_transcript: Option<String>,

    /// Raw harness metadata as serialized JSON, when available.
    pub raw_metadata_json: Option<String>,

    /// Non-fatal warnings produced while parsing harness output.
    pub warnings: Vec<String>,

    /// Fatal parse or harness error message, if the run could not be normalized cleanly.
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ToolCallResult {
    /// Stable tool-call id from the harness, when available.
    pub id: Option<String>,

    /// Zero-based index in the normalized tool-call sequence.
    pub index: usize,

    /// Tool name, such as `bash`, `read`, `write`, or `edit`.
    pub name: String,

    /// Tool parameters serialized as JSON.
    pub args_json: String,

    /// Tool result serialized as JSON, when available.
    pub result_json: Option<String>,

    /// Human-readable text extracted from the tool result, when available.
    pub result_text: Option<String>,

    /// Whether the tool call failed.
    pub is_error: bool,

    /// Error message if the tool failed.
    pub error: Option<String>,

    /// Tool runtime in milliseconds, when available.
    pub duration_ms: Option<u64>,
}

impl RunResults {
    pub fn finalize(&mut self) {
        if self.exit_code == -1 {
            self.status = StatusOptions::Timeout;
        } else if self.error.is_some() {
            self.status = StatusOptions::Error;
        } else if self.event_count() == 0 && !self.stdout.trim().is_empty() {
            self.status = StatusOptions::Error;
            self.error = Some("harness output did not contain any parseable events".to_owned());
        } else if self.exit_code == 0 {
            self.status = StatusOptions::Passed;
        } else {
            self.status = StatusOptions::Failed;
            let stderr = self.stderr.trim();
            self.error = Some(if stderr.is_empty() {
                format!("harness exited with status {}", self.exit_code)
            } else {
                format!(
                    "harness exited with status {}: {}",
                    self.exit_code,
                    stderr.lines().collect::<Vec<_>>().join(" ")
                )
            });
        }
    }

    fn event_count(&self) -> u64 {
        self.raw_metadata_json
            .as_deref()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
            .and_then(|metadata| {
                metadata
                    .get("event_count")
                    .and_then(serde_json::Value::as_u64)
            })
            .unwrap_or(0)
    }

    pub fn tool_call_count(&self, tool_name: Option<&Regex>, tool_params: Option<&Regex>) -> u64 {
        self.tool_calls
            .iter()
            .filter(|tool_call| Self::tool_call_matches(tool_call, tool_name, tool_params))
            .count() as u64
    }

    pub fn tool_was_called(&self, tool_name: Option<&Regex>, tool_params: Option<&Regex>) -> bool {
        self.tool_calls
            .iter()
            .any(|tool_call| Self::tool_call_matches(tool_call, tool_name, tool_params))
    }

    pub fn changed_file_count(&self, scope: Option<&Path>) -> u64 {
        match scope {
            Some(path) => {
                let scope = path.to_string_lossy();
                let scope = scope.trim_end_matches('/');
                let scope_prefix = format!("{scope}/");

                self.changed_files
                    .iter()
                    .filter(|changed_file| {
                        changed_file.as_str() == scope || changed_file.starts_with(&scope_prefix)
                    })
                    .count() as u64
            }
            None => self.changed_files.len() as u64,
        }
    }

    fn tool_call_matches(
        tool_call: &ToolCallResult,
        tool_name: Option<&Regex>,
        tool_params: Option<&Regex>,
    ) -> bool {
        let tool_name_matches =
            tool_name.map_or_else(|| true, |regex| regex.is_match(&tool_call.name));
        let tool_params_matches =
            tool_params.map_or_else(|| true, |regex| regex.is_match(&tool_call.args_json));
        tool_name_matches && tool_params_matches
    }
}
