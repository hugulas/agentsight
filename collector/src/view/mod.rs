// SPDX-License-Identifier: MIT
// Copyright (c) 2026 eunomia-bpf org.

pub mod types;

use crate::framework::core::Event;
use crate::framework::storage::sqlite::StorageResult;
use crate::framework::storage::{SnapshotOptions, SqliteStore, ViewProjector, ViewUpdate};
use crate::view::types::{AuditRow, LlmCallRow, Snapshot, TokenSummary};
use std::collections::BTreeMap;
use std::path::Path;

pub(crate) struct MaterializedView {
    store: SqliteStore,
    projector: ViewProjector,
}

impl MaterializedView {
    pub(crate) fn open_sqlite(path: impl AsRef<Path>) -> StorageResult<Self> {
        Ok(Self {
            store: SqliteStore::open(path)?,
            projector: ViewProjector::new(),
        })
    }

    pub(crate) fn ingest_event(&mut self, event: &Event) -> StorageResult<()> {
        self.store.insert_event(event, &mut self.projector)?;
        Ok(())
    }

    pub(crate) fn ingest_update(&mut self, update: &ViewUpdate) -> StorageResult<()> {
        self.store.apply_view_update(update)
    }

    pub(crate) fn ingest_jsonl_file(&mut self, path: impl AsRef<Path>) -> StorageResult<usize> {
        let content = std::fs::read_to_string(path)?;
        let mut inserted = 0usize;
        for (idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(update) = serde_json::from_str::<ViewUpdate>(trimmed) {
                self.ingest_update(&update)?;
            } else {
                let event: Event = serde_json::from_str(trimmed)
                    .map_err(|e| format!("failed to parse JSONL line {}: {}", idx + 1, e))?;
                self.ingest_event(&event)?;
            }
            inserted += 1;
        }
        Ok(inserted)
    }

    pub(crate) fn export_snapshot(&self, options: SnapshotOptions) -> StorageResult<Snapshot> {
        self.store.export_snapshot(options)
    }

    pub(crate) fn token_summary(&self, group_by: &str) -> StorageResult<Vec<TokenSummary>> {
        self.store.token_summary(group_by)
    }

    pub(crate) fn audit_rows(
        &self,
        audit_type: Option<&str>,
        limit: usize,
    ) -> StorageResult<Vec<AuditRow>> {
        self.store.audit_rows(audit_type, limit)
    }

    pub(crate) fn llm_call_rows(&self, limit: usize) -> StorageResult<Vec<LlmCallRow>> {
        self.store.llm_call_rows(limit)
    }

    pub(crate) fn first_tool_timestamp_ms(&mut self) -> StorageResult<Option<u64>> {
        let timestamp: Option<i64> = self.store.connection_mut().query_row(
            "SELECT MIN(timestamp_ms) FROM tool_calls",
            [],
            |row| row.get(0),
        )?;
        Ok(timestamp.map(|value| value as u64))
    }

    pub(crate) fn tool_call_count(&mut self) -> StorageResult<i64> {
        Ok(self
            .store
            .connection_mut()
            .query_row("SELECT COUNT(*) FROM tool_calls", [], |row| row.get(0))?)
    }

    pub(crate) fn tool_counts(&mut self) -> StorageResult<BTreeMap<String, usize>> {
        let mut stmt = self.store.connection_mut().prepare(
            "SELECT COALESCE(tool_name, '?'), COUNT(*)
             FROM tool_calls
             GROUP BY COALESCE(tool_name, '?')
             ORDER BY COUNT(*) DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
        })?;
        let mut counts = BTreeMap::new();
        for row in rows {
            let (name, count) = row?;
            counts.insert(name, count);
        }
        Ok(counts)
    }

    pub(crate) fn tool_durations_ms(&mut self) -> StorageResult<Vec<u64>> {
        let mut stmt = self
            .store
            .connection_mut()
            .prepare("SELECT duration_ms FROM tool_calls WHERE duration_ms >= 0")?;
        let rows = stmt.query_map([], |row| row.get::<_, i64>(0))?;
        let mut durations = Vec::new();
        for row in rows {
            durations.push(row? as u64);
        }
        Ok(durations)
    }

    pub(crate) fn resource_samples(&mut self) -> StorageResult<Vec<(Option<f64>, Option<i64>)>> {
        let mut stmt = self
            .store
            .connection_mut()
            .prepare("SELECT cpu_percent, rss_mb FROM resource_samples")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, Option<f64>>(0)?, row.get::<_, Option<i64>>(1)?))
        })?;
        let mut samples = Vec::new();
        for row in rows {
            samples.push(row?);
        }
        Ok(samples)
    }
}
