#!/usr/bin/env bash
# Native (non-Docker) CI for macOS / Linux agents: toolchain bootstrap, tests, release build.
# Idempotent: installs rustup into the build user's $HOME only when cargo is missing.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
export PATH="$HOME/.cargo/bin:$PATH"
if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo not found: installing rustup (minimal, stable) for $(whoami)"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable --no-modify-path
fi
rustup toolchain install stable --profile minimal >/dev/null
rustup target add wasm32-unknown-unknown >/dev/null
if ! wasm-tools --version 2>/dev/null | grep -q '^wasm-tools 1.262.0'; then
  cargo install wasm-tools --version 1.262.0 --locked
fi
rustc --version && cargo --version
cd "$root"
bash scripts/build-wasm-example.sh
cargo test --workspace --locked
cargo build --release --locked -p arcana-app
ls -l target/release/arcana-app
# On macOS also prove the .app/.dmg packaging works (ad-hoc signed; release build reused).
if [ "$(uname -s)" = Darwin ]; then
  bash scripts/package.sh macos
fi
