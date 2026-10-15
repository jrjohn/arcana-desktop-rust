//! C# `PluginVersionRepositoryTests` (23 + 3 Rust additions) against the in-memory store.
//! The same suite runs against `SQLite` in `arcana-infra/tests/plugin_store_tests.rs`.

#[path = "support/version_store_suite.rs"]
mod suite;

use std::sync::Arc;

use arcana_plugin_host::version_store::InMemoryPluginVersionStore;

all_cases!(run_suite, Arc::new(InMemoryPluginVersionStore::new()));
