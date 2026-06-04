// SPDX-License-Identifier: MIT
// Copyright (c) 2026 eunomia-bpf org.

use super::common::{AnalyzerProcessor, BinaryExecutor, parse_json_event};
use super::{EventStream, Runner, RunnerError};
use crate::framework::analyzers::Analyzer;
use async_trait::async_trait;
use futures::stream::StreamExt;
use std::path::Path;
use std::sync::{Arc, atomic::AtomicU64};

/// Runner for collecting SSL/TLS events
pub struct SslRunner {
    analyzers: Vec<Box<dyn Analyzer>>,
    executor: BinaryExecutor,
}

impl SslRunner {
    /// Create from binary extractor (real execution mode)
    pub fn from_binary_extractor(binary_path: impl AsRef<Path>) -> Self {
        let path_str = binary_path.as_ref().to_string_lossy().to_string();
        Self {
            analyzers: Vec::new(),
            executor: BinaryExecutor::new(path_str).with_runner_name("SSL".to_string()),
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
            .with_runner_name("SSL".to_string());
        self
    }
}

#[async_trait]
impl Runner for SslRunner {
    async fn run(&mut self) -> Result<EventStream, RunnerError> {
        // Get raw JSON stream from the binary executor
        let json_stream = self.executor.get_json_stream().await?;

        let errors = Arc::new(AtomicU64::new(0));
        let event_stream = json_stream
            .map(move |json_value| parse_json_event("ssl", "timestamp_ns", json_value, &errors));

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

    /// Test that actually runs the real SSL binary
    ///
    /// This test is ignored by default and only runs when specifically requested.
    /// To run this test: `cargo test test_ssl_runner_with_real_binary -- --ignored`
    ///
    /// Prerequisites:
    /// - The sslsniff binary must be built and available at ../src/sslsniff
    /// - Sufficient privileges to run eBPF programs (usually requires sudo)
    ///
    /// Note: This test may fail if:
    /// - The binary doesn't exist
    /// - Insufficient privileges
    /// - No SSL/TLS traffic occurs during the execution window
    #[tokio::test]
    #[ignore = "requires real binary and may need sudo privileges"]
    async fn test_ssl_runner_with_real_binary() {
        use std::path::Path;
        use std::time::Duration;
        use tokio::time::timeout;

        // Initialize debug logging for the test
        let _ = env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        let binary_path = "../src/sslsniff";

        // Check if binary exists before attempting to run
        if !Path::new(binary_path).exists() {
            return;
        }

        // Create runner with real binary
        let mut runner = SslRunner::from_binary_extractor(binary_path);

        // Run the binary and collect events for 30 seconds
        if let Ok(mut stream) = runner.run().await {
            let _ = timeout(Duration::from_secs(30), async {
                while futures::StreamExt::next(&mut stream).await.is_some() {}
            })
            .await;
        }
    }
}
