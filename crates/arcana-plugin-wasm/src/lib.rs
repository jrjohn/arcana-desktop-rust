//! WASM sandbox for third-party plugins (wasmtime component model, `wit/plugin.wit`).
//!
//! Isolation rules:
//! - **No capability by default.** The linker always defines the four `arcana:plugin` host
//!   interfaces so any conforming component instantiates, but `host-fs` / `host-net` answer
//!   `Err("permission denied: …")` and `host-log` drops messages unless the matching [`Grants`]
//!   entry was approved. No WASI is linked: a component importing anything else fails to load.
//! - **Limits per call:** fuel ([`Limits::fuel`]), linear memory ([`Limits::memory_bytes`],
//!   growth beyond it traps) and wall clock ([`Limits::wall_clock`], epoch interruption).
//! - **Transactional:** every call runs in a fresh `Store` + instance. Writes through
//!   `host-state.set` go to a copy that replaces [`WasmPlugin::state`] only when the call
//!   returns `Ok`; a trap, limit hit or `Err` result discards them. Plugin memory therefore
//!   never survives between calls — durable plugin state lives in the host state map.

mod host;
pub mod ui;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use std::time::Duration;

use arcana_core::{AppError, AppResult, ErrorCode};
use serde::Deserialize;
use wasmtime::component::{Component, ComponentNamedList, Lift, Linker, Lower};
use wasmtime::{Config, Engine, Store, StoreLimitsBuilder, Trap};

pub use ui::UiNode;

use host::Host;

/// ABI version the host speaks; a component reporting another value is rejected at load.
pub const ABI_VERSION: u32 = 1;

/// Resource ceilings applied to every call into a plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Fuel units (≈ wasm instructions) per call.
    pub fuel: u64,
    /// Maximum linear memory in bytes.
    pub memory_bytes: usize,
    /// Maximum wall-clock time per call.
    pub wall_clock: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            fuel: 50_000_000,
            memory_bytes: 32 * 1024 * 1024,
            wall_clock: Duration::from_secs(2),
        }
    }
}

/// Outbound HTTP used when the network permission is granted (the host chooses the client).
pub trait HttpGet: Send + Sync {
    /// Fetch `url` and return the body.
    ///
    /// # Errors
    /// Any transport failure, as a message for the plugin.
    fn get(&self, url: &str) -> Result<String, String>;
}

/// Approved permissions. `Default` grants nothing.
#[derive(Clone, Default)]
pub struct Grants {
    /// `host-log.log` messages are kept (see [`WasmPlugin::logs`]).
    pub log: bool,
    /// `host-fs.read-file` may read files below this directory.
    pub fs_read: Option<PathBuf>,
    /// `host-net.http-get` is served by this client.
    pub net: Option<Arc<dyn HttpGet>>,
}

impl std::fmt::Debug for Grants {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Grants")
            .field("log", &self.log)
            .field("fs_read", &self.fs_read)
            .field("net", &self.net.is_some())
            .finish()
    }
}

/// `metadata` export contents.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WasmPluginMeta {
    pub id: String,
    pub name: String,
    pub version: String,
}

/// Compiles and loads plugin components under one set of [`Limits`].
#[derive(Debug, Clone)]
pub struct WasmSandbox {
    limits: Limits,
}

impl WasmSandbox {
    /// A sandbox applying `limits` to every plugin it loads.
    #[must_use]
    pub fn new(limits: Limits) -> Self {
        Self { limits }
    }

    /// The limits applied to plugins.
    #[must_use]
    pub fn limits(&self) -> Limits {
        self.limits
    }

    /// Compile `bytes`, check the ABI version and read the metadata.
    ///
    /// # Errors
    /// `PluginIncompatible` for an ABI mismatch or a missing export; `PluginLoadFailed` for an
    /// invalid component, unknown imports, bad metadata or a limit hit while loading.
    pub fn load(&self, bytes: &[u8], grants: Grants) -> AppResult<WasmPlugin> {
        let mut config = Config::new();
        config.consume_fuel(true).epoch_interruption(true);
        // Each plugin gets its own engine so one plugin's timeout never interrupts another.
        let engine = Engine::new(&config).map_err(|e| load_err(&format!("engine: {e}")))?;
        let component = Component::new(&engine, bytes)
            .map_err(|e| load_err(&format!("invalid component: {e}")))?;
        let mut linker = Linker::new(&engine);
        host::define(&mut linker).map_err(|e| load_err(&format!("linker: {e}")))?;
        let mut plugin = WasmPlugin {
            engine,
            component,
            linker: Arc::new(linker),
            limits: self.limits,
            grants,
            state: BTreeMap::new(),
            logs: Vec::new(),
            meta: None,
        };
        let (abi,) = plugin.call::<(), (u32,)>("abi-version", ())?;
        if abi != ABI_VERSION {
            return Err(AppError::Plugin {
                code: ErrorCode::PluginIncompatible,
                message: format!("ABI version {abi} not supported (host speaks {ABI_VERSION})"),
                plugin_id: None,
            });
        }
        let (json,) = plugin.call::<(), (String,)>("metadata", ())?;
        let meta: WasmPluginMeta =
            serde_json::from_str(&json).map_err(|e| load_err(&format!("invalid metadata: {e}")))?;
        plugin.meta = Some(meta);
        Ok(plugin)
    }
}

/// A loaded third-party plugin.
pub struct WasmPlugin {
    engine: Engine,
    component: Component,
    linker: Arc<Linker<Host>>,
    limits: Limits,
    grants: Grants,
    state: BTreeMap<String, String>,
    logs: Vec<String>,
    meta: Option<WasmPluginMeta>,
}

