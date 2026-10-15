//! Plugin runtime (C# `Arcana.Plugins`).
//!
//! Instance-scoped: every registry lives inside a [`PluginHost`]-owned value, nothing is a
//! global static except the compile-time [`builtin`] registration list (`inventory`).

pub mod builtin;
pub mod dependency;
pub mod health;
pub mod permission;
pub mod services;
pub mod version;
pub mod version_store;
