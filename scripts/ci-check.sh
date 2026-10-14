#!/usr/bin/env bash
# The same gate runs locally and in Jenkins: `bash scripts/ci-check.sh`.
set -euo pipefail
cargo fmt --all -- --check
cargo xtask layers
# JSON for SonarQube, then the blocking human-readable run.
cargo clippy --workspace --all-targets --message-format=json -- -D warnings > clippy-report.json || true
cargo clippy --workspace --all-targets -- -D warnings
cargo llvm-cov --workspace --lcov --output-path lcov.info
echo "ci-check: OK"
