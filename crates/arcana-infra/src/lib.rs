//! Service implementations, security, settings, localization and logging
//! (C# `Arcana.Infrastructure`).
//!
//! Not ported here:
//! - Everything touching `Arcana.Plugins` (`AddPluginServices`: message bus, plugin manager,
//!   manifest/lazy loading, health monitor) — milestone 5.
//! - `NetworkMonitorImpl` (wraps .NET `NetworkInterface` change events; there is no portable
//!   std equivalent) — the `arcana_core::network::NetworkMonitor` contract stays; the
//!   implementation is decided with the UI shell in milestone 4.
//! - The DI container itself: [`wiring::ArcanaServices`] builds the same object graph by hand.

pub mod locale;
pub mod localization;
pub mod logging;
pub mod security;
pub mod services;
pub mod settings;
pub mod wiring;

pub use localization::{
    CultureChanged, CultureChangedHandler, LocalizationService, LocalizationServiceImpl,
    format_template,
};
pub use services::{CustomerServiceImpl, OrderServiceImpl, ProductServiceImpl, SharedValidator};
pub use settings::{
    AppSettings, AppSettingsService, JsonSettingsService, SettingChanged, SettingsService,
    SettingsServiceExt, setting_keys,
};
