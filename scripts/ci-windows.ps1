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
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$hasMsvc = (Test-Path $vswhere) -and (& $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath)
if ($hasMsvc) {
    $toolchain = 'stable-x86_64-pc-windows-msvc'
} else {
    $toolchain = 'stable-x86_64-pc-windows-gnu'
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
# This agent is a persistent native Windows box (no container, no ephemeral workspace):
# an interrupted build (agent disconnect mid-compile) leaves corrupt incremental .rmeta
# files in target/, which every later build then reuses and fails on (E0786 invalid
# metadata) regardless of source changes. Start each run from a known-clean target/.
Run cargo @('clean')
# Example WASM plugin component for the sandbox tests (scripts/build-wasm-example.sh).
Run cargo @('build', '--release', '--target', 'wasm32-unknown-unknown', '--manifest-path', 'plugins-wasm/example/Cargo.toml', '--target-dir', 'target/wasm-example')
New-Item -ItemType Directory -Force -Path 'target/wasm' | Out-Null
Run wasm-tools @('component', 'new', 'target/wasm-example/wasm32-unknown-unknown/release/arcana_wasm_example.wasm', '-o', 'target/wasm/example.wasm')
Run cargo @('test', '--workspace', '--locked')
Run cargo @('build', '--release', '--locked', '-p', 'arcana-app')
Get-Item 'target/release/arcana-app.exe' | Format-List Name, Length
