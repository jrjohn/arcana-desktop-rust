# AGENTS.md — arcana-desktop-rust

Instructions for any AI coding agent (Claude Code, Codex, Cursor, Copilot, Gemini…) working
in this repository **or in any product created from it**. `CLAUDE.md`, `GEMINI.md` and
`.github/copilot-instructions.md` all point here; this file is the single source.

> **You MUST NOT report a task as done until `tools/arch-check.sh` exits 0 and
> `bash scripts/ci-check.sh` exits 0 (§5).** Breaking the layering costs a red build: the
> pre-commit hook (`tools/githooks/pre-commit`) rejects new flat files and `*Impl` references
> outside the composition root, `cargo xtask layers` fails a forbidden crate dependency, and
> Jenkins stage "Architecture Qube" fails the merge below score 90 or on any critical violation.

## 0. First command in a fresh clone

Run `tools/install-hooks.sh` once. Git does not install hooks on clone; until you run it the
pre-commit check does not exist and only CI catches a misplaced file — after the work is
written. `tools/new-app.sh` runs it for you.

## 1. Starting something new

| Situation | Do this | Never do this |
|---|---|---|
| New desktop product | `tools/new-app.sh <name> <dest-dir>` — copies the whole workspace, renames packages `arcana-*` → `<name>-*` (Rust paths `arcana_*` → `<name>_*`), binary, window title, data directory, bundle id, Sonar key and CI image/container names, resets version to 0.1.0, installs the hook | `cargo new` and copy a few crates or files into it |
| New business feature (entity + CRUD + page) | `tools/new-feature.sh <Name>` — one vertical slice: entity, service trait, migration + table mapping, repository trait + `SQLite` impl, service impl, wiring, list page (view model + task + view), `UiServices` field, tests | Put SQL in a service, call a service from `view.rs`, or add `foo.rs` next to `lib.rs` |
| Show the new page in the shell | Route it like `ProductList`: a `Route` variant + `NavGraph::to_*` in `crates/arcana-ui/src/nav.rs`, then the `Page` / `PageMsg` / `PageTask` arms in `crates/arcana-ui/src/pages/mod.rs` | Open a window or tab from the view directly |
| New plugin | Mirror `plugins/flowchart` (depends on `arcana-core` + `arcana-plugin-api` only) and add it to the table in `xtask/src/main.rs` | Let a plugin depend on `arcana-infra` or `arcana-data` |

Copying parts instead of `new-app.sh` loses this file, the hook and the CI gates, so nothing
tells the next agent the rules exist. In a product the crate **directories** keep their
`crates/arcana-*` names on purpose (arch-qube finds layers and composition roots by them);
only package names and Rust paths change.

## 2. Layers — where a file goes

One crate per layer. Paths are directories; "may depend on" is the allow-list in
`xtask/src/main.rs` (`cargo xtask layers` fails anything else).

| Layer | Crate / path | Contains | May depend on |
|---|---|---|---|
| Core | `crates/arcana-core` | `AppError`/`AppResult`, `PageRequest`/`PagedResult`, `Repository<T>` + unit-of-work traits, security + network traits | no workspace crate |
| Domain | `crates/arcana-domain/src/entities/<x>.rs`, `src/services/<x>.rs` (or `services.rs`), `validation.rs` | entities (embed `BaseEntity`), service **traits**, validators | core |
| Data | `crates/arcana-data`: `migrations/`, `tables.rs` (`base_table!` mapping), `dao.rs`, `repository/<x>.rs`, `unit_of_work.rs`, `db.rs` | repository traits + `XRepositoryImpl`, DAO traits + impls, all SQL (`sqlx`) | core, domain |
| Sync | `crates/arcana-sync` | CRDTs, vector clocks, sync queue | core, data |
| Plugin API | `crates/arcana-plugin-api` | plugin contract, manifest, plugin MVVM types | core |
| Plugin sandbox / host | `crates/arcana-plugin-wasm`, `crates/arcana-plugin-host` | WASM sandbox; plugin loader + in-process host services | wasm: core · host: core, plugin-api, plugin-wasm |
| Infra (services) | `crates/arcana-infra/src/services/<x>.rs` (or `services.rs`), `security/`, `settings.rs`, `localization.rs`, `plugin_runtime.rs`, **`wiring.rs`** | `XServiceImpl`s, auth, settings, l10n; `wiring.rs` = the composition root (`ArcanaServices`) | core, domain, data, sync, plugin-api, plugin-host |
| UI (view model + view) | `crates/arcana-ui/src/pages/<page>/{mod.rs,task.rs,view.rs}`, `shell/`, `shell_vm/`, `nav.rs`, `effect.rs`, `widgets.rs`, `services.rs` (`UiServices`) | `mod.rs`: State/Input/`on(Input) -> Vec<Effect>`; `task.rs`: async service calls; `view.rs`: egui drawing only | core, domain, infra, plugin-api, plugin-host, plugin-flowchart |
| App | `crates/arcana-app/src/{main.rs,bootstrap.rs}` | process entry, opens the database, builds `ArcanaServices` + `UiServices` | ui, infra |
| Plugins | `plugins/flowchart`, `plugins-wasm/example` | built-in / example plugins | core, plugin-api |

