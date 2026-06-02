// SPDX-License-Identifier: MIT
// Copyright (c) 2026 eunomia-bpf org.

use crate::framework::{
    core::Event,
    runners::RunnerError,
    storage::{GenericProjector, SqliteStore},
};
use serde_json::Value;
use std::fs::OpenOptions;
use std::io::Write;
use std::process::ExitStatus;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

pub const CLI_OUTPUT_CAPTURE_MAX_BYTES: usize = 8 * 1024 * 1024;

/// This gates optional stdout/stderr evidence capture, not process tracing.
/// Unknown commands still run and are traced at the OS boundary; their terminal
/// output is not stored unless AgentSight knows how to parse and redact it.
pub fn should_capture_cli_output(program: &str, args: &[String], db_path: Option<&str>) -> bool {
    if db_path.is_none() {
        return false;
    }

    let base = std::path::Path::new(program)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(program)
        .to_ascii_lowercase();
    let structured_cli_output_supported = matches!(
        base.as_str(),
        "claude" | "gemini" | "openclaw" | "opencode" | "codex"
    );
    if !structured_cli_output_supported {
        return false;
    }

    let headless = args.iter().any(|arg| {
        matches!(
            arg.as_str(),
            "-p" | "--print" | "--prompt" | "--local" | "--gateway"
        )
    }) || (base == "opencode"
        && args
            .iter()
            .any(|arg| matches!(arg.as_str(), "run" | "--command")))
        || (base == "codex"
            && args
                .iter()
                .any(|arg| matches!(arg.as_str(), "exec" | "e" | "review")));
    let structured_output = args.iter().enumerate().any(|(idx, arg)| {
        arg == "--json"
            || arg == "--output-format=json"
            || arg == "--output-format=stream-json"
            || arg == "--format=json"
            || (arg == "--output-format"
                && args
                    .get(idx + 1)
                    .map(|value| value == "json" || value == "stream-json")
                    .unwrap_or(false))
            || (arg == "--format"
                && args
                    .get(idx + 1)
                    .map(|value| value == "json")
                    .unwrap_or(false))
    });

    headless && structured_output
}

pub async fn tee_child_stream<R>(
    mut reader: R,
    stream_name: &'static str,
    max_capture_bytes: usize,
) -> std::io::Result<Vec<u8>>
where
    R: AsyncRead + Unpin,
{
    let mut captured = Vec::new();
    let mut buf = [0u8; 8192];

    loop {
        let n = reader.read(&mut buf).await?;
        if n == 0 {
            break;
        }

        if stream_name == "stderr" {
            let mut stderr = tokio::io::stderr();
            stderr.write_all(&buf[..n]).await?;
            stderr.flush().await?;
        } else {
            let mut stdout = tokio::io::stdout();
            stdout.write_all(&buf[..n]).await?;
            stdout.flush().await?;
        }

        let remaining = max_capture_bytes.saturating_sub(captured.len());
        if remaining > 0 {
            captured.extend_from_slice(&buf[..n.min(remaining)]);
        }
    }

    Ok(captured)
}

fn parse_cli_json(text: &str) -> Option<Value> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return Some(value);
    }

    let values: Vec<Value> = trimmed
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.starts_with('{') || line.starts_with('[') {
                serde_json::from_str::<Value>(line).ok()
            } else {
                None
            }
        })
        .collect();
    if values.len() == 1 {
        return values.into_iter().next();
    }
    if !values.is_empty() {
        return Some(Value::Array(values));
    }

    let json_start = trimmed
        .char_indices()
        .find(|(_, ch)| *ch == '{' || *ch == '[')
        .map(|(idx, _)| idx)?;
    let candidate = &trimmed[json_start..];
    let mut stream = serde_json::Deserializer::from_str(candidate).into_iter::<Value>();
    if let Some(Ok(value)) = stream.next() {
        return Some(value);
    }

    None
}

fn sanitize_gemini_cli_json(value: &Value) -> Option<Value> {
    let models = value
        .get("stats")
        .and_then(|stats| stats.get("models"))
        .and_then(|models| models.as_object())?;
    let sanitized_models = models
        .iter()
        .map(|(model, usage)| {
            let mut model_obj = serde_json::Map::new();
            if let Some(tokens) = usage.get("tokens") {
                model_obj.insert("tokens".to_string(), tokens.clone());
            }
            if let Some(api) = usage.get("api") {
                model_obj.insert("api".to_string(), api.clone());
            }
            (model.clone(), Value::Object(model_obj))
        })
        .collect::<serde_json::Map<_, _>>();

    Some(serde_json::json!({
        "stats": {
            "models": sanitized_models
        }
    }))
}

