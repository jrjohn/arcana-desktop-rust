#!/usr/bin/env bash
# Build the example WASM plugin as a component at target/wasm/example.wasm (sandbox tests load it).
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cargo build --release --target wasm32-unknown-unknown \
  --manifest-path "$root/plugins-wasm/example/Cargo.toml" \
  --target-dir "$root/target/wasm-example"
mkdir -p "$root/target/wasm"
wasm-tools component new \
  "$root/target/wasm-example/wasm32-unknown-unknown/release/arcana_wasm_example.wasm" \
  -o "$root/target/wasm/example.wasm"
echo "built $root/target/wasm/example.wasm"
