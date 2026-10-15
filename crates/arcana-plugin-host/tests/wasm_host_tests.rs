//! End-to-end: the example WASM component installed as a package, lazily activated by its
//! command, permission-gated, rendering a declarative UI tree. New (C# has no sandbox).
//! Build the component first: `bash scripts/build-wasm-example.sh`.

#[path = "support/host.rs"]
mod host;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use arcana_plugin_api::{CommandService, PluginState};
use arcana_plugin_host::compat::HOST_ABI_VERSION;
use arcana_plugin_host::wasm::WasmLoader;

fn example() -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/wasm/example.wasm");
    std::fs::read(&p).unwrap_or_else(|e| {
        panic!(
            "{}: {e} — run `bash scripts/build-wasm-example.sh`",
            p.display()
        )
    })
}

fn package(dir: &Path, abi: u32) -> PathBuf {
    let pkg = dir.join("pkg");
    std::fs::create_dir_all(&pkg).unwrap();
    std::fs::write(pkg.join("example.wasm"), example()).unwrap();
    std::fs::write(
        pkg.join("plugin.json"),
        format!(
            r#"{{"id":"example.counter","name":"Counter","version":"1.0.0","main":"example.wasm",
            "abiVersion":{abi},"permissions":["ReadLocalData"],
            "activationEvents":["onCommand:inc"],
            "contributes":{{"commands":[{{"id":"inc","title":"Increment"}},{{"id":"read","title":"Read"}}]}}}}"#
        ),
    )
    .unwrap();
    pkg
}

#[tokio::test]
async fn wasm_package_installs_activates_lazily_and_renders() {
    let fx = host::fixture();
    let dir = host::tmp();
    let host = fx.host(dir.path());
    host.add_loader(Arc::new(WasmLoader::default()));
    let r = host.install(&package(dir.path(), HOST_ABI_VERSION)).await;
    assert!(r.success, "{r:?}");
    assert_eq!(host.state("example.counter"), PluginState::Loaded);
    // Requested permission is pending, not granted.
    assert!(host.permissions.permissions("example.counter").is_empty());
    // Lazy command activates the plugin and runs the guest's command.
    fx.commands.execute("inc", Vec::new()).unwrap();
    assert_eq!(host.state("example.counter"), PluginState::Active);
    let tree = host
        .with_plugin("example.counter", |p| p.ui_tree())
        .flatten()
        .unwrap();
    assert_eq!(tree["type"], "form");
    assert!(tree.to_string().contains("Count: 1"));
}

#[tokio::test]
async fn wasm_file_read_denied_until_approved() {
    let fx = host::fixture();
    let dir = host::tmp();
    let host = fx.host(dir.path());
    host.add_loader(Arc::new(WasmLoader::default()));
    assert!(
        host.install(&package(dir.path(), HOST_ABI_VERSION))
            .await
            .success
    );
    host.activate("example.counter").unwrap();
    // `read` with no grant: the guest's import call is refused, command fails.
    assert!(fx.commands.execute("read", Vec::new()).is_err());
    assert!(host.approve_permissions("example.counter").success);
    assert!(!host.permissions.permissions("example.counter").is_empty());
}

#[tokio::test]
async fn wasm_bad_abi_is_rejected_at_install() {
    let fx = host::fixture();
    let dir = host::tmp();
    let host = fx.host(dir.path());
    let r = host
        .install(&package(dir.path(), HOST_ABI_VERSION + 1))
        .await;
    assert!(!r.success);
    assert_eq!(r.error_code.as_deref(), Some("INCOMPATIBLE"));
}
