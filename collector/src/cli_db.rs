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
    db: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // If no DB, try reading local Claude/Codex sessions directly
    if db.is_none() && let Some((source, file, data)) = read_latest_local_session() {
        print_local_summary(&source, &file, &data);
        return Ok(());
    }
    let db = db.ok_or("No session data found. Run `agentsight exec` first, or pass --db.")?;
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

// ---------------------------------------------------------------------------
// Local agent session reader (reads ~/.claude and ~/.codex JSONL directly)
// ---------------------------------------------------------------------------

use std::collections::BTreeMap;

fn local_session_dirs() -> Vec<(&'static str, std::path::PathBuf)> {
    let home = dirs::home_dir().unwrap_or_default();
    [("claude", home.join(".claude/projects")), ("codex", home.join(".codex/sessions"))]
        .into_iter()
        .filter(|(_, d)| d.is_dir())
        .collect()
}

fn walk_jsonl(dir: &std::path::Path, f: &mut dyn FnMut(&std::path::Path, &std::fs::Metadata)) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_jsonl(&path, f);
        } else if path.extension().is_some_and(|e| e == "jsonl")
            && let Ok(meta) = path.metadata() {
            f(&path, &meta);
        }
    }
}

pub(crate) fn count_local_sessions() -> Vec<(&'static str, std::path::PathBuf, usize, u64)> {
    local_session_dirs().into_iter().filter_map(|(name, dir)| {
        let (mut count, mut bytes) = (0usize, 0u64);
        walk_jsonl(&dir, &mut |_, meta| { count += 1; bytes += meta.len(); });
        (count > 0).then_some((name, dir, count, bytes))
    }).collect()
}

fn read_latest_local_session() -> Option<(String, String, serde_json::Value)> {
    for (name, dir) in local_session_dirs() {
        let mut best: Option<(std::time::SystemTime, std::path::PathBuf)> = None;
        walk_jsonl(&dir, &mut |path, meta| {
            let mtime = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
            if best.as_ref().is_none_or(|(t, _)| mtime > *t) {
                best = Some((mtime, path.to_path_buf()));
            }
        });
        let path = best?.1;
        let content = std::fs::read_to_string(&path).ok()?;
        let parsed = parse_local_session(name, &content);
        if !parsed.is_null() {
            return Some((name.into(), path.display().to_string(), parsed));
        }
    }
    None
}

fn json_u64(v: &serde_json::Value, key: &str) -> u64 {
    v.get(key).and_then(|v| v.as_u64()).unwrap_or(0)
}

fn parse_local_session(source: &str, content: &str) -> serde_json::Value {
    let mut models: BTreeMap<String, (u64, u64, u64)> = BTreeMap::new();
    let mut tools: BTreeMap<String, usize> = BTreeMap::new();
    let mut duration_ms = 0u64;
    let mut num_turns = 0u64;
    let mut cost_usd = 0.0f64;
    let mut codex_model = String::new();

    for line in content.lines() {
        let Ok(obj) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        let typ = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match (source, typ) {
            ("claude", "result") => {
                duration_ms = json_u64(&obj, "duration_ms");
                num_turns = json_u64(&obj, "num_turns");
                cost_usd = obj.get("total_cost_usd").and_then(|v| v.as_f64()).unwrap_or(0.0);
                if let Some(mu) = obj.get("modelUsage").and_then(|v| v.as_object()) {
                    for (m, u) in mu {
                        let inp = json_u64(u, "inputTokens");
                        let out = json_u64(u, "outputTokens");
                        let total = inp + out + json_u64(u, "cacheReadInputTokens") + json_u64(u, "cacheCreationInputTokens");
                        models.insert(m.clone(), (inp, out, total));
                    }
                }
            }
            ("claude", "assistant") => {
                if let Some(items) = obj.pointer("/message/content").and_then(|v| v.as_array()) {
                    for item in items {
                        if item.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                            let n = item.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                            *tools.entry(n.into()).or_default() += 1;
                        }
                    }
                }
            }
            ("codex", "turn_context") => {
                if let Some(m) = obj.pointer("/payload/model").and_then(|v| v.as_str()) {
                    codex_model = m.into();
                }
            }
            ("codex", "event_msg") => {
                if obj.pointer("/payload/type").and_then(|v| v.as_str()) == Some("token_count")
                    && let Some(u) = obj.pointer("/payload/info/total_token_usage")
                {
                    let key = if codex_model.is_empty() { "unknown" } else { &codex_model };
                    models.insert(key.into(), (json_u64(u, "input_tokens"), json_u64(u, "output_tokens"), json_u64(u, "total_tokens")));
                }
            }
            ("codex", "response_item") => {
                if obj.pointer("/payload/type").and_then(|v| v.as_str()) == Some("function_call") {
                    let n = obj.pointer("/payload/name").and_then(|v| v.as_str()).unwrap_or("?");
                    *tools.entry(n.into()).or_default() += 1;
                }
            }
            _ => {}
        }
    }

    if models.is_empty() { return serde_json::Value::Null; }
    serde_json::json!({ "models": models, "tools": tools, "duration_ms": duration_ms, "num_turns": num_turns, "cost_usd": cost_usd })
}

fn print_local_summary(source: &str, file: &str, data: &serde_json::Value) {
    let models = data.get("models").and_then(|v| v.as_object()).unwrap();
    let total_tokens: u64 = models.values().map(|v| v.get(2).and_then(|v| v.as_u64()).unwrap_or(0)).sum();

    print!("Latest {} session", source);
    let ms = json_u64(data, "duration_ms");
    if ms > 0 { print!(" · {:.0}s", ms as f64 / 1000.0); }
    let turns = json_u64(data, "num_turns");
    if turns > 0 { print!(" · {} turns", turns); }
    let cost = data.get("cost_usd").and_then(|v| v.as_f64()).unwrap_or(0.0);
    if cost > 0.0 { print!(" · ${:.2}", cost); }
    println!(" · {} tokens", total_tokens);
    println!();

    for (model, usage) in models {
        let (inp, out, total) = (json_u64(usage, "0"), json_u64(usage, "1"), json_u64(usage, "2"));
        println!("  {} — {} tokens (in: {}, out: {})", model, total, inp, out);
    }

    if let Some(tools) = data.get("tools").and_then(|v| v.as_object())
        && !tools.is_empty()
    {
        let total_calls: u64 = tools.values().map(|v| v.as_u64().unwrap_or(0)).sum();
        let mut sorted: Vec<_> = tools.iter().collect();
        sorted.sort_by(|a, b| b.1.as_u64().cmp(&a.1.as_u64()));
        let top: Vec<String> = sorted.iter().take(8).map(|(n, c)| format!("{}({})", n, c)).collect();
        println!("\n{} tool calls: {}", total_calls, top.join(", "));
    }
    println!("\n  Source: {}", file);
}