fn sanitize_claude_cli_json(value: &Value) -> Option<Value> {
    fn sanitize_result(value: &Value) -> Option<Value> {
        if value.get("type").and_then(|v| v.as_str()) != Some("result") {
            return None;
        }

        let mut out = serde_json::Map::new();
        out.insert("type".to_string(), Value::String("result".to_string()));
        if let Some(subtype) = value.get("subtype") {
            out.insert("subtype".to_string(), subtype.clone());
        }
        if let Some(model_usage) = value.get("modelUsage") {
            out.insert("modelUsage".to_string(), model_usage.clone());
        }
        if let Some(usage) = value.get("usage") {
            out.insert("usage".to_string(), usage.clone());
        }
        out.contains_key("modelUsage").then_some(Value::Object(out))
    }

    fn sanitize_tool_message(value: &Value) -> Option<Value> {
        if value.get("type").and_then(|v| v.as_str()) != Some("assistant") {
            return None;
        }
        let content = value
            .get("message")
            .and_then(|message| message.get("content"))
            .and_then(|content| content.as_array())?;
        let tool_uses: Vec<_> = content
            .iter()
            .filter(|block| block.get("type").and_then(|v| v.as_str()) == Some("tool_use"))
            .map(|block| {
                serde_json::json!({
                    "type": "tool_use",
                    "id": block.get("id"),
                    "name": block.get("name"),
                    "input_redacted": true
                })
            })
            .collect();
        if tool_uses.is_empty() {
            return None;
        }

        Some(serde_json::json!({
            "type": "assistant",
            "session_id": value.get("session_id"),
            "message": {
                "content": tool_uses
            }
        }))
    }

    match value {
        Value::Array(values) => {
            let results: Vec<_> = values
                .iter()
                .filter_map(|value| sanitize_result(value).or_else(|| sanitize_tool_message(value)))
                .collect();
            (!results.is_empty()).then_some(Value::Array(results))
        }
        Value::Object(_) => sanitize_result(value).or_else(|| sanitize_tool_message(value)),
        _ => None,
    }
}

fn sanitize_opencode_cli_json(value: &Value) -> Option<Value> {
    fn sanitize_event(value: &Value) -> Option<Value> {
        let event_type = value.get("type").and_then(|v| v.as_str())?;
        match event_type {
            "step_finish" => Some(serde_json::json!({
                "type": "step_finish",
                "timestamp": value.get("timestamp"),
                "sessionID": value.get("sessionID"),
                "part": {
                    "reason": value.pointer("/part/reason"),
                    "messageID": value.pointer("/part/messageID"),
                    "tokens": value.pointer("/part/tokens"),
                    "cost": value.pointer("/part/cost")
                }
            })),
            "tool_use" => Some(serde_json::json!({
                "type": "tool_use",
                "timestamp": value.get("timestamp"),
                "sessionID": value.get("sessionID"),
                "part": {
                    "tool": value.pointer("/part/tool"),
                    "callID": value.pointer("/part/callID"),
                    "messageID": value.pointer("/part/messageID"),
                    "state": {
                        "status": value.pointer("/part/state/status"),
                        "input_redacted": true,
                        "metadata": value.pointer("/part/state/metadata"),
                        "title": value.pointer("/part/state/title"),
                        "time": value.pointer("/part/state/time")
                    }
                }
            })),
            _ => None,
        }
    }

    match value {
        Value::Array(values) => {
            let events: Vec<_> = values.iter().filter_map(sanitize_event).collect();
            (!events.is_empty()).then_some(Value::Array(events))
        }
        Value::Object(_) => sanitize_event(value),
        _ => None,
    }
}

fn sanitize_codex_cli_json(value: &Value) -> Option<Value> {
    fn sanitize_event(value: &Value) -> Option<Value> {
        match value.get("type").and_then(|v| v.as_str())? {
            "thread.started" => Some(serde_json::json!({
                "type": "thread.started",
                "thread_id": value.get("thread_id")
            })),
            "turn.completed" => Some(serde_json::json!({
                "type": "turn.completed",
                "usage": value.get("usage")
            })),
            "item.completed" => {
                if value.pointer("/item/type").and_then(|v| v.as_str())
                    != Some("command_execution")
                {
                    return None;
                }
                Some(serde_json::json!({
                    "type": "item.completed",
                    "item": {
                        "id": value.pointer("/item/id"),
                        "type": "command_execution",
                        "status": value.pointer("/item/status"),
                        "exit_code": value.pointer("/item/exit_code"),
                        "command_redacted": true,
                        "output_redacted": true
                    }
                }))
            }
            _ => None,
        }
    }

    match value {
        Value::Array(values) => {
            let events: Vec<_> = values.iter().filter_map(sanitize_event).collect();
            (!events.is_empty()).then_some(Value::Array(events))
        }
        Value::Object(_) => sanitize_event(value),
        _ => None,
    }
}

