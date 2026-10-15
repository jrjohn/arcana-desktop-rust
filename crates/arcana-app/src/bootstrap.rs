//! Composition root (C# `App.OnLaunched` + `ConfigureServices`): database in the app data
//! directory, wired services, demo data on first run, network monitor.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use arcana_infra::network::{ProbeNetworkMonitor, TcpProbe};
use arcana_infra::security::TokenServiceOptions;
use arcana_infra::wiring::{ArcanaConfig, ArcanaServices};
use arcana_infra::{AppError, AppResult, ErrorCode};

/// How often the network probe runs.
pub const NETWORK_PROBE_INTERVAL: Duration = Duration::from_secs(30);

/// Where the app keeps its files.
#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data_dir: PathBuf,
}

impl AppPaths {
    /// C# `<local app data>/Arcana`.
    #[must_use]
    pub fn default_location() -> Self {
        Self {
            data_dir: arcana_infra::settings::app_data_dir(),
        }
    }

    #[must_use]
    pub fn database_url(&self) -> String {
        arcana_infra::wiring::sqlite_url(&self.data_dir.join("data").join("arcana.db"))
    }
}

/// A fresh random token key per run: the desktop app issues no long-lived tokens, and the
/// M3 rule is that the key is never derived from guessable machine data.
#[must_use]
pub fn config(dir: &Path) -> ArcanaConfig {
    let mut key = uuid::Uuid::new_v4().as_bytes().to_vec();
    key.extend_from_slice(uuid::Uuid::new_v4().as_bytes());
    ArcanaConfig {
        token_key: key,
        token_options: TokenServiceOptions::default(),
        settings_path: dir.join("preferences.json"),
        app_settings_path: dir.join("settings.json"),
    }
}

/// Open the database, wire services, seed demo data if the database is empty.
///
/// # Errors
/// Directory creation, database or seeding failure.
pub async fn bootstrap(paths: &AppPaths) -> AppResult<ArcanaServices> {
    std::fs::create_dir_all(paths.data_dir.join("data"))
        .map_err(|e| AppError::data(ErrorCode::FileAccessDenied, e.to_string()))?;
    let services = ArcanaServices::open(&paths.database_url(), config(&paths.data_dir)).await?;
    if arcana_infra::seed::seed_demo_data(&services).await? {
        tracing::info!("seeded demo data");
    }
    Ok(services)
}

/// Start the TCP-probe network monitor.
#[must_use]
pub fn network_monitor() -> Arc<ProbeNetworkMonitor> {
    let m = Arc::new(ProbeNetworkMonitor::new(TcpProbe::default()));
    if let Err(e) = m.start(NETWORK_PROBE_INTERVAL) {
        tracing::warn!("network monitor not started: {e}");
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_keys_are_random_and_paths_live_in_data_dir() {
        let a = config(Path::new("/x"));
        let b = config(Path::new("/x"));
        assert_eq!(a.token_key.len(), 32);
        assert_ne!(a.token_key, b.token_key);
        assert_eq!(a.app_settings_path, Path::new("/x/settings.json"));
        let p = AppPaths {
            data_dir: PathBuf::from("/d"),
        };
        assert_eq!(p.database_url(), "sqlite:///d/data/arcana.db");
        assert!(AppPaths::default_location().data_dir.ends_with("Arcana"));
    }

    #[test]
    fn bootstrap_creates_and_seeds_then_reuses() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths {
            data_dir: dir.path().to_path_buf(),
        };
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let svc = rt.block_on(bootstrap(&paths)).unwrap();
        let page = rt
            .block_on(
                svc.orders
                    .get_orders(&arcana_infra::PageRequest::new(1, 50)),
            )
            .unwrap();
        assert_eq!(page.total_count, 10);
        drop(svc);
        let again = rt.block_on(bootstrap(&paths)).unwrap();
        let page = rt
            .block_on(
                again
                    .orders
                    .get_orders(&arcana_infra::PageRequest::new(1, 50)),
            )
            .unwrap();
        assert_eq!(page.total_count, 10, "not seeded twice");
        let m = network_monitor();
        m.stop();
    }
}
