//! The services the UI talks to, cloned into async tasks.

use std::sync::Arc;

use arcana_core::network::NetworkMonitor;
use arcana_domain::services::{CustomerService, OrderService, ProductService};
use arcana_infra::wiring::ArcanaServices;
use arcana_infra::{AppSettingsService, LocalizationService, SettingsService};

/// Everything a page task may call. Cheap to clone (all `Arc`s).
#[derive(Clone)]
pub struct UiServices {
    pub orders: Arc<dyn OrderService>,
    pub customers: Arc<dyn CustomerService>,
    pub products: Arc<dyn ProductService>,
    pub localization: Arc<dyn LocalizationService>,
    pub app_settings: Arc<AppSettingsService>,
    pub settings: Arc<dyn SettingsService>,
    pub network: Arc<dyn NetworkMonitor>,
}

impl std::fmt::Debug for UiServices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("UiServices")
    }
}

impl UiServices {
    /// Take the UI-facing services out of the wired graph.
    #[must_use]
    pub fn from_arcana(s: &ArcanaServices, network: Arc<dyn NetworkMonitor>) -> Self {
        Self {
            orders: s.orders.clone(),
            customers: s.customers.clone(),
            products: s.products.clone(),
            localization: s.localization.clone(),
            app_settings: s.app_settings.clone(),
            settings: s.settings.clone(),
            network,
        }
    }
}
