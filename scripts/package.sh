#!/usr/bin/env bash
# Build release installers into dist/. Usage: scripts/package.sh <linux|macos|windows> [version]
#   linux   → arcana-desktop_<v>_<arch>.deb (cargo-deb) + Arcana-<v>-<arch>.AppImage (linuxdeploy)
#   macos   → Arcana-<v>-<arch>.dmg containing Arcana.app (unsigned)
#   windows → arcana-desktop-<v>-x86_64.msi (cargo-wix, WiX v3; run from Git Bash)
# Tools are installed on demand (cargo install / download) so a clean runner works.
set -euo pipefail
os=${1:?linux|macos|windows}
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
version=${2:-$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)}
version=${version#v}
arch=$(uname -m)
T=${CARGO_TARGET_DIR:-target}
mkdir -p dist
cargo build --release -p arcana-app

case "$os" in
linux)
  command -v cargo-deb >/dev/null || cargo install cargo-deb --locked
  cargo deb -p arcana-app --no-build --deb-version "$version" -o "dist/arcana-desktop_${version}_$(dpkg --print-architecture).deb"
  # AppImage
  tool="target/linuxdeploy-$arch.AppImage"
  [ -x "$tool" ] || { curl -fsSL -o "$tool" "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-$arch.AppImage"; chmod +x "$tool"; }
  rm -rf target/AppDir
  APPIMAGE_EXTRACT_AND_RUN=1 LINUXDEPLOY_OUTPUT_VERSION="$version" OUTPUT="dist/Arcana-$version-$arch.AppImage" \
    "$tool" --appdir target/AppDir \
      --executable "$T/release/arcana-app" \
      --desktop-file packaging/arcana.desktop \
      --icon-file packaging/arcana.png \
      --output appimage
  ;;
macos)
  app="target/Arcana.app"
  rm -rf "$app"; mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
  cp "$T/release/arcana-app" "$app/Contents/MacOS/arcana-app"
  iconset="target/arcana.iconset"; rm -rf "$iconset"; mkdir -p "$iconset"
  for s in 16 32 128 256; do
    sips -z $s $s packaging/arcana.png --out "$iconset/icon_${s}x${s}.png" >/dev/null
    d=$((s*2)); sips -z $d $d packaging/arcana.png --out "$iconset/icon_${s}x${s}@2x.png" >/dev/null
  done
  iconutil -c icns "$iconset" -o "$app/Contents/Resources/arcana.icns"
  cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>Arcana</string>
  <key>CFBundleDisplayName</key><string>Arcana</string>
  <key>CFBundleIdentifier</key><string>com.jrjohn.arcana-desktop</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleExecutable</key><string>arcana-app</string>
  <key>CFBundleIconFile</key><string>arcana</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
  # Ad-hoc signature only (Apple Silicon refuses unsigned code); Developer ID + notarization
  # are a follow-up (README "macOS signing").
  codesign --force --deep -s - "$app"
  stage="target/dmg"; rm -rf "$stage"; mkdir -p "$stage"
  cp -R "$app" "$stage/"; ln -s /Applications "$stage/Applications"
  hdiutil create -volname "Arcana $version" -srcfolder "$stage" -ov -format UDZO "dist/Arcana-$version-$arch.dmg"
  ;;
windows)
  command -v cargo-wix >/dev/null || cargo install cargo-wix --locked
  [ -f crates/arcana-app/wix/main.wxs ] || cargo wix init -p arcana-app
  cargo wix -p arcana-app --no-build --nocapture --install-version "$version" \
    -o "dist/arcana-desktop-$version-x86_64.msi"
  ;;
*) echo "unknown os $os"; exit 2 ;;
esac
ls -l dist
