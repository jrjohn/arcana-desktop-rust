//! `PluginHost` lifecycle: C# `PluginLoadContextTests` (21) mapped onto the host's
//! load / unload through an injected `PluginLoader`, plus install / lazy activation /
//! permission approval / upgrade / rollback / uninstall / reload (C# `PluginManagerService`
//! has no unit tests; these are new).

#[path = "support/host.rs"]
mod host;

use std::any::Any;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use arcana_core::AppResult;
use arcana_plugin_api::{
    CommandService, Plugin, PluginHotReload, PluginManifest, PluginMetadata, PluginPermission,
    PluginState, PluginUpgradeable,
};
use arcana_plugin_host::PluginHost;
use arcana_plugin_host::loader::PluginLoader;
use arcana_plugin_host::plugin_base::{BasePlugin, PluginBase, PluginBehavior};
use host::{Fixture, fixture, tmp, write};

/// Test plugin built from a manifest; `pluginClass` picks extra behaviour.
struct Fake {
    meta: PluginMetadata,
    class: String,
    count: i64,
    log: Arc<Mutex<Vec<String>>>,
}

impl PluginBehavior for Fake {
    fn metadata(&self) -> PluginMetadata {
        self.meta.clone()
    }
    fn register_contributions(&mut self, base: &mut PluginBase) -> AppResult<()> {
        let log = self.log.clone();
        let id = self.meta.id.clone();
        base.register_command(&format!("{id}.run"), move |_| {
            log.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push("ran".into());
            Ok(())
        })
    }
    fn as_hot_reload(&mut self) -> Option<&mut dyn PluginHotReload> {
        (self.class == "hot").then_some(self as &mut dyn PluginHotReload)
    }
    fn as_upgradeable(&mut self) -> Option<&mut dyn PluginUpgradeable> {
        (self.class == "veto").then_some(self as &mut dyn PluginUpgradeable)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl PluginHotReload for Fake {
    fn on_before_reload(&mut self) -> AppResult<()> {
        Ok(())
    }
    fn on_after_reload(&mut self) -> AppResult<()> {
        Ok(())
    }
    fn state(&self) -> BTreeMap<String, serde_json::Value> {
        BTreeMap::from([("count".to_string(), serde_json::Value::from(self.count))])
    }
    fn restore_state(&mut self, state: BTreeMap<String, serde_json::Value>) {
        self.count = state
            .get("count")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0);
    }
}

impl PluginUpgradeable for Fake {
    fn on_before_upgrade(&mut self, _: &semver::Version, _: &semver::Version) -> bool {
        true
    }
    fn on_after_upgrade(&mut self, _: &semver::Version, _: &semver::Version) {}
    fn on_before_rollback(&mut self, _: &semver::Version, _: &semver::Version) -> bool {
        false
    }
    fn on_after_rollback(&mut self, _: &semver::Version, _: &semver::Version) {}
    fn can_upgrade_to(&self, target: &semver::Version) -> bool {
        target.major < 9
    }
    fn can_rollback_to(&self, _: &semver::Version) -> bool {
        true
    }
}

#[derive(Default)]
struct FakeLoader {
    loads: AtomicUsize,
    granted: Mutex<Vec<PluginPermission>>,
    log: Arc<Mutex<Vec<String>>>,
}

impl PluginLoader for FakeLoader {
    fn can_load(&self, m: &PluginManifest) -> bool {
        m.main
            .as_deref()
            .is_some_and(|f| f.to_ascii_lowercase().ends_with(".fake"))
    }
    fn load(
        &self,
        m: &PluginManifest,
        _dir: &Path,
        granted: PluginPermission,
    ) -> AppResult<Box<dyn Plugin>> {
        self.loads.fetch_add(1, Ordering::SeqCst);
        self.granted
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(granted);
        let mut meta = PluginMetadata::new(
            &m.id,
            &m.name,
            arcana_plugin_host::compat::parse_lenient(&m.version),
            m.parsed_type(),
        );
        meta.dependencies = m.dependencies.clone().unwrap_or_default();
        Ok(BasePlugin::boxed(Fake {
            meta,
            class: m.plugin_class.clone().unwrap_or_default(),
            count: 0,
            log: self.log.clone(),
        }))
    }
}

struct Env {
    f: Fixture,
    dir: tempfile::TempDir,
    host: Arc<PluginHost>,
    loader: Arc<FakeLoader>,
}

fn env() -> Env {
    let f = fixture();
    let dir = tmp();
    let host = f.host(dir.path());
    let loader = Arc::new(FakeLoader::default());
    host.add_loader(loader.clone());
    Env {
        f,
        dir,
        host,
        loader,
    }
}

fn manifest(id: &str, version: &str, extra: &str) -> String {
    format!(
        r#"{{"id":"{id}","name":"{id}","version":"{version}","main":"p.fake"{extra},
            "activationEvents":["onCommand:{id}.run"],
            "contributes":{{"commands":[{{"id":"{id}.run"}}]}}}}"#
    )
}

