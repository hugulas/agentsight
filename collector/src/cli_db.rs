// SPDX-License-Identifier: MIT
// Copyright (c) 2026 eunomia-bpf org.

use crate::framework::{
    adapters::{builtin_adapters, run_sql_adapters},
    core::Event,
    runners::RunnerError,
    storage::{GenericProjector, SnapshotOptions, SqliteStore},
};
use clap::Subcommand;
use std::io::Write;

#[derive(Subcommand)]
pub(crate) enum AdapterCommand {
    /// List built-in SQL adapters
    List {
        /// Emit JSON output
        #[arg(long)]
        json: bool,
    },
    /// Run SQL adapters on an existing SQLite database
    Run {
        /// SQLite database path
        #[arg(long)]
        db: String,
        /// SQL adapter to run: auto, anthropic, claude-code, openclaw, gemini-cli
        #[arg(long, default_value = "auto")]
        adapter: String,
    },
}

pub(crate) fn configured_db_path(cli_value: &Option<String>) -> Option<String> {
    cli_value
        .clone()
        .or_else(|| std::env::var("AGENTSIGHT_DB_PATH").ok())
}

pub(crate) fn run_replay(
    input: &str,
    db: &str,
    adapter: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let content = std::fs::read_to_string(input)?;
    let mut store = SqliteStore::open(db)?;
    let mut projector = GenericProjector::new();
    let mut inserted = 0usize;

    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let event: Event = serde_json::from_str(trimmed)
            .map_err(|e| format!("failed to parse JSONL line {}: {}", idx + 1, e))?;
        store.insert_event(&event, &mut projector)?;
        inserted += 1;
    }

    if let Some(adapter) = adapter {
        run_sql_adapters(&mut store, adapter)?;
        println!(
            "Replayed {} events into {} and ran adapter '{}'",
            inserted, db, adapter
        );
    } else {
        println!(
            "Replayed {} events into {} without SQL adapters",
            inserted, db
        );
    }
    Ok(())
}

pub(crate) fn run_token_query(
    db: &str,
    group_by: &str,
    json: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let store = SqliteStore::open(db)?;
    let rows = store.token_summary(group_by)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        println!("Token usage grouped by {}", group_by);
        println!(
            "{:<32} {:>12} {:>12} {:>12} {:>12} {:>12} {:>8}",
            "group", "input", "output", "cache_new", "cache_read", "total", "calls"
        );
        for row in rows {
            println!(
                "{:<32} {:>12} {:>12} {:>12} {:>12} {:>12} {:>8}",
                truncate(&row.group, 32),
                row.input_tokens,
                row.output_tokens,
                row.cache_creation_tokens,
                row.cache_read_tokens,
                row.total_tokens,
                row.calls
            );
        }
    }
    Ok(())
}

pub(crate) fn run_audit_query(
    db: &str,
    audit_type: Option<&str>,
    limit: usize,
    json: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let store = SqliteStore::open(db)?;
    let rows = store.audit_rows(audit_type, limit)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        println!("Audit events");
        println!(
            "{:<15} {:<10} {:<8} {:<16} {:<10} {:<28} summary",
            "timestamp_ms", "type", "pid", "comm", "status", "target"
        );
        for row in rows {
            println!(
                "{:<15} {:<10} {:<8} {:<16} {:<10} {:<28} {}",
                row.timestamp_ms,
                row.audit_type,
                row.pid
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                truncate(row.comm.as_deref().unwrap_or("-"), 16),
                row.status.as_deref().unwrap_or("-"),
                truncate(row.target.as_deref().unwrap_or("-"), 28),
                row.summary.as_deref().unwrap_or("")
            );
        }
    }
    Ok(())
}

pub(crate) fn run_export(
    db: &str,
    output: &str,
    event_limit: usize,
    audit_limit: usize,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let store = SqliteStore::open(db)?;
    let snapshot = store.export_snapshot(SnapshotOptions {
        event_limit,
        audit_limit,
    })?;
    let json = serde_json::to_vec_pretty(&snapshot)?;
    if output == "-" {
        let mut stdout = std::io::stdout().lock();
        stdout.write_all(&json)?;
        stdout.write_all(b"\n")?;
    } else {
        std::fs::write(output, json)?;
        println!("Exported snapshot to {}", output);
    }
    Ok(())
}

pub(crate) fn run_adapters_command(
    parent_json: bool,
    command: &Option<AdapterCommand>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match command {
        Some(AdapterCommand::List { json }) => run_adapters_list(parent_json || *json),
        Some(AdapterCommand::Run { db, adapter }) => run_adapters_on_db(db, adapter),
        None => run_adapters_list(parent_json),
    }
}

pub(crate) fn run_capture_adapters(
    db_path: Option<&str>,
    adapter: Option<&str>,
) -> Result<(), RunnerError> {
    let Some(db_path) = db_path else {
        return Ok(());
    };
    let Some(adapter) = adapter else {
        return Ok(());
    };
    let mut store = SqliteStore::open(db_path).map_err(|e| {
        RunnerError::from(format!(
            "failed to open SQLite database '{}': {}",
            db_path, e
        ))
    })?;
    run_sql_adapters(&mut store, adapter).map_err(|e| {
        RunnerError::from(format!("failed to run SQL adapter '{}': {}", adapter, e))
    })?;
    println!("✓ SQL adapters projected: {} ({})", adapter, db_path);
    Ok(())
}

