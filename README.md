# arcana-desktop-rust

Rust + egui port of [arcana-windows](https://github.com/jrjohn/arcana-windows): Clean Architecture,
MVVM-UDF, plugin system (built-in + sandboxed WASM), CRDT offline sync. Runs on Windows, macOS, Linux.

Plan and milestones: [docs/PORTING_PLAN.md](docs/PORTING_PLAN.md).

```bash
cargo run                    # desktop app
bash scripts/ci-check.sh     # the CI gate: fmt, layers, clippy pedantic, tests + coverage
cargo xtask layers           # dependency-direction check only
```