fn sanitize_cli_parsed_json(program: &str, value: &Value) -> Option<Value> {
    let base = std::path::Path::new(program)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(program)
        .to_ascii_lowercase();

    match base.as_str() {
        "gemini" => sanitize_gemini_cli_json(value),
        "claude" => sanitize_claude_cli_json(value),
        "opencode" => sanitize_opencode_cli_json(value),
        "codex" => sanitize_codex_cli_json(value),
        _ => None,
    }
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn append_event_to_log(log_file: &str, event: &Event) -> Result<(), RunnerError> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file)
        .map_err(|e| RunnerError::from(format!("failed to append CLI output event: {}", e)))?;
    let line = event
        .to_json()
        .map_err(|e| RunnerError::from(format!("failed to serialize CLI output event: {}", e)))?;
    writeln!(file, "{}", line)
        .map_err(|e| RunnerError::from(format!("failed to write CLI output event: {}", e)))?;
    Ok(())
}

fn build_cli_output_event(
    timestamp_ms: u64,
    program: &str,
    args: &[String],
    pid: u32,
    comm: &str,
    stream_name: &str,
    bytes: &[u8],
    exit_status: Option<ExitStatus>,
) -> Option<Event> {
    if bytes.is_empty() {
        return None;
    }

    let text = String::from_utf8_lossy(bytes).to_string();
    let parsed_json =
        parse_cli_json(&text).and_then(|value| sanitize_cli_parsed_json(program, &value));
    let mut data = serde_json::json!({
        "event": "CLI_OUTPUT",
        "program": program,
        "arg_count": args.len(),
        "args_redacted": true,
        "stream": stream_name,
        "exit_code": exit_status.and_then(|status| status.code()),
        "success": exit_status.map(|status| status.success()).unwrap_or(false),
        "text_redacted": true,
        "captured_bytes": bytes.len(),
        "truncated": bytes.len() >= CLI_OUTPUT_CAPTURE_MAX_BYTES,
    });

    if let Some(parsed) = parsed_json
        && let Some(obj) = data.as_object_mut()
    {
        obj.insert("parsed_json".to_string(), parsed);
    }

    Some(Event::new_with_timestamp(
        timestamp_ms,
        "cli_output".to_string(),
        pid,
        comm.to_string(),
        data,
    ))
}

