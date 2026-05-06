use std::path::Path;

use crate::types::{
    CommandOutput, OutputStream,
    test::{LlmAction, TestRun, ToolCall, ToolState},
};
use anyhow::anyhow;
use serde::de::DeserializeOwned;
use serde_json::Value;

#[derive(Default)]
pub struct Adaptor;

impl super::HarnessAdaptor for Adaptor {
    fn parse_result(&self, test_run: &mut TestRun, output: CommandOutput) -> anyhow::Result<()> {
        match output.stream {
            OutputStream::Stdout => {
                parse_stdout_line(test_run, output.line)?;
            }
            OutputStream::Stderr => {
                return Ok(());
            }
        }
        Ok(())
    }

    fn get_run_command(
        &self,
        system_prompt: Option<&str>,
        workspace: &Path,
        extra_args: &[&str],
        prompt: &str,
        is_review: bool,
    ) -> anyhow::Result<Vec<String>> {
        let mut cmd = vec![
            "pi".into(),
            "--mode".into(),
            "json".into(),
            "-p".into(),
            "--session-dir".into(),
            workspace
                .join(".evalt/sessions")
                .to_string_lossy()
                .to_string(),
            "--name".into(),
            if is_review {
                "review".into()
            } else {
                "session".into()
            },
        ];

        if let Some(system) = system_prompt {
            cmd.push("--system-prompt".into());
            cmd.push(system.into());
        }

        for arg in extra_args {
            cmd.push((*arg).into());
        }
        cmd.push(prompt.into());

        Ok(cmd)
    }
}

fn parse_stdout_line(test_run: &mut TestRun, line: String) -> anyhow::Result<()> {
    if line.trim().is_empty() {
        return Ok(());
    }

    let obj: Value = serde_json::from_str(&line)?;
    let event_type: String = pointer(&obj, "/type")?;

    if event_type != "message_update"
        && let Ok(message) = pointer::<Value>(&obj, "/message")
        && pointer::<String>(&message, "/role").ok().as_deref() == Some("assistant")
    {
        if pointer::<String>(&message, "/stopReason").ok().as_deref() == Some("error") {
            return Err(anyhow!(
                "pi assistant message failed: {}",
                pointer::<String>(&message, "/errorMessage")
                    .unwrap_or_else(|_| "unknown error".to_owned())
            ));
        }

        if let Ok(content) = pointer::<Vec<Value>>(&message, "/content") {
            for item in content {
                match pointer::<String>(&item, "/type").ok().as_deref() {
                    Some("text") => {
                        test_run.assistant_response = pointer(&item, "/text")?;
                        test_run.last_action = LlmAction::Responding;
                    }
                    Some("thinking") => {
                        test_run.thinking_response = pointer(&item, "/thinking")?;
                        test_run.last_action = LlmAction::Thinking;
                    }
                    Some("toolCall") => {
                        let id = pointer(&item, "/id")
                            .unwrap_or_else(|_| test_run.tool_calls.len().to_string());
                        if !test_run.tool_calls.iter().any(|call| call.id == id) {
                            test_run.tool_calls.push(ToolCall {
                                id,
                                name: pointer(&item, "/name")
                                    .unwrap_or_else(|_| "<unknown>".to_owned()),
                                args: pointer(&item, "/arguments").unwrap_or(Value::Null),
                                tool_state: ToolState::Running,
                            });
                        }
                        test_run.last_action = LlmAction::ToolCall;
                    }
                    _ => {}
                }
            }

            if let Ok(usage) = pointer::<Value>(&message, "/usage") {
                test_run.input_tokens += pointer::<u64>(&usage, "/input").unwrap_or(0);
                test_run.output_tokens += pointer::<u64>(&usage, "/output").unwrap_or(0);
                test_run.total_tokens += pointer::<u64>(&usage, "/totalTokens").unwrap_or(0);
                test_run.total_cost_micros += (pointer::<f64>(&usage, "/cost/total").unwrap_or(0.0)
                    * 1_000_000.0)
                    .round()
                    .max(0.0) as u64;
            }
        }
    }

    match event_type.as_str() {
        "turn_end" => test_run.turns += 1,
        "message_update" => match pointer::<String>(&obj, "/assistantMessageEvent/type")
            .ok()
            .as_deref()
        {
            Some("text_end") => {
                test_run.assistant_response = pointer(&obj, "/assistantMessageEvent/content")?
            }
            Some("thinking_delta") => test_run
                .thinking_response
                .push_str(&pointer::<String>(&obj, "/assistantMessageEvent/delta")?),
            Some("toolcall_end") => {
                let id = pointer(&obj, "/assistantMessageEvent/toolCall/id")
                    .unwrap_or_else(|_| test_run.tool_calls.len().to_string());
                if !test_run.tool_calls.iter().any(|call| call.id == id) {
                    test_run.tool_calls.push(ToolCall {
                        id,
                        name: pointer(&obj, "/assistantMessageEvent/toolCall/name")
                            .unwrap_or_else(|_| "<unknown>".to_owned()),
                        args: pointer(&obj, "/assistantMessageEvent/toolCall/arguments")
                            .unwrap_or(Value::Null),
                        tool_state: ToolState::Running,
                    });
                }
            }
            _ => {}
        },
        "tool_execution_end" => {
            if let Ok(id) = pointer::<String>(&obj, "/toolCallId")
                && let Some(tool_call) = test_run.tool_calls.iter_mut().find(|call| call.id == id)
            {
                tool_call.tool_state = ToolState::Finished {
                    output: pointer(&obj, "/result").unwrap_or(Value::Null),
                    duration_ms: None,
                    errored: pointer(&obj, "/isError").unwrap_or(false),
                };
            }
        }
        _ => {}
    };

    Ok(())
}