Only `arcana-ui`, `arcana-app` and `arcana-plugin-flowchart` may use `egui`/`eframe`/`rfd`;
only `arcana-data` may use `sqlx`. Worked example of every layer: the Customer feature —
`pages/customer.rs` (list/form) → `UiServices.customers: Arc<dyn CustomerService>` →
`CustomerServiceImpl` (`arcana-infra/src/services.rs`) → `Arc<dyn CustomerRepository>` →
`CustomerRepositoryImpl` + `CustomerDao` (`arcana-data`), wired in `arcana-infra/src/wiring.rs`.

## 3. Rules

| # | Rule | Enforced by |
|---|---|---|
| R1 | Crate dependencies follow the table in §2; a new crate must be added to `xtask/src/main.rs` or the check fails. | `cargo xtask layers` (in `scripts/ci-check.sh`); arch-qube "Layer Direction Enforcement" |
| R2 | Only the composition roots (`crates/arcana-infra/src/wiring.rs`, `crates/arcana-app/src/{main,bootstrap}.rs`) import or construct a concrete `*Impl`. Everyone else holds `Arc<dyn Trait>`. Also allowed: the file that defines `struct XImpl`, `pub use` re-exports in `lib.rs`/`mod.rs`, files under `tests/`. | arch-qube "Only DI Container Imports Impl"; pre-commit hook |
| R3 | Every service and repository is a trait (`XService` in domain, `XRepository` in data) implemented by `XServiceImpl` / `XRepositoryImpl`. | arch-qube "Interface/Impl Colocation", "Implementation Naming Convention" |
| R4 | MVVM-UDF: `view.rs` reads `State` and returns `Vec<Input>`; it never calls a service. The view model's `on(Input)` mutates state and returns `Effect`s — no egui, no I/O. Async work lives in `task.rs` and comes back as an `Input`. | arch-qube "View Cannot Call Service", "Unidirectional Data Flow", "No Business Logic in Boundary"; "MVVM I/O/E Structure" is AI-only (not evaluated with `--no-ai`) — review it yourself |
| R5 | Navigation goes through `NavGraph::to_*` / `Route` (`crates/arcana-ui/src/nav.rs`), never ad-hoc tab or window creation in a view. | arch-qube "Type-Safe NavGraph" |
| R6 | A new `.rs` file goes in a layer directory (`crates/<c>/src/<module>/…`, `tests/`, `benches/`, `examples/`, `src/bin/`), never next to `lib.rs`. | pre-commit hook |
| R7 | SQL only in `arcana-data`, built at runtime with `sqlx::query` and `?` binds (no `query!` macros: builds need no `DATABASE_URL`). A schema change is a new file in `crates/arcana-data/migrations/`, never an edit of an applied one. | `cargo xtask layers` (sqlx only in data); migrations — review |
| R8 | `cargo fmt` clean; clippy `all` + `pedantic` clean with `-D warnings`. Where a lint is wrong for one item, `#[allow(clippy::<lint>, reason = "...")]` on that item; never relax `[workspace.lints]`. `unsafe_code` is `forbid`. | `scripts/ci-check.sh` (Jenkins stage "Check"); compiler |
| R9 | Every new service, repository and view model has tests (repository/service: `crates/<c>/tests/` on `Db::in_memory()`; view model: `#[cfg(test)]` in its `mod.rs`), as `new-feature.sh` generates. | arch-qube "Test Coverage Threshold"; SonarQube quality gate (Jenkins stage "SonarQube Analysis") |