pub fn persist_cli_output_evidence(
    db_path: Option<&str>,
    log_file: &str,
    program: &str,
    args: &[String],
    pid: u32,
    comm: &str,
    exit_status: Option<ExitStatus>,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<(), RunnerError> {
    if stdout.is_empty() && stderr.is_empty() {
        return Ok(());
    }

    let mut events = Vec::new();
    let base_ts = now_unix_ms();
    if let Some(event) = build_cli_output_event(
        base_ts,
        program,
        args,
        pid,
        comm,
        "stdout",
        stdout,
        exit_status,
    ) {
        events.push(event);
    }
    if let Some(event) = build_cli_output_event(
        base_ts + 1,
        program,
        args,
        pid,
        comm,
        "stderr",
        stderr,
        exit_status,
    ) {
        events.push(event);
    }

    for event in &events {
        append_event_to_log(log_file, event)?;
    }

    let Some(db_path) = db_path else {
        return Ok(());
    };
    let mut store = SqliteStore::open(db_path).map_err(|e| {
        RunnerError::from(format!(
            "failed to open SQLite database '{}' for CLI output: {}",
            db_path, e
        ))
    })?;
    let mut projector = GenericProjector::new();
    for event in &events {
        store.insert_event(event, &mut projector).map_err(|e| {
            RunnerError::from(format!("failed to store CLI output evidence: {}", e))
        })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_after_cli_log_prefix() {
        let parsed = parse_cli_json("Loaded cached credentials.\n{\"stats\":{\"models\":{}}}")
            .expect("json should parse");
        assert!(parsed.get("stats").is_some());
    }

    #[test]
    fn captures_known_headless_agent_cli_only_with_db() {
        let args = vec![
            "-p".to_string(),
            "hi".to_string(),
            "--output-format".to_string(),
            "json".to_string(),
        ];
        assert!(should_capture_cli_output(
            "gemini",
            &args,
            Some("record.db")
        ));
        assert!(!should_capture_cli_output(
            "gemini",
            &["-p".to_string(), "hi".to_string()],
            Some("record.db")
        ));
        assert!(!should_capture_cli_output("gemini", &args, None));
        assert!(!should_capture_cli_output("vim", &args, Some("record.db")));
    }

    #[test]
    fn captures_opencode_run_json_output() {
        let args = vec![
            "run".to_string(),
            "--format".to_string(),
            "json".to_string(),
            "hi".to_string(),
        ];
        assert!(should_capture_cli_output(
            "opencode",
            &args,
            Some("record.db")
        ));
    }

    #[test]
    fn captures_codex_exec_json_output() {
        let args = vec![
            "exec".to_string(),
            "--json".to_string(),
            "hi".to_string(),
        ];
        assert!(should_capture_cli_output(
            "codex",
            &args,
            Some("record.db")
        ));
    }

    #[test]
    fn cli_output_event_redacts_text_and_prompt_args() {
        let event = build_cli_output_event(
            1,
            "gemini",
            &[
                "-p".to_string(),
                "secret prompt".to_string(),
                "--output-format".to_string(),
                "json".to_string(),
            ],
            42,
            "gemini",
            "stdout",
            br#"{"response":"secret answer","stats":{"models":{"gemini-test":{"tokens":{"input":1,"candidates":2,"total":3}}}}}"#,
            None,
        )
        .expect("event");

        let rendered = event.data.to_string();
        assert!(!rendered.contains("secret prompt"));
        assert!(!rendered.contains("secret answer"));
        assert_eq!(event.data["text_redacted"], true);
        assert_eq!(event.data["args_redacted"], true);
        assert_eq!(event.data["arg_count"], 4);
        assert_eq!(
            event.data["parsed_json"]["stats"]["models"]["gemini-test"]["tokens"]["total"],
            3
        );
    }

    #[test]
    fn claude_cli_output_keeps_tool_metadata_without_tool_input() {
        let event = build_cli_output_event(
            1,
            "claude",
            &[
                "-p".to_string(),
                "secret prompt".to_string(),
                "--output-format".to_string(),
                "json".to_string(),
            ],
            42,
            "claude",
            "stdout",
            br#"{"type":"assistant","session_id":"s1","message":{"content":[{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"cat secret.txt"}}]}}"#,
            None,
        )
        .expect("event");

        let rendered = event.data.to_string();
        assert!(rendered.contains("toolu_1"));
        assert!(rendered.contains("Bash"));
        assert!(!rendered.contains("secret.txt"));
        assert_eq!(
            event.data["parsed_json"]["message"]["content"][0]["input_redacted"],
            true
        );
    }

    #[test]
    fn opencode_cli_output_redacts_tool_content_but_keeps_metadata() {
        let event = build_cli_output_event(
            1,
            "opencode",
            &[
                "run".to_string(),
                "--format".to_string(),
                "json".to_string(),
            ],
            42,
            "opencode",
            "stdout",
            br#"{"type":"tool_use","timestamp":1780382677191,"sessionID":"s1","part":{"type":"tool","tool":"write","callID":"call_1","state":{"status":"completed","input":{"filePath":"/tmp/package-lock.json","content":"secret content"},"output":"Wrote file successfully.","metadata":{"filepath":"/tmp/package-lock.json","exists":false},"title":"tmp/package-lock.json","time":{"start":1,"end":2}},"messageID":"msg_1"}}"#,
            None,
        )
        .expect("event");

        let rendered = event.data.to_string();
        assert!(rendered.contains("/tmp/package-lock.json"));
        assert!(rendered.contains("input_redacted"));
        assert!(!rendered.contains("secret content"));
        assert_eq!(event.data["text_redacted"], true);
    }

    #[test]
    fn codex_cli_output_redacts_command_text_but_keeps_usage() {
        let event = build_cli_output_event(
            1,
            "codex",
            &[
                "exec".to_string(),
                "--json".to_string(),
                "secret prompt".to_string(),
            ],
            42,
            "codex",
            "stdout",
            br#"{"type":"item.completed","item":{"id":"item_0","type":"command_execution","command":"/bin/bash -lc 'cat secret.txt'","aggregated_output":"secret","exit_code":0,"status":"completed"}}
{"type":"turn.completed","usage":{"input_tokens":10,"cached_input_tokens":4,"output_tokens":2,"reasoning_output_tokens":1}}"#,
            None,
        )
        .expect("event");

        let rendered = event.data.to_string();
        assert!(rendered.contains("command_redacted"));
        assert!(rendered.contains("input_tokens"));
        assert!(!rendered.contains("secret.txt"));
        assert!(!rendered.contains("secret prompt"));
        assert_eq!(
            event.data["parsed_json"][1]["usage"]["input_tokens"],
            10
        );
    }

    #[test]
    fn parses_ndjson_stream_as_array() {
        let parsed = parse_cli_json(
            "{\"type\":\"system\"}\n{\"type\":\"result\",\"modelUsage\":{\"claude-test\":{\"inputTokens\":1}}}",
        )
        .expect("stream json should parse");
        assert!(parsed.is_array());
        assert_eq!(parsed.as_array().unwrap().len(), 2);
    }
}
