# Native Windows CI (Jenkins agent label `windows`): toolchain bootstrap, tests, release build.
# Idempotent: installs rustup into the build user's profile only when cargo is missing.
# Linker: MSVC build tools (link.exe) when Visual Studio C++ tools are installed; otherwise
# (the office agent has only the .NET/WinUI workload, and installing VS C++ needs admin) the
# GNU toolchain plus a portable MinGW-w64 GCC (WinLibs, msvcrt) unpacked under the build user's
# profile. Everything lives in %USERPROFILE% (.cargo, .rustup, .arcana-ci): delete to undo.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
function Run([string]$exe, [string[]]$argv) {
    & $exe @argv
    if ($LASTEXITCODE -ne 0) { throw "$exe $($argv -join ' ') failed with exit code $LASTEXITCODE" }
}
$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
$env:PATH = "$cargoBin;$env:PATH"
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "cargo not found: installing rustup (minimal, stable) for $env:USERNAME"
    $init = Join-Path $env:TEMP 'rustup-init.exe'
    Invoke-WebRequest -Uri 'https://win.rustup.rs/x86_64' -OutFile $init
    Run $init @('-y', '--profile', 'minimal', '--default-toolchain', 'stable', '--no-modify-path')
}
# Pinned, not `stable`: `rustup toolchain install stable` silently upgraded the toolchain under
# running builds, and 2026-10-19 PR-17/18/19 failed only here with corrupt-artifact errors
# ("can't find crate for `std`", "only metadata stub found"). Bump deliberately.
# Single source of truth: rust-toolchain.toml at the repo root.
$rustVersion = ((Get-Content rust-toolchain.toml | Select-String '^channel\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value)
# The agent is a 4-core / 7.7 GB laptop (0.6 GB free observed mid-build): cap rustc parallelism.
$env:CARGO_BUILD_JOBS = '2'
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$hasMsvc = (Test-Path $vswhere) -and (& $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath)
if ($hasMsvc) {
    $toolchain = "$rustVersion-x86_64-pc-windows-msvc"
} else {
    $toolchain = "$rustVersion-x86_64-pc-windows-gnu"
    $mingwRoot = Join-Path $env:USERPROFILE '.arcana-ci'
    $gcc = Join-Path $mingwRoot 'mingw64\bin\gcc.exe'
    if (-not (Test-Path $gcc)) {
        Write-Host "no MSVC C++ tools: installing portable MinGW-w64 GCC into $mingwRoot"
        $name = 'winlibs-x86_64-posix-seh-gcc-16.1.0-mingw-w64msvcrt-14.0.0-r4.zip'
        $base = 'https://github.com/brechtsanders/winlibs_mingw/releases/download/16.1.0posix-14.0.0-msvcrt-r4'
        $zip = Join-Path $env:TEMP $name
        Invoke-WebRequest -Uri "$base/$name" -OutFile $zip
        Invoke-WebRequest -Uri "$base/$name.sha256" -OutFile "$zip.sha256"
        $want = ((Get-Content -Raw "$zip.sha256").Trim() -split '\s+')[0]
        Remove-Item "$zip.sha256"
        $got = (Get-FileHash -Algorithm SHA256 $zip).Hash
        if ($want -and ($got -ne $want.ToUpper())) { throw "MinGW zip checksum mismatch: $got vs $want" }
        New-Item -ItemType Directory -Force -Path $mingwRoot | Out-Null
        Expand-Archive -Path $zip -DestinationPath $mingwRoot -Force
        Remove-Item $zip
    }
    $env:PATH = "$(Split-Path $gcc);$env:PATH"
}
Write-Host "toolchain: $toolchain"
$env:RUSTUP_TOOLCHAIN = $toolchain
Run rustup @('toolchain', 'install', $toolchain, '--profile', 'minimal')
Run rustup @('target', 'add', 'wasm32-unknown-unknown', '--toolchain', $toolchain)
$wt = ''
if (Get-Command wasm-tools -ErrorAction SilentlyContinue) { $wt = (& wasm-tools --version) -join '' }
if ($wt -notmatch '^wasm-tools 1\.262\.0') { Run cargo @('install', 'wasm-tools', '--version', '1.262.0', '--locked') }
Run rustc @('--version')
# No `cargo clean` per run any more: it made every Windows build a 50-64 min cold compile
# (one hit the 90 min stage timeout, 2026-10-19). The corruption it guarded against
# ("can't find crate for std", E0786 invalid metadata) came from TWO builds running on this
# agent at once (fixed: numExecutors 1, arcana-devops #51). Incremental artifacts are the
# part an interrupted build can leave half-written, so turn incremental off and keep the
# reusable dependency/target cache.
$env:CARGO_INCREMENTAL = '0'
# Example WASM plugin component for the sandbox tests (scripts/build-wasm-example.sh).
Run cargo @('build', '--release', '--target', 'wasm32-unknown-unknown', '--manifest-path', 'plugins-wasm/example/Cargo.toml', '--target-dir', 'target/wasm-example')
New-Item -ItemType Directory -Force -Path 'target/wasm' | Out-Null
Run wasm-tools @('component', 'new', 'target/wasm-example/wasm32-unknown-unknown/release/arcana_wasm_example.wasm', '-o', 'target/wasm/example.wasm')
# One shared cargo target for every branch/PR workspace. Each Jenkins workspace kept its own
# ~17.6 GB target/ once per-run `cargo clean` was dropped, and 20+ of them filled C: (0.4 GB free,
# "ld.exe: final link failed: No space left on device", 2026-10-19). Safe to share: the agent runs
# one build at a time (numExecutors 1). The small wasm example stays in the workspace's target/
# because the sandbox tests load it from there (explicit --target-dir above).
$env:CARGO_TARGET_DIR = 'C:\jenkins-agent\cargo-target\arcana-desktop-rust'
New-Item -ItemType Directory -Force -Path $env:CARGO_TARGET_DIR | Out-Null
Run cargo @('test', '--workspace', '--locked')
Run cargo @('build', '--release', '--locked', '-p', 'arcana-app')
Get-Item (Join-Path $env:CARGO_TARGET_DIR 'release\arcana-app.exe') | Format-List Name, Length