impl Env {
    fn package(&self, name: &str, json: &str) -> std::path::PathBuf {
        let p = self.dir.path().join("packages").join(name);
        write(&p.join("plugin.json"), json);
        write(&p.join("p.fake"), name);
        p
    }
    async fn install(&self, id: &str, version: &str, extra: &str) {
        let pkg = self.package(&format!("{id}-{version}"), &manifest(id, version, extra));
        let r = self.host.install(&pkg).await;
        assert!(r.success, "{r:?}");
    }
    fn loads(&self) -> usize {
        self.loader.loads.load(Ordering::SeqCst)
    }
    fn state(&self, id: &str) -> PluginState {
        self.host.state(id)
    }
    fn count(&self, id: &str) -> Option<i64> {
        self.host
            .with_plugin(id, |p| p.as_hot_reload().map(|h| h.state()))
            .flatten()
            .and_then(|s| s.get("count").and_then(serde_json::Value::as_i64))
    }
    fn set_count(&self, id: &str, n: i64) {
        let _ = self.host.with_plugin(id, |p| {
            if let Some(h) = p.as_hot_reload() {
                h.restore_state(BTreeMap::from([("count".into(), n.into())]));
            }
        });
    }
}

// ── C# PluginLoadContextTests ──────────────────────────────────────────────

// Constructor_ShouldSetPluginPath
#[tokio::test]
async fn load_sets_plugin_path() {
    let e = env();
    e.install("a", "1.0.0", "").await;
    let p = e.host.plugin("a").unwrap_or_else(|| panic!("a"));
    assert_eq!(Path::new(&p.install_path), e.host.paths.plugins.join("a"));
}

// Constructor_ShouldSetPluginId
#[tokio::test]
async fn load_sets_plugin_id() {
    let e = env();
    e.install("a", "1.0.0", "").await;
    assert!(e.host.activate("a").is_ok());
    assert_eq!(e.host.plugin("a").map(|p| p.id).as_deref(), Some("a"));
}

// CreateContext_ShouldReturnNewContext — activation of a pending plugin loads it once.
#[tokio::test]
async fn activation_loads_through_the_loader() {
    let e = env();
    e.install("a", "1.0.0", "").await;
    assert_eq!(e.loads(), 0, "lazy: nothing loaded at install");
    assert!(e.host.activate("a").is_ok());
    assert_eq!(e.loads(), 1);
}

// CreateContext_SamePluginTwice_ShouldReturnNewContext — reload builds a fresh instance.
#[tokio::test]
async fn reload_creates_a_new_instance() {
    let e = env();
    e.install("a", "1.0.0", "").await;
    assert!(e.host.activate("a").is_ok());
    assert!(e.host.reload("a").success);
    assert_eq!(e.loads(), 2);
    assert_eq!(e.state("a"), PluginState::Active);
}

// CreateContext_DifferentPlugins_ShouldReturnDifferentContexts
#[tokio::test]
async fn different_plugins_load_separately() {
    let e = env();
    e.install("a", "1.0.0", "").await;
    e.install("b", "1.0.0", "").await;
    assert!(e.host.activate("a").is_ok() && e.host.activate("b").is_ok());
    assert_eq!(e.loads(), 2);
}

// IsAlive_WithActiveContext_ShouldBeTrue
// GetContext_WithActiveContext_ShouldReturnContext
// GetContext_ExistingPlugin_ShouldReturnContext
#[tokio::test]
async fn loaded_plugin_is_alive_and_found() {
    let e = env();
    e.install("a", "1.0.0", "").await;
    assert!(e.host.activate("a").is_ok());
    assert_eq!(e.state("a"), PluginState::Active);
    assert!(!e.host.is_pending("a"));
    assert!(e.host.with_plugin("a", |_| ()).is_some());
}

// GetContext_NonExistingPlugin_ShouldReturnNull
#[test]
fn unknown_plugin_is_none() {
    let e = env();
    assert!(e.host.plugin("nope").is_none());
    assert!(e.host.activate("nope").is_err());
}

// UnloadContext_ExistingPlugin_ShouldInitiateUnload
// UnloadContextAsync_ShouldReturnTrue_WhenNoReferencesHeld
// GetContext_AfterContextUnloaded_ShouldReturnNull
#[tokio::test]
async fn unload_existing_removes_it() {
    let e = env();
    e.install("a", "1.0.0", "").await;
    assert!(e.host.activate("a").is_ok());
    assert!(e.host.unload("a"));
    assert!(e.host.with_plugin("a", |_| ()).is_none());
    assert!(!e.f.commands.has_command("a.run"), "subscriptions disposed");
}

