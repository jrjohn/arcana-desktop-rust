//! Sandbox behaviour against the real example component (`scripts/build-wasm-example.sh`)
//! plus hand-written WAT components for ABI cases.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use arcana_core::ErrorCode;
use arcana_plugin_wasm::{Grants, HttpGet, Limits, UiNode, WasmPlugin, WasmSandbox};

fn example_bytes() -> Vec<u8> {
    let path = std::env::var_os("ARCANA_WASM_EXAMPLE").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/wasm/example.wasm"),
        PathBuf::from,
    );
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "example plugin missing at {} ({e}); run `bash scripts/build-wasm-example.sh`",
            path.display()
        )
    })
}

fn load(limits: Limits, grants: Grants) -> WasmPlugin {
    WasmSandbox::new(limits)
        .load(&example_bytes(), grants)
        .unwrap()
}

fn plain() -> WasmPlugin {
    load(Limits::default(), Grants::default())
}

#[test]
fn example_loads_with_metadata_and_activates() {
    let mut p = load(
        Limits::default(),
        Grants {
            log: true,
            ..Grants::default()
        },
    );
    assert_eq!(p.meta().unwrap().id, "example.counter");
    p.activate().unwrap();
    assert_eq!(p.state().get("activated").map(String::as_str), Some("true"));
    assert_eq!(p.logs(), ["example activated"]);
    p.deactivate().unwrap();
    assert_eq!(
        p.state().get("activated").map(String::as_str),
        Some("false")
    );
}

#[test]
fn log_is_dropped_without_grant() {
    let mut p = plain();
    p.activate().unwrap();
    assert_eq!(p.logs().len(), 0);
}

#[test]
fn render_returns_validated_tree_reflecting_state() {
    let mut p = plain();
    assert_eq!(p.run_command("inc", "").unwrap(), "1");
    assert_eq!(p.run_command("inc", "").unwrap(), "2");
    let UiNode::Form { title, children } = p.render().unwrap() else {
        panic!("expected form")
    };
    assert_eq!(title, "Counter");
    assert_eq!(
        children[0],
        UiNode::Text {
            text: "Count: 2".into()
        }
    );
    assert!(matches!(children[2], UiNode::Table { .. }));
}

#[test]
fn file_read_denied_without_grant() {
    let mut p = plain();
    let e = p.run_command("read", "Cargo.toml").unwrap_err();
    assert!(e.message().contains("permission denied"), "{e}");
}

#[test]
fn file_read_allowed_below_root_only() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "hello").unwrap();
    let mut p = load(
        Limits::default(),
        Grants {
            fs_read: Some(dir.path().to_path_buf()),
            ..Grants::default()
        },
    );
    assert_eq!(p.run_command("read", "a.txt").unwrap(), "hello");
    for escape in ["../a.txt", "/etc/passwd"] {
        let e = p.run_command("read", escape).unwrap_err();
        assert!(e.message().contains("permission denied"), "{escape}: {e}");
    }
}

#[test]
fn network_denied_without_grant() {
    let mut p = plain();
    let e = p.run_command("net", "http://example.com").unwrap_err();
    assert!(e.message().contains("permission denied: network"), "{e}");
}

struct FakeHttp;
impl HttpGet for FakeHttp {
    fn get(&self, url: &str) -> Result<String, String> {
        Ok(format!("body of {url}"))
    }
}

#[test]
fn network_served_by_granted_client() {
    let mut p = load(
        Limits::default(),
        Grants {
            net: Some(Arc::new(FakeHttp)),
            ..Grants::default()
        },
    );
    assert_eq!(
        p.run_command("net", "http://x").unwrap(),
        "body of http://x"
    );
}

#[test]
fn fuel_exhaustion_stops_infinite_loop() {
    let mut p = load(
        Limits {
            fuel: 5_000_000,
            wall_clock: Duration::from_secs(30),
            ..Limits::default()
        },
        Grants::default(),
    );
    let e = p.run_command("spin", "").unwrap_err();
    assert!(e.message().contains("fuel exhausted"), "{e}");
}

