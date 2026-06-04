// SPDX-License-Identifier: MIT
// Copyright (c) 2026 eunomia-bpf org.

use crate::model::ViewResult;
use crate::sinks::sqlite::SqliteStore;
use crate::view::MaterializedView;
use std::path::Path;

pub(crate) fn load_view(path: impl AsRef<Path>) -> ViewResult<MaterializedView> {
    let store = SqliteStore::open_readonly(path)?;
    let mut view = MaterializedView::new();
    view.set_source("sqlite");

    for row in store.all_llm_call_rows()? {
        view.apply_llm_call(&row);
    }
    for row in store.token_usage_rows()? {
        view.apply_token_usage(&row);
    }
    for row in store.all_audit_event_rows()? {
        view.apply_audit_event(&row);
    }
    for row in store.process_node_rows()? {
        view.upsert_process_node(&row);
    }
    for row in store.tool_call_rows()? {
        view.apply_tool_call(&row);
    }
    for row in store.network_target_rows()? {
        view.upsert_network_target(&row);
    }
    for row in store.resource_sample_rows()? {
        view.apply_resource_sample(&row);
    }

    Ok(view)
}