impl std::fmt::Debug for WasmPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasmPlugin")
            .field("meta", &self.meta)
            .field("grants", &self.grants)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl WasmPlugin {
    /// Metadata read at load time (always `Some` for plugins returned by `load`).
    #[must_use]
    pub fn meta(&self) -> Option<&WasmPluginMeta> {
        self.meta.as_ref()
    }

    /// Committed plugin state.
    #[must_use]
    pub fn state(&self) -> &BTreeMap<String, String> {
        &self.state
    }

    /// Log lines from successful calls (empty unless [`Grants::log`]).
    #[must_use]
    pub fn logs(&self) -> &[String] {
        &self.logs
    }

    /// Call the `activate` export.
    ///
    /// # Errors
    /// The plugin's own error, or a sandbox error (limit, trap).
    pub fn activate(&mut self) -> AppResult<()> {
        let (r,) = self.call_checked("activate", (), |(r,): &(Result<(), String>,)| r.is_ok())?;
        r.map_err(|m| self.plugin_err(m))
    }

    /// Call the `deactivate` export.
    ///
    /// # Errors
    /// As [`Self::activate`].
    pub fn deactivate(&mut self) -> AppResult<()> {
        let (r,) = self.call_checked("deactivate", (), |(r,): &(Result<(), String>,)| r.is_ok())?;
        r.map_err(|m| self.plugin_err(m))
    }

    /// Call `render` and validate the returned UI tree.
    ///
    /// # Errors
    /// Sandbox errors or an invalid tree.
    pub fn render(&mut self) -> AppResult<UiNode> {
        let (json,) = self.call::<(), (String,)>("render", ())?;
        UiNode::parse(&json)
    }

    /// Call `run-command(name, arg)`.
    ///
    /// # Errors
    /// The plugin's own error, or a sandbox error.
    pub fn run_command(&mut self, name: &str, arg: &str) -> AppResult<String> {
        let (r,) = self.call_checked(
            "run-command",
            (name.to_string(), arg.to_string()),
            |(r,): &(Result<String, String>,)| r.is_ok(),
        )?;
        r.map_err(|m| self.plugin_err(m))
    }

    fn plugin_err(&self, message: String) -> AppError {
        AppError::Plugin {
            code: ErrorCode::PluginLoadFailed,
            message,
            plugin_id: self.meta.as_ref().map(|m| m.id.clone()),
        }
    }

    /// A call whose state commits always on a clean return.
    fn call<P, R>(&mut self, export: &str, params: P) -> AppResult<R>
    where
        P: ComponentNamedList + Lower + Send + Sync,
        R: ComponentNamedList + Lift + Send + Sync + 'static,
    {
        self.call_checked(export, params, |_| true)
    }

    /// One transactional call: fresh store, limits, watchdog. State writes commit only when
    /// the call returns without trapping **and** `commit_if` accepts the plugin's result
    /// (a plugin-level `Err` is a failure too).
    fn call_checked<P, R>(
        &mut self,
        export: &str,
        params: P,
        commit_if: impl FnOnce(&R) -> bool,
    ) -> AppResult<R>
    where
        P: ComponentNamedList + Lower + Send + Sync,
        R: ComponentNamedList + Lift + Send + Sync + 'static,
    {
        let limiter = StoreLimitsBuilder::new()
            .memory_size(self.limits.memory_bytes)
            .trap_on_grow_failure(true)
            .build();
        let host = Host::new(self.grants.clone(), self.state.clone(), limiter);
        let mut store = Store::new(&self.engine, host);
        store.limiter(|h| &mut h.limits);
        store
            .set_fuel(self.limits.fuel)
            .map_err(|e| load_err(&format!("fuel: {e}")))?;
        store.set_epoch_deadline(1);

        let (tx, rx) = mpsc::channel::<()>();
        let engine = self.engine.clone();
        let wall = self.limits.wall_clock;
        std::thread::spawn(move || {
            if rx.recv_timeout(wall) == Err(mpsc::RecvTimeoutError::Timeout) {
                engine.increment_epoch();
            }
        });

        let result = (|| {
            let instance = self.linker.instantiate(&mut store, &self.component)?;
            let func = instance.get_typed_func::<P, R>(&mut store, export)?;
            func.call(&mut store, params)
        })();
        drop(tx);

        match result {
            Ok(r) => {
                if !commit_if(&r) {
                    return Ok(r);
                }
                let host = store.into_data();
                self.state = host.pending;
                self.logs.extend(host.logs);
                Ok(r)
            }
            Err(e) => Err(self.classify(export, &e)),
        }
    }

    fn classify(&self, export: &str, e: &wasmtime::Error) -> AppError {
        let detail = format!("{e:?}");
        let (code, message) = match e.downcast_ref::<Trap>() {
            Some(Trap::OutOfFuel) => (ErrorCode::PluginLoadFailed, "fuel exhausted".to_string()),
            Some(Trap::Interrupt) => (
                ErrorCode::PluginLoadFailed,
                "wall-clock timeout".to_string(),
            ),
            _ if detail.contains("growing memory") => (
                ErrorCode::PluginLoadFailed,
                "memory limit exceeded".to_string(),
            ),
            _ if detail.contains("failed to find function export")
                || detail.contains("no export named") =>
            {
                (
                    ErrorCode::PluginIncompatible,
                    format!("missing export `{export}`"),
                )
            }
            _ => (ErrorCode::PluginLoadFailed, format!("trap: {e}")),
        };
        AppError::Plugin {
            code,
            message: format!("{export}: {message}"),
            plugin_id: self.meta.as_ref().map(|m| m.id.clone()),
        }
    }
}

fn load_err(message: &str) -> AppError {
    ui::plugin_err(message.to_string())
}
