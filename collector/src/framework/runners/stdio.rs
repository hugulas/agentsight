// SPDX-License-Identifier: MIT
// Copyright (c) 2026 eunomia-bpf org.

use super::common::{AnalyzerProcessor, BinaryExecutor, parse_json_event};
use super::{EventStream, Runner, RunnerError};
use crate::framework::analyzers::Analyzer;
use async_trait::async_trait;
use futures::stream::StreamExt;
use std::path::Path;
use std::sync::{Arc, atomic::AtomicU64};

/// Runner for collecting stdio payload events
pub struct StdioRunner {
    analyzers: Vec<Box<dyn Analyzer>>,
    executor: BinaryExecutor,
}

impl StdioRunner {
    /// Create from binary extractor (real execution mode)
    pub fn from_binary_extractor(binary_path: impl AsRef<Path>) -> Self {
        let path_str = binary_path.as_ref().to_string_lossy().to_string();
        Self {
            analyzers: Vec::new(),
            executor: BinaryExecutor::new(path_str).with_runner_name("Stdio".to_string()),
        }
    }

    /// Add additional command-line arguments to pass to the binary
    pub fn with_args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let args = args
            .into_iter()
            .map(|s| s.as_ref().to_string())
            .collect::<Vec<_>>();
        self.executor = self
            .executor
            .with_args(&args)
            .with_runner_name("Stdio".to_string());
        self
    }
}

#[async_trait]
impl Runner for StdioRunner {
    async fn run(&mut self) -> Result<EventStream, RunnerError> {
        let json_stream = self.executor.get_json_stream().await?;

        let errors = Arc::new(AtomicU64::new(0));
        let event_stream = json_stream
            .map(move |json_value| parse_json_event("stdio", "timestamp_ns", json_value, &errors));

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
        let event = parse_json_event("stdio", "timestamp_ns", invalid, &errors);
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
        let event = parse_json_event("stdio", "timestamp_ns", valid, &errors);
        assert_eq!(event.source, "stdio");
        assert_eq!(event.pid, 1234);
        assert_eq!(event.comm, "python3");
        assert_eq!(event.timestamp, 1);
    }
}
