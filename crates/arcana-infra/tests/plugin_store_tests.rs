//! C# `PluginVersionRepositoryTests` (23 + 3 Rust additions) against the `SQLite` store —
//! the same suite as `arcana-plugin-host/tests/version_store_tests.rs`.

#[path = "../../arcana-plugin-host/tests/support/version_store_suite.rs"]
mod suite;

use std::sync::Arc;

use arcana_data::Db;
use arcana_infra::plugin_store::SqlitePluginVersionStore;

#[allow(clippy::unwrap_used)]
async fn store() -> Arc<SqlitePluginVersionStore> {
    Arc::new(SqlitePluginVersionStore::new(
        Db::in_memory().await.unwrap(),
    ))
}

all_cases!(run_suite, store().await);