fn run_adapters_on_db(
    db: &str,
    adapter: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut store = SqliteStore::open(db)?;
    run_sql_adapters(&mut store, adapter)?;
    println!("Ran SQL adapter '{}' on {}", adapter, db);
    Ok(())
}

fn run_adapters_list(json: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let adapters = builtin_adapters();
    if json {
        let rows: Vec<_> = adapters
            .iter()
            .map(|a| {
                serde_json::json!({
                    "id": a.id,
                    "version": a.version,
                    "type": a.adapter_type,
                    "supports_detect": a.supports_detect(),
                    "sql_files": a.sql_files.iter().map(|(name, _)| *name).collect::<Vec<_>>()
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        println!("{:<16} {:<10} {:<8} detect", "id", "version", "type");
        for adapter in adapters {
            println!(
                "{:<16} {:<10} {:<8} {}",
                adapter.id,
                adapter.version,
                adapter.adapter_type,
                if adapter.supports_detect() {
                    "yes"
                } else {
                    "no"
                }
            );
        }
    }
    Ok(())
}

pub(crate) fn run_db_summary(
    db: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let store = SqliteStore::open(db)?;

    let snap = store.export_snapshot(SnapshotOptions {
        event_limit: 50_000,
        audit_limit: 50_000,
    })?;
    let s = &snap.summary;

    // Duration
    let duration_s = match (s.start_timestamp_ms, s.end_timestamp_ms) {
        (Some(start), Some(end)) if end > start => (end - start) as f64 / 1000.0,
        _ => 0.0,
    };

    // Token totals
    let total_calls: i64 = snap.token_summary.iter().map(|r| r.calls).sum();

    // Header
    if duration_s > 0.0 {
        println!(
            "{:.0}s session · {} API calls · {} tokens",
            duration_s, total_calls, s.total_tokens
        );
    } else {
        println!(
            "{} API calls · {} tokens",
            total_calls, s.total_tokens
        );
    }
    println!();

    // Models
    for row in &snap.token_summary {
        println!(
            "  {} — {} calls, {} tokens (in: {}, out: {})",
            row.group, row.calls, row.total_tokens, row.input_tokens, row.output_tokens
        );
    }
    println!();

    // Process analysis from audit events
    let mut exec_count = 0usize;
    let mut programs: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let mut git_subcommands: Vec<String> = Vec::new();
    let mut test_runs = 0usize;
    let mut git_commits = 0usize;

    for row in &snap.audit_events {
        if row.action.as_deref() != Some("exec") {
            continue;
        }
        exec_count += 1;
        let comm = row.comm.as_deref().unwrap_or("?");
        *programs.entry(comm.to_string()).or_default() += 1;

        // Detect specific activities from full_command or target
        let target = row.target.as_deref().unwrap_or("");
        let details = row.details.to_string();

        if comm == "git" {
            // Try to extract git subcommand from details or summary
            let summary = row.summary.as_deref().unwrap_or("");
            if summary.contains("commit") || details.contains("commit") {
                git_commits += 1;
            }
            // Store for display
            if let Some(sub) = summary.split("git ").nth(1) {
                let sub = sub.split_whitespace().next().unwrap_or("");
                if !sub.is_empty() {
                    git_subcommands.push(sub.to_string());
                }
            }
        }

        if matches!(comm, "pytest" | "cargo" | "npm" | "jest" | "make")
            && (target.contains("test") || details.contains("test"))
        {
            test_runs += 1;
        }
    }

    // Files accessed (from file audit events)
    let mut files_accessed: Vec<String> = Vec::new();
    for row in &snap.audit_events {
        if row.audit_type == "file" && row.target.as_ref().is_some_and(|t| !files_accessed.contains(t)) {
            files_accessed.push(row.target.clone().unwrap());
        }
    }

    // Network endpoints (from HTTP events in canonical events)
    let mut endpoints: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for event in &snap.events {
        if let Some(host) = &event.host {
            endpoints.insert(host.clone());
        }
    }

    // Print process summary
    if exec_count > 0 {
        let mut sorted: Vec<_> = programs.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        let top: Vec<String> = sorted
            .iter()
            .take(8)
            .map(|(name, count)| format!("{}({})", name, count))
            .collect();
        println!("{} processes spawned: {}", exec_count, top.join(", "));

        if git_commits > 0 {
            println!("{} git commits", git_commits);
        }
        if test_runs > 0 {
            println!("{} test runs", test_runs);
        }
    }

    if !files_accessed.is_empty() {
        let display: Vec<&str> = files_accessed.iter().take(5).map(|s| s.as_str()).collect();
        if files_accessed.len() > 5 {
            println!(
                "{} files accessed: {}, ... (+{} more)",
                files_accessed.len(),
                display.join(", "),
                files_accessed.len() - 5
            );
        } else {
            println!(
                "{} files accessed: {}",
                files_accessed.len(),
                display.join(", ")
            );
        }
    }

    if !endpoints.is_empty() {
        let eps: Vec<&str> = endpoints.iter().map(|s| s.as_str()).collect();
        println!("Network: {}", eps.join(", "));
    }

    println!();
    println!("  agentsight db audit --db {} for full details", db);
    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    if max <= 3 {
        return ".".repeat(max);
    }
    let mut out: String = s.chars().take(max - 3).collect();
    out.push_str("...");
    out
}