fn pointer<T>(obj: &Value, path: &str) -> anyhow::Result<T>
where
    T: DeserializeOwned,
{
    obj.pointer(path)
        .ok_or(anyhow!("Path {path} doesn't exist in object"))
        .and_then(|value| serde_json::from_value::<T>(value.clone()).map_err(anyhow::Error::from))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::test::ToolState;
    use serde_json::json;

    #[test]
    fn ignores_empty_stdout_line() {
        let mut test_run = TestRun::new();

        parse_stdout_line(&mut test_run, "   ".to_owned()).unwrap();

        assert_eq!(test_run.assistant_response, "");
        assert_eq!(test_run.thinking_response, "");
        assert_eq!(test_run.turns, 0);
        assert_eq!(test_run.tool_calls.len(), 0);
    }

    #[test]
    fn parses_text_end_into_assistant_response() {
        let mut test_run = TestRun::new();
        let line = json!({
            "type": "message_update",
            "assistantMessageEvent": {
                "type": "text_end",
                "content": "Hello from Pi"
            }
        })
        .to_string();

        parse_stdout_line(&mut test_run, line).unwrap();

        assert_eq!(test_run.assistant_response, "Hello from Pi");
    }

    #[test]
    fn appends_thinking_delta_into_thinking_response() {
        let mut test_run = TestRun::new();
        let first = json!({
            "type": "message_update",
            "assistantMessageEvent": { "type": "thinking_delta", "delta": "Think" }
        })
        .to_string();
        let second = json!({
            "type": "message_update",
            "assistantMessageEvent": { "type": "thinking_delta", "delta": "ing" }
        })
        .to_string();

        parse_stdout_line(&mut test_run, first).unwrap();
        parse_stdout_line(&mut test_run, second).unwrap();

        assert_eq!(test_run.thinking_response, "Thinking");
    }

    #[test]
    fn increments_turns_on_turn_end() {
        let mut test_run = TestRun::new();
        let line = json!({ "type": "turn_end" }).to_string();

        parse_stdout_line(&mut test_run, line).unwrap();

        assert_eq!(test_run.turns, 1);
    }

    #[test]
    fn errors_on_pi_assistant_error_message() {
        let mut test_run = TestRun::new();
        let line = json!({
            "type": "message_end",
            "message": {
                "role": "assistant",
                "content": [],
                "stopReason": "error",
                "errorMessage": "Connection error."
            }
        })
        .to_string();

        let err = parse_stdout_line(&mut test_run, line).unwrap_err();

        assert_eq!(
            err.to_string(),
            "pi assistant message failed: Connection error."
        );
    }

    #[test]
    fn parses_message_end_usage_totals() {
        let mut test_run = TestRun::new();
        let line = json!({
            "type": "message_end",
            "message": {
                "role": "assistant",
                "content": [{ "type": "text", "text": "Done" }],
                "usage": {
                    "input": 10,
                    "output": 5,
                    "totalTokens": 15,
                    "cost": { "total": 0.000123 }
                }
            }
        })
        .to_string();

        parse_stdout_line(&mut test_run, line).unwrap();

        assert_eq!(test_run.input_tokens, 10);
        assert_eq!(test_run.output_tokens, 5);
        assert_eq!(test_run.total_tokens, 15);
        assert_eq!(test_run.total_cost_micros, 123);
    }

    #[test]
    fn records_toolcall_end_as_running_tool_call() {
        let mut test_run = TestRun::new();
        let line = json!({
            "type": "message_update",
            "assistantMessageEvent": {
                "type": "toolcall_end",
                "toolCall": {
                    "id": "call_1",
                    "name": "bash",
                    "arguments": { "command": "pwd" }
                }
            }
        })
        .to_string();

        parse_stdout_line(&mut test_run, line).unwrap();

        assert_eq!(test_run.tool_calls.len(), 1);
        assert_eq!(test_run.tool_calls[0].id, "call_1");
        assert_eq!(test_run.tool_calls[0].name, "bash");
        assert_eq!(test_run.tool_calls[0].args, json!({ "command": "pwd" }));
        assert!(matches!(
            test_run.tool_calls[0].tool_state,
            ToolState::Running
        ));
    }

    #[test]
    fn marks_tool_execution_end_as_finished() {
        let mut test_run = TestRun::new();
        let tool_call = json!({
            "type": "message_update",
            "assistantMessageEvent": {
                "type": "toolcall_end",
                "toolCall": {
                    "id": "call_1",
                    "name": "bash",
                    "arguments": { "command": "pwd" }
                }
            }
        })
        .to_string();
        let tool_result = json!({
            "type": "tool_execution_end",
            "toolCallId": "call_1",
            "toolName": "bash",
            "result": { "content": [{ "type": "text", "text": "/workspace\n" }] },
            "isError": false
        })
        .to_string();

        parse_stdout_line(&mut test_run, tool_call).unwrap();
        parse_stdout_line(&mut test_run, tool_result).unwrap();

        match &test_run.tool_calls[0].tool_state {
            ToolState::Finished {
                output,
                duration_ms,
                errored,
            } => {
                assert_eq!(
                    output,
                    &json!({ "content": [{ "type": "text", "text": "/workspace\n" }] })
                );
                assert_eq!(*duration_ms, None);
                assert!(!errored);
            }
            ToolState::Running => panic!("expected finished tool call"),
        }
    }

    #[test]
    fn does_not_duplicate_same_tool_call_id() {
        let mut test_run = TestRun::new();
        let line = json!({
            "type": "message_update",
            "assistantMessageEvent": {
                "type": "toolcall_end",
                "toolCall": {
                    "id": "call_1",
                    "name": "bash",
                    "arguments": { "command": "pwd" }
                }
            }
        })
        .to_string();

        parse_stdout_line(&mut test_run, line.clone()).unwrap();
        parse_stdout_line(&mut test_run, line).unwrap();

        assert_eq!(test_run.tool_calls.len(), 1);
    }

    #[test]
    fn parses_multi_line_pi_sequence_into_test_run() {
        let mut test_run = TestRun::new();
        let lines = [
            json!({
                "type": "message_update",
                "assistantMessageEvent": { "type": "thinking_delta", "delta": "Inspecting " }
            })
            .to_string(),
            json!({
                "type": "message_update",
                "assistantMessageEvent": {
                    "type": "toolcall_end",
                    "toolCall": {
                        "id": "call_1",
                        "name": "bash",
                        "arguments": { "command": "pwd && ls", "timeout": 5 }
                    }
                }
            })
            .to_string(),
            json!({
                "type": "tool_execution_end",
                "toolCallId": "call_1",
                "toolName": "bash",
                "result": { "content": [{ "type": "text", "text": "/workspace\nREADME.md\n" }] },
                "isError": false
            })
            .to_string(),
            json!({
                "type": "message_update",
                "assistantMessageEvent": { "type": "text_end", "content": "Ran pwd and ls." }
            })
            .to_string(),
            json!({
                "type": "message_end",
                "message": {
                    "role": "assistant",
                    "content": [{ "type": "text", "text": "Ran pwd and ls." }],
                    "usage": {
                        "input": 20,
                        "output": 7,
                        "totalTokens": 27,
                        "cost": { "total": 0.000042 }
                    }
                }
            })
            .to_string(),
            json!({ "type": "turn_end" }).to_string(),
        ];

        for line in lines {
            parse_stdout_line(&mut test_run, line).unwrap();
        }

        assert_eq!(test_run.thinking_response, "Inspecting ");
        assert_eq!(test_run.assistant_response, "Ran pwd and ls.");
        assert_eq!(test_run.turns, 1);
        assert_eq!(test_run.input_tokens, 20);
        assert_eq!(test_run.output_tokens, 7);
        assert_eq!(test_run.total_tokens, 27);
        assert_eq!(test_run.total_cost_micros, 42);
        assert_eq!(test_run.tool_calls.len(), 1);
        assert_eq!(
            test_run.tool_calls[0].args,
            json!({ "command": "pwd && ls", "timeout": 5 })
        );
        assert!(matches!(
            test_run.tool_calls[0].tool_state,
            ToolState::Finished { .. }
        ));
    }
}