// UnloadContext_NonExistingPlugin_ShouldNotThrow
// InitiateUnload_Twice_ShouldNotThrow
#[tokio::test]
async fn unload_unknown_or_twice_is_false() {
    let e = env();
    assert!(!e.host.unload("nope"));
    e.install("a", "1.0.0", "").await;
    assert!(e.host.activate("a").is_ok());
    assert!(e.host.unload("a"));
    assert!(!e.host.unload("a"));
}

// Dispose_ShouldInitiateUnloadForAllContexts
#[tokio::test]
async fn unloading_all_leaves_nothing_loaded() {
    let e = env();
    e.install("a", "1.0.0", "").await;
    e.install("b", "1.0.0", "").await;
    assert!(e.host.activate("a").is_ok() && e.host.activate("b").is_ok());
    for id in ["a", "b"] {
        assert!(e.host.unload(id));
    }
    assert!(e.host.all_plugins().is_empty());
}

// Not ported (6): Constructor_ShouldBeCollectible, Constructor_ShouldSetName,
// InitiateUnload_ShouldRaiseUnloadingEvent, GetUnloadStatus_EmptyManager / _WithPlugins,
// Dispose_Twice — AssemblyLoadContext / GC-collectibility specifics with no Rust analogue
// (a dropped `Box<dyn Plugin>` is freed deterministically; WASM instance isolation is tested
// in arcana-plugin-wasm).

// ── PluginManagerService lifecycle (new) ───────────────────────────────────

#[tokio::test]
async fn install_registers_lazy_command_that_activates_on_use() {
    let e = env();
    e.install("a", "1.0.0", "").await;
    assert_eq!(e.state("a"), PluginState::Loaded);
    assert!(e.f.commands.has_command("a.run"), "lazy placeholder");
    assert!(e.f.commands.execute("a.run", Vec::new()).is_ok());
    assert_eq!(e.state("a"), PluginState::Active);
    assert_eq!(e.loads(), 1);
    assert_eq!(
        e.loader
            .log
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len(),
        1,
        "the real handler ran after activation"
    );
}

#[tokio::test]
async fn install_twice_is_rejected_and_missing_package_fails() {
    let e = env();
    e.install("a", "1.0.0", "").await;
    let pkg = e.package("dup", &manifest("a", "1.0.0", ""));
    let r = e.host.install(&pkg).await;
    assert_eq!(r.error_code.as_deref(), Some("ALREADY_INSTALLED"));
    let r = e.host.install(&e.dir.path().join("missing")).await;
    assert_eq!(r.error_code.as_deref(), Some("FILE_NOT_FOUND"));
}

