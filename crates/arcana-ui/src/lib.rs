//! egui shell, views and view models (C# `Arcana.App`).
//!
//! MVVM-UDF: each page has a view model with `on(Input) -> Vec<Effect>` that never touches
//! egui or I/O ([`pages`]); its view only reads state and returns inputs; async work runs on
//! a tokio runtime and comes back as inputs ([`runtime`]). The [`shell::Shell`] hosts pages
//! in dockable tabs and routes everything through the [`commands`] registry and the
//! [`nav::NavGraph`].

pub mod commands;
pub mod effect;
pub mod fmt;
pub mod fonts;
pub mod l10n;
pub mod nav;
pub mod pages;
pub mod plugin_bridge;
pub mod plugin_menu;
pub mod runtime;
pub mod services;
pub mod shell;
pub mod shell_vm;
pub mod theme;
pub mod widgets;

pub use services::UiServices;
pub use shell::{Shell, TabId};
