// SPDX-License-Identifier: MIT
// Copyright (c) 2026 eunomia-bpf org.

use super::common::{AnalyzerProcessor, BinaryExecutor, parse_error_event};
use super::{EventStream, Runner, RunnerError};
use crate::framework::analyzers::Analyzer;
use crate::framework::core::Event;
use async_trait::async_trait;
use futures::stream::StreamExt;
use std::path::Path;
use std::sync::{Arc, atomic::AtomicU64};

/// Runner for collecting stdio payload events
pub struct StdioRunner {
    analyzers: Vec<Box<dyn Analyzer>>,
    binary_path: String,
    additional_args: Vec<String>,
}

impl StdioRunner {
    /// Create from binary extractor (real execution mode)
    pub fn from_binary_extractor(binary_path: impl AsRef<Path>) -> Self {
        let path_str = binary_path.as_ref().to_string_lossy().to_string();
        Self {
            analyzers: Vec::new(),
            binary_path: path_str,
            additional_args: Vec::new(),
        }
    }

    /// Add additional command-line arguments to pass to the binary
    pub fn with_args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.additional_args = args.into_iter().map(|s| s.as_ref().to_string()).collect();
        self
    }

    fn parse_stdio_event(json_value: serde_json::Value, errors: &AtomicU64) -> Event {
        let Some(timestamp) = json_value.get("timestamp_ns").and_then(|v| v.as_u64()) else {
            return parse_error_event("stdio", json_value, "missing timestamp_ns", errors);
        };
        let Some(pid) = json_value
            .get("pid")
            .and_then(|v| v.as_u64())
            .map(|value| value as u32)
        else {
            return parse_error_event("stdio", json_value, "missing pid", errors);
        };
        let Some(comm) = json_value
            .get("comm")
            .and_then(|v| v.as_str())
            .map(str::to_string)
        else {
            return parse_error_event("stdio", json_value, "missing comm", errors);
        };

        Event::new_with_timestamp(timestamp, "stdio".to_string(), pid, comm, json_value)
    }
}

#[async_trait]
impl Runner for StdioRunner {
    async fn run(&mut self) -> Result<EventStream, RunnerError> {
        let executor = BinaryExecutor::new(self.binary_path.clone())
            .with_args(&self.additional_args)
            .with_runner_name("Stdio".to_string());
        let json_stream = executor.get_json_stream().await?;

        let errors = Arc::new(AtomicU64::new(0));
        let event_stream =
            json_stream.map(move |json_value| Self::parse_stdio_event(json_value, &errors));

        AnalyzerProcessor::process_through_analyzers(Box::pin(event_stream), &mut self.analyzers)
            .await
    }

    fn add_analyzer(mut self, analyzer: Box<dyn Analyzer>) -> Self {
        self.analyzers.push(analyzer);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stdio_runner_skips_invalid_events() {
        let invalid = serde_json::json!({
            "timestamp_ns": 1,
            "pid": 1234
        });

        let errors = AtomicU64::new(0);
        let event = StdioRunner::parse_stdio_event(invalid, &errors);
        assert_eq!(event.source, "diagnostic");
        assert_eq!(event.data["type"], "runner_parse_error");
    }

    #[test]
    fn test_stdio_runner_parses_valid_event() {
        let valid = serde_json::json!({
            "timestamp_ns": 1,
            "pid": 1234,
            "comm": "python3",
            "data": "hello"
        });

        let errors = AtomicU64::new(0);
        let event = StdioRunner::parse_stdio_event(valid, &errors);
        assert_eq!(event.source, "stdio");
        assert_eq!(event.pid, 1234);
        assert_eq!(event.comm, "python3");
        assert_eq!(event.timestamp, 1);
    }
}
