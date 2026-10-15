# Native Windows CI (Jenkins agent label `windows`): toolchain bootstrap, tests, release build.
# Idempotent: installs rustup into the build user's profile only when cargo is missing.
# Prerequisite (not installable without admin): MSVC build tools (link.exe) — present on the
# agent because arcana-windows builds WinUI 3 with Visual Studio there.
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
Run rustup @('toolchain', 'install', 'stable', '--profile', 'minimal')
Run rustup @('target', 'add', 'wasm32-unknown-unknown')
$wt = (& wasm-tools --version 2>$null) -join ''
if ($wt -notmatch '^wasm-tools 1\.262\.0') { Run cargo @('install', 'wasm-tools', '--version', '1.262.0', '--locked') }
Run rustc @('--version')
# Example WASM plugin component for the sandbox tests (scripts/build-wasm-example.sh).
Run cargo @('build', '--release', '--target', 'wasm32-unknown-unknown', '--manifest-path', 'plugins-wasm/example/Cargo.toml', '--target-dir', 'target/wasm-example')
New-Item -ItemType Directory -Force -Path 'target/wasm' | Out-Null
Run wasm-tools @('component', 'new', 'target/wasm-example/wasm32-unknown-unknown/release/arcana_wasm_example.wasm', '-o', 'target/wasm/example.wasm')
Run cargo @('test', '--workspace', '--locked')
Run cargo @('build', '--release', '--locked', '-p', 'arcana-app')
Get-Item 'target/release/arcana-app.exe' | Format-List Name, Length