## 4. Architecture check (arch-qube)

arch-qube (https://github.com/jrjohn/arcana-arch-qube) scans the source for layer-direction,
DI, MVVM and naming violations and scores it; usage guide:
https://github.com/jrjohn/arcana-skills/tree/main/arch-qube-skill. The version is pinned in
`tools/arch-qube.version` (0.5.6), read by both `tools/arch-check.sh` and the Jenkinsfile.

- Run: `tools/arch-check.sh`. It uses `ghcr.io/jrjohn/arch-qube:<version>` (public,
  multi-arch), then `arcana.boo/arcana/arch-qube:<version>`, with the same
  `--framework rust-desktop`, `--threshold 90` and exclusions as Jenkins stage
  "Architecture Qube". Exit 0 = pass, 1 = fail, 2 = could not run (no Docker / neither image
  pullable) — 2 is **not** a pass. There is no `:latest` fallback on purpose.
- Without Docker: on a tree without `target/`, run
  `pipx run --spec git+https://github.com/jrjohn/arcana-arch-qube.git@v0.5.6 arch-qube scan . --framework rust-desktop --no-ai --ci --format json,markdown -o arch-qube-reports --threshold 90`.
- Report: `arch-qube-reports/arch-qube.md` (summary) and `arch-qube.json` (per rule:
  `files_checked`, violations). PASS needs the score ≥ 90 **and** zero violations in every
  critical rule — a 99.4 can still FAIL. Read it rule by rule:
  - a rule under "Not evaluated" (here: MVVM I/O/E Structure, MVVM I/O/E Models Binding,
    4-Layer Progressive Cache, Offline-First, Defense-in-Depth — AI-only with `--no-ai`) is
    **not** a pass for code you touched; review it by hand against §3;
  - a rule with `files_checked: 0` checked nothing.
- `--ci` prints nothing on failure. Rerun without `--ci` (or open `arch-qube.md`) to see why.
- False positive: open an issue on arcana-arch-qube with the report attached. Never lower
  `--threshold`, add exclusions, or move code to dodge a rule.

## 5. Before you say "done"

Run these, in this order, and paste the last line of each into your summary:

```bash
tools/arch-check.sh          # must print "arch-check: PASS"
bash scripts/ci-check.sh     # the Jenkins "Check" lane; must print "ci-check: OK"
```

`scripts/ci-check.sh` is exactly what Jenkins stage "Check" runs (in `Dockerfile.ci --target ci`):

```bash
cargo fmt --all -- --check
cargo xtask layers
cargo clippy --workspace --all-targets -- -D warnings      # all + pedantic via [workspace.lints]
bash scripts/build-wasm-example.sh                          # the sandbox tests load it
cargo llvm-cov --workspace --lcov --output-path lcov.info   # runs every test
```

Needs `cargo-llvm-cov` and `wasm-tools 1.262.0`; without them run the identical lane in
Docker: `docker build -f Dockerfile.ci --target ci -t desktop-ci . && docker run --rm desktop-ci`.
The Windows and macOS lanes (`scripts/ci-windows.ps1`, `scripts/ci-native.sh`) run
`cargo test --workspace --locked` + a release build; the GUI smoke lane runs the real app under
X11 and Wayland (`Dockerfile.ci --target gui`). CI also runs the SonarQube quality gate, which
you cannot run locally.

## 6. Reference

- No desktop-specific developer skill exists. The nearest, **optional** reference for Rust
  conventions (error handling, traits behind `Arc<dyn …>`, clippy pedantic, testing) is
  `arcana-rust-developer-skill`
  (https://github.com/jrjohn/arcana-skills/tree/main/arcana-rust-developer-skill). It describes
  a **server** template: its Axum/Tonic controllers, gRPC, Shaku DI, MySQL DAOs, Redis job
  queue and deployment modes do not apply here — this workspace has egui pages, a hand-built
  `wiring.rs`, `SQLite` via sqlx and a WASM plugin host instead.
- This file is the source of truth: if the skill and this file disagree, this file wins.