#[tokio::test]
async fn requested_permissions_are_granted_only_after_approval() {
    let e = env();
    e.install("a", "1.0.0", r#","permissions":["NetworkAccess"]"#)
        .await;
    assert!(e.host.activate("a").is_ok());
    assert!(e.host.approve_permissions("a").success);
    assert!(e.host.reload("a").success);
    let g = e
        .loader
        .granted
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert!(!g[0].contains(PluginPermission::NETWORK_ACCESS));
    assert!(g[1].contains(PluginPermission::NETWORK_ACCESS));
}

#[tokio::test]
async fn upgrade_backs_up_records_version_and_keeps_state() {
    let e = env();
    e.install("a", "1.0.0", r#","pluginClass":"hot""#).await;
    assert!(e.host.activate("a").is_ok());
    e.set_count("a", 7);
    let pkg = e.package("a2", &manifest("a", "2.0.0", r#","pluginClass":"hot""#));
    let r = e.host.upgrade("a", &pkg).await;
    assert!(r.success, "{r:?}");
    assert_eq!(
        e.host.plugin("a").map(|p| p.version),
        Some(semver::Version::new(2, 0, 0))
    );
    assert_eq!(e.state("a"), PluginState::Active);
    assert_eq!(e.count("a"), Some(7), "hot-reload state carried over");
    assert!(e.host.paths.backups.join("a/1.0.0/plugin.json").is_file());
    let v: Vec<String> = e
        .host
        .available_versions("a")
        .await
        .iter()
        .map(|v| format!("{}{}", v.version, if v.is_current { "*" } else { "" }))
        .collect();
    assert_eq!(v, ["2.0.0*", "1.0.0"]);
}

#[tokio::test]
async fn upgrade_of_inactive_plugin_keeps_it_inactive() {
    let e = env();
    e.install("a", "1.0.0", "").await;
    let pkg = e.package("a2", &manifest("a", "2.0.0", ""));
    assert!(e.host.upgrade("a", &pkg).await.success);
    assert_ne!(e.state("a"), PluginState::Active);
}

#[tokio::test]
async fn upgradeable_plugin_can_veto() {
    let e = env();
    e.install("a", "1.0.0", r#","pluginClass":"veto""#).await;
    assert!(e.host.activate("a").is_ok());
    let pkg = e.package("a9", &manifest("a", "9.0.0", ""));
    let r = e.host.upgrade("a", &pkg).await;
    assert_eq!(r.error_code.as_deref(), Some("UPGRADE_NOT_SUPPORTED"));
    assert_eq!(
        e.host.plugin("a").map(|p| p.version),
        Some(semver::Version::new(1, 0, 0))
    );
    assert_eq!(e.state("a"), PluginState::Active);
}

#[tokio::test]
async fn failed_upgrade_restores_files() {
    let e = env();
    e.install("a", "1.0.0", "").await;
    // A package for another plugin id is refused before anything is touched.
    let wrong = e.package("b", &manifest("b", "2.0.0", ""));
    let r = e.host.upgrade("a", &wrong).await;
    assert!(!r.success);
    // An incompatible WASM ABI is refused too; the installed files stay at 1.0.0.
    let bad = e.package(
        "abi",
        &manifest("a", "2.0.0", "").replace("p.fake", "p.wasm"),
    );
    assert_eq!(
        e.host.upgrade("a", &bad).await.error_code.as_deref(),
        Some("INCOMPATIBLE")
    );
    let installed =
        std::fs::read_to_string(e.host.paths.plugins.join("a/plugin.json")).unwrap_or_default();
    assert!(installed.contains("\"1.0.0\""));
    assert_eq!(e.host.plugin("a").map(|p| p.version.major), Some(1));
}

#[tokio::test]
async fn rollback_restores_previous_version() {
    let e = env();
    e.install("a", "1.0.0", "").await;
    assert!(e.host.activate("a").is_ok());
    let pkg = e.package("a2", &manifest("a", "2.0.0", ""));
    assert!(e.host.upgrade("a", &pkg).await.success);
    let r = e.host.rollback("a", "1.0.0").await;
    assert!(r.success, "{r:?}");
    assert_eq!(
        e.host.plugin("a").map(|p| p.version),
        Some(semver::Version::new(1, 0, 0))
    );
    assert_eq!(e.state("a"), PluginState::Active);
    let r = e.host.rollback("a", "0.1.0").await;
    assert_eq!(r.error_code.as_deref(), Some("VERSION_NOT_FOUND"));
}

#[tokio::test]
async fn rollback_veto_by_upgradeable_plugin() {
    let e = env();
    e.install("a", "1.0.0", r#","pluginClass":"veto""#).await;
    assert!(e.host.activate("a").is_ok());
    let pkg = e.package("a2", &manifest("a", "2.0.0", r#","pluginClass":"veto""#));
    assert!(e.host.upgrade("a", &pkg).await.success);
    let r = e.host.rollback("a", "1.0.0").await;
    assert_eq!(r.error_code.as_deref(), Some("ROLLBACK_REJECTED"));
}

#[tokio::test]
async fn uninstall_removes_files_history_and_contributions() {
    let e = env();
    e.install("a", "1.0.0", "").await;
    assert!(e.host.activate("a").is_ok());
    let r = e.host.uninstall("a").await;
    assert!(r.success, "{r:?}");
    assert!(e.host.plugin("a").is_none());
    assert!(!e.host.paths.plugins.join("a").exists());
    assert!(e.host.available_versions("a").await.is_empty());
    assert!(!e.f.commands.has_command("a.run"));
}

#[test]
fn builtins_cannot_be_uninstalled_or_upgraded() {
    struct B;
    impl PluginBehavior for B {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata::new(
                "b",
                "B",
                semver::Version::new(1, 0, 0),
                arcana_plugin_api::PluginType::Module,
            )
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
    }
    let e = env();
    e.host.register_builtin(BasePlugin::boxed(B));
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap_or_else(|err| panic!("{err}"));
    let r = rt.block_on(e.host.uninstall("b"));
    assert_eq!(r.error_code.as_deref(), Some("BUILTIN_PLUGIN"));
    let r = rt.block_on(e.host.upgrade("b", e.dir.path()));
    assert_eq!(r.error_code.as_deref(), Some("BUILTIN_PLUGIN"));
}

#[tokio::test]
async fn dependencies_activate_first() {
    let e = env();
    e.install("base", "1.0.0", "").await;
    e.install("top", "1.0.0", r#","dependencies":["base"]"#)
        .await;
    assert!(e.host.activate("top").is_ok());
    assert_eq!(e.state("base"), PluginState::Active);
}