#[test]
fn wall_clock_timeout_interrupts_call() {
    let mut p = load(
        Limits {
            fuel: u64::MAX,
            wall_clock: Duration::from_millis(200),
            ..Limits::default()
        },
        Grants::default(),
    );
    let start = Instant::now();
    let e = p.run_command("spin", "").unwrap_err();
    assert!(e.message().contains("wall-clock timeout"), "{e}");
    assert!(start.elapsed() < Duration::from_secs(10));
    // The plugin is still usable afterwards: the next call gets a fresh deadline.
    assert_eq!(p.run_command("inc", "").unwrap(), "1");
}

#[test]
fn memory_cap_traps_large_allocation() {
    let mut p = load(
        Limits {
            memory_bytes: 8 * 1024 * 1024,
            ..Limits::default()
        },
        Grants::default(),
    );
    let e = p.run_command("alloc", "64").unwrap_err();
    assert!(e.message().contains("memory limit exceeded"), "{e}");
    // A small allocation under the cap still works.
    assert_eq!(p.run_command("alloc", "1").unwrap(), "1048576");
}

#[test]
fn failed_calls_leave_state_unchanged() {
    let mut p = load(
        Limits {
            memory_bytes: 8 * 1024 * 1024,
            ..Limits::default()
        },
        Grants::default(),
    );
    p.run_command("inc", "").unwrap();
    let before = p.state().clone();
    // Plugin-level Err after a state write.
    assert!(p.run_command("fail", "").is_err());
    assert_eq!(p.state(), &before);
    // Trap (memory cap) after a state write.
    assert!(p.run_command("alloc", "64").is_err());
    assert_eq!(p.state(), &before);
}

fn wat_component(abi: u32, with_metadata: bool) -> Vec<u8> {
    let meta = if with_metadata {
        r#"(core module $mm
             (memory (export "mem") 1)
             (data (i32.const 16) "{\"id\":\"w\",\"name\":\"W\",\"version\":\"1.0.0\"}")
             (data (i32.const 0) "\10\00\00\00\27\00\00\00")
             (func (export "meta") (result i32) i32.const 0)
             (func (export "realloc") (param i32 i32 i32 i32) (result i32) i32.const 256))
           (core instance $mi (instantiate $mm))
           (func (export "metadata") (result string)
             (canon lift (core func $mi "meta") (memory (core memory $mi "mem")) (realloc (core func $mi "realloc"))))"#
    } else {
        ""
    };
    let text = format!(
        r#"(component
             (core module $m (func (export "abi") (result i32) i32.const {abi}))
             (core instance $i (instantiate $m))
             (func (export "abi-version") (result u32) (canon lift (core func $i "abi")))
             {meta})"#
    );
    wat::parse_str(text).unwrap()
}

#[test]
fn bad_abi_version_is_rejected() {
    let e = WasmSandbox::new(Limits::default())
        .load(&wat_component(99, true), Grants::default())
        .unwrap_err();
    assert_eq!(e.code(), ErrorCode::PluginIncompatible);
    assert!(e.message().contains("ABI version 99"), "{e}");
}

#[test]
fn matching_abi_minimal_component_loads() {
    let p = WasmSandbox::new(Limits::default())
        .load(&wat_component(1, true), Grants::default())
        .unwrap();
    assert_eq!(p.meta().unwrap().name, "W");
}

#[test]
fn missing_export_is_incompatible() {
    let e = WasmSandbox::new(Limits::default())
        .load(&wat_component(1, false), Grants::default())
        .unwrap_err();
    assert_eq!(e.code(), ErrorCode::PluginIncompatible, "{e}");
}

#[test]
fn unknown_import_fails_to_load() {
    let bytes = wat::parse_str(
        r#"(component
             (import "wasi:filesystem/types@0.2.0" (instance (export "stat" (func))))
             (core module $m (func (export "abi") (result i32) i32.const 1))
             (core instance $i (instantiate $m))
             (func (export "abi-version") (result u32) (canon lift (core func $i "abi"))))"#,
    )
    .unwrap();
    let e = WasmSandbox::new(Limits::default())
        .load(&bytes, Grants::default())
        .unwrap_err();
    assert!(e.message().contains("wasi:filesystem"), "{e}");
}

#[test]
fn garbage_bytes_fail_to_load() {
    let e = WasmSandbox::new(Limits::default())
        .load(b"not wasm", Grants::default())
        .unwrap_err();
    assert!(e.message().contains("invalid component"), "{e}");
}
