#!/usr/bin/env bash
# GUI smoke test (Linux, run inside the Dockerfile.ci `gui` stage): start the real app on
# X11 (Xvfb) and on Wayland (weston headless), let it present frames, grab the framebuffer
# (ARCANA_SMOKE_SCREENSHOT, see crates/arcana-app/src/smoke.rs), assert it is not blank and
# that the app exits cleanly. Screenshots land in smoke-out/.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -p arcana-app --locked
BIN="$PWD/target/debug/arcana-app"
OUT="$PWD/smoke-out"; mkdir -p "$OUT"
export LIBGL_ALWAYS_SOFTWARE=1
run_app() { # name, then env assignments
  local name=$1; shift
  local home; home=$(mktemp -d)
  echo "== $name"
  set +e
  env HOME="$home" ARCANA_SMOKE_SCREENSHOT="$OUT/$name.ppm" "$@" timeout 180 "$BIN"
  local rc=$?
  set -e
  echo "$name exit code: $rc"
  [ "$rc" -eq 0 ] || return "$rc"
  convert "$OUT/$name.ppm" "$OUT/$name.png"
  local colours; colours=$(identify -format '%k' "$OUT/$name.png")
  echo "$name: $(identify -format '%wx%h' "$OUT/$name.png"), $colours colours"
  [ "$colours" -ge 16 ] || { echo "$name: blank frame"; return 1; }
}

# X11
Xvfb :99 -screen 0 1440x900x24 -nolisten tcp &
XPID=$!
for _ in $(seq 50); do [ -e /tmp/.X11-unix/X99 ] && break; sleep 0.1; done
run_app x11 DISPLAY=:99 WAYLAND_DISPLAY= XDG_SESSION_TYPE=x11
kill "$XPID" || true

# Wayland
export XDG_RUNTIME_DIR; XDG_RUNTIME_DIR=$(mktemp -d); chmod 700 "$XDG_RUNTIME_DIR"
weston --backend=headless-backend.so --use-pixman --width=1440 --height=900 --socket=wayland-smoke --idle-time=0 &
WPID=$!
for _ in $(seq 100); do [ -S "$XDG_RUNTIME_DIR/wayland-smoke" ] && break; sleep 0.1; done
run_app wayland WAYLAND_DISPLAY=wayland-smoke DISPLAY= XDG_SESSION_TYPE=wayland WINIT_UNIX_BACKEND=wayland
kill "$WPID" || true
echo "gui-smoke: OK"
