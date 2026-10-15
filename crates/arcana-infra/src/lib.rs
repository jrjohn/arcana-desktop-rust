//! Service implementations, security, settings, localization and logging
//! (C# `Arcana.Infrastructure`).
//!
//! M4 additions: [`network::ProbeNetworkMonitor`] (portable replacement for C#
//! `NetworkMonitorImpl`) and [`seed`] (C# `App.SeedSampleDataAsync`).
//!
//! Not ported here:
//! - Everything touching `Arcana.Plugins` (`AddPluginServices`: message bus, plugin manager,
//!   manifest/lazy loading, health monitor) — milestone 5.
//! - The DI container itself: [`wiring::ArcanaServices`] builds the same object graph by hand.

pub mod locale;
pub mod localization;
pub mod logging;
pub mod network;
pub mod plugins;
pub mod security;
pub mod seed;
pub mod services;
pub mod settings;
pub mod wiring;

/// Core types the composition root needs without depending on `arcana-core` directly.
pub use arcana_core::{AppError, AppResult, ErrorCode, PageRequest};
pub use localization::{
    CultureChanged, CultureChangedHandler, LocalizationService, LocalizationServiceImpl,
    format_template,
};
pub use services::{CustomerServiceImpl, OrderServiceImpl, ProductServiceImpl, SharedValidator};
pub use settings::{
    AppSettings, AppSettingsService, JsonSettingsService, SettingChanged, SettingsService,
    SettingsServiceExt, setting_keys,
};
