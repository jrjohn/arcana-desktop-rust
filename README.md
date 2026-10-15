# arcana-desktop-rust

Rust + egui port of [arcana-windows](https://github.com/jrjohn/arcana-windows): Clean Architecture,
MVVM-UDF, plugin system (built-in + sandboxed WASM), CRDT offline sync. Runs on Windows, macOS, Linux.

Plan and milestones: [docs/PORTING_PLAN.md](docs/PORTING_PLAN.md).

```bash
cargo run                    # desktop app
bash scripts/ci-check.sh     # the CI gate: fmt, layers, clippy pedantic, tests + coverage
cargo xtask layers           # dependency-direction check only
```

## Run / build per OS

Rust stable (`rustup`, edition 2024) on every OS. Release binary: `cargo build --release -p arcana-app`
→ `target/release/arcana-app[.exe]`. Data (SQLite, logs, plugins) goes to the OS data directory.

| OS | Prerequisites | Notes |
|---|---|---|
| Windows 10/11 | MSVC build tools (Visual Studio "Desktop development with C++") | CJK font: Microsoft JhengHei → Yu Gothic UI → Microsoft YaHei (built in) |
| macOS 11+ | Xcode Command Line Tools (`xcode-select --install`) | CJK font: PingFang TC → Hiragino Sans (built in) |
| Linux (X11 or Wayland) | see below | CJK font: Noto Sans CJK → WenQuanYi; install one or zh-TW/ja-JP shows boxes (a status-bar warning says so) |

Linux system packages (Debian/Ubuntu names; build and run):

```bash
sudo apt-get install build-essential pkg-config libgl1-mesa-dev libegl1-mesa-dev \
  libxkbcommon-dev libwayland-dev libxcursor1 libxrandr2 libxi6 libx11-xcb1 fonts-noto-cjk
```

File dialogs use the XDG desktop portal on Linux (`xdg-desktop-portal` + a backend such as
`-gtk`/`-kde`); without a portal the app falls back to typing a path.

## Install (release artifacts)

Each GitHub Release (cut by release-please) carries installers built by
`.github/workflows/package.yml` / `scripts/package.sh`:

- **Windows** `arcana-desktop-<v>-x86_64.msi` (cargo-wix / WiX v3) — double-click; unsigned, so
  SmartScreen asks once ("More info → Run anyway").
- **macOS** `Arcana-<v>-arm64.dmg` — drag Arcana.app to Applications. Ad-hoc signed only: first
  launch via right-click → Open (or `xattr -dr com.apple.quarantine /Applications/Arcana.app`).
- **Linux** `arcana-desktop_<v>_amd64.deb` (`sudo apt install ./arcana-desktop_*.deb`) or
  `Arcana-<v>-x86_64.AppImage` (`chmod +x` and run; needs `libfuse2`).

Local: `bash scripts/package.sh <linux|macos|windows> [version]` → `dist/`.

**Follow-ups (not done):** macOS Developer ID signing + notarization (`codesign --options runtime`
with a Developer ID cert, `xcrun notarytool submit --wait`, `xcrun stapler staple` on the .dmg —
needs an Apple Developer account secret in CI); Windows Authenticode signing of the .msi.

## CI

Jenkins (`Jenkinsfile`) runs three platforms in parallel; any red fails the build:
Linux in Docker on bluesea (`Dockerfile.ci --target ci` = `scripts/ci-check.sh`, then
`--target gui` = `scripts/gui-smoke.sh`: the real app under Xvfb and headless Weston, screenshot
must not be blank, then SonarQube), Windows agent (`scripts/ci-windows.ps1`) and Mac mini
(`scripts/ci-native.sh`) run `cargo test --workspace` + a release build, installing rustup for the
build user first if cargo is missing.
