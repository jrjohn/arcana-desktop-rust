#!/usr/bin/env bash
# Start a new desktop product from this repo's full layered workspace.
#
#   tools/new-app.sh inventory ~/Documents/projects/inventory
#
# Copies the committed tree (git archive HEAD), so the new product keeps AGENTS.md,
# CLAUDE.md, tools/ (scaffolding + hook + arch-check), xtask (crate layer check), the
# Jenkinsfile and the arch-qube gate. Copying single crates into a fresh `cargo new` loses all
# of that (AGENTS.md §1).
set -euo pipefail

[ $# -eq 2 ] || { echo "usage: $0 <app-name> <dest-dir>" >&2; exit 1; }
APP="$1"
DEST="$2"
[[ "$APP" =~ ^[a-z][a-z0-9]*(-[a-z0-9]+)*$ ]] || { echo "app-name: lower-case letters/digits, words joined by '-' (it becomes crate names <app>-core, <app>_core…)" >&2; exit 1; }
[ "$APP" != arcana ] || { echo "app-name must differ from the template's 'arcana'" >&2; exit 1; }
[ ! -e "$DEST" ] || { echo "$DEST already exists — refusing to overwrite" >&2; exit 1; }
command -v perl >/dev/null || { echo "perl is required" >&2; exit 1; }

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
git -C "$ROOT" diff --quiet HEAD -- AGENTS.md tools/ || echo "note: uncommitted changes in AGENTS.md/tools are NOT copied (git archive HEAD)" >&2

mkdir -p "$DEST"
git -C "$ROOT" archive HEAD | tar -x -C "$DEST"
SRC_SHA="$(git -C "$ROOT" rev-parse --short HEAD)"
APP_US="$(printf '%s' "$APP" | tr '-' '_')"
# Display name: "inventory" -> "Inventory", "my-shop" -> "My Shop"
TITLE="$(printf '%s' "$APP" | perl -pe 's/(^|-)([a-z])/($1?" ":"").uc($2)/ge')"
cd "$DEST"

# ---- Product identity. Every place the template name is load-bearing:
#  1. package names  arcana-<x> (Cargo.toml, Cargo.lock, -p flags, scripts, .github) and their
#     Rust paths arcana_<x> (use statements); the example WASM plugin arcana-wasm-example;
#     the binary arcana-app; the .deb / .msi name arcana-desktop.
#  2. app identity: window title, "app.name" string, data directory <local data>/Arcana,
#     macOS bundle name + identifier, .desktop entry and icon file names.
#  3. CI identities: Sonar key/name (desktop-rust-app), CI image tags arcana-desktop-rust-ci/-gui,
#     per-build container names desktop-ci-/desktop-gui-/arcana-arch-qube-desktop- — two
#     products on one Jenkins host must not share them. (There is no APP_NAME variable in this
#     Jenkinsfile.) The lock name ci-rust-build is NOT renamed: it is the host-wide memory guard
#     shared by every Rust repo on bluesea.
# Not renamed on purpose: the crate DIRECTORIES crates/arcana-*/ (see below), the ARCANA_* dev
# env switches (ARCANA_SMOKE_SCREENSHOT…, read by scripts/gui-smoke.sh), C# lineage in doc
# comments, and arcana.boo (the registry / arch-qube image host).
#
# Why the directories keep their names: arch-qube's rust-desktop profile identifies the layers
# AND the composition roots (di_container_files **/arcana-infra/src/wiring.rs,
# **/arcana-app/src/{main,bootstrap}.rs) by directory. arch-qube.yaml `layer_paths` can re-map
# layers, but not the composition roots, so a renamed directory turns every legitimate *Impl
# wiring into a CRITICAL "Only DI Container Imports Impl" violation (measured 2026-11-24).
PKGS="$(perl -ne 'print "$1\n" if /^name\s*=\s*"arcana-([a-z0-9-]+)"/' crates/*/Cargo.toml plugins/*/Cargo.toml plugins-wasm/*/Cargo.toml | sort -u)"
[ -n "$PKGS" ] || { echo "new-app: no arcana-* packages found" >&2; exit 1; }
# tools/ is excluded: these scripts describe the template and must keep working in the product.
TEXT_FILES="$(grep -rIl -e arcana -e Arcana -e desktop-rust-app -e 'Desktop Rust App' -e desktop-ci- -e desktop-gui- . \
    --exclude-dir=.git --exclude-dir=tools --exclude=CHANGELOG.md || true)"
for c in $PKGS; do
    c_us="$(printf '%s' "$c" | tr '-' '_')"
    # (?<![\w.-]) / (?![\w-]) keep arcana.boo, arcana-desktop-rust and crates/arcana-x/ paths intact:
    # a path segment "crates/arcana-ui/" is preceded by "/" — renamed separately below? No: the
    # directory keeps its name, so only a token NOT preceded by "/" is a package name.
    printf '%s\n' "$TEXT_FILES" | xargs perl -pi -e "s{(?<![\\w./-])arcana-${c}(?![\\w-])}{${APP}-${c}}g; s{(?<![\\w])arcana_${c_us}(?!\\w)}{${APP_US}_${c_us}}g"
done
printf '%s\n' "$TEXT_FILES" | xargs perl -pi -e "
    s/arcana-desktop-rust-(ci|gui):/${APP}-\$1:/g;
    s/\"desktop-(ci|gui)-\\\$CI_ID\"/\"${APP}-\$1-\\\$CI_ID\"/g;
    s/arcana-arch-qube-desktop-/${APP}-arch-qube-/g;
    s{(release|debug|MacOS)/arcana-app\b}{\$1/${APP}-app}g;
    s/desktop-rust-app/${APP}/g; s/Desktop Rust App/${TITLE}/g;
    s/com\\.jrjohn\\.arcana-desktop/com.example.${APP}/g;
    s/\"Arcana - Enterprise Management System\"/\"${TITLE}\"/g;
    s/\\{title\\} — Arcana/{title} — ${TITLE}/g;
    s/\"app\\.name\": \"Arcana\"/\"app.name\": \"${TITLE}\"/g;
    s/(join|ends_with)\\(\"Arcana\"\\)/\$1(\"${TITLE}\")/g;
    s{repository = \"https://github.com/jrjohn/arcana-desktop-rust\"}{repository = \"https://github.com/jrjohn/${APP}\"}g;
"
# Packaging: bundle / launcher names and the icon + .desktop file names.
perl -pi -e "s/\\bArcana\\b/${TITLE}/g; s{packaging/arcana\\.}{packaging/${APP}.}g; s/\\barcana\\.(iconset|icns)/${APP}.\$1/g; s{<string>arcana</string>}{<string>${APP}</string>}g" scripts/package.sh
perl -pi -e "s/\\bArcana\\b/${TITLE}/g; s/^Icon=arcana\$/Icon=${APP}/" packaging/arcana.desktop
perl -pi -e "s{packaging/arcana\\.}{packaging/${APP}.}g" crates/arcana-app/Cargo.toml
mv packaging/arcana.desktop "packaging/${APP}.desktop"
mv packaging/arcana.png "packaging/${APP}.png"

# Fail loudly if a package-name token survived (same token rule as the rename above).
LEFT="$(grep -rIl --exclude-dir=.git --exclude-dir=tools --exclude=CHANGELOG.md -e 'arcana[-_]' . \
    | xargs perl -ne 'print "$ARGV:$.: $_" if /(?<![\w.\/-])arcana[-_](core|domain|data|sync|infra|ui|app|plugin[-_](api|host|wasm|flowchart)|wasm[-_]example|desktop)(?![\w-])/; close ARGV if eof' || true)"
if [ -n "$LEFT" ]; then
    echo "new-app: template package names still present after rename:" >&2
    printf '%s\n' "$LEFT" | head >&2
    exit 1
fi

# ---- Fresh history: version 0.1.0, empty changelog.
rm -f CHANGELOG.md
printf '0.1.0\n' > version.txt
printf '{\n  ".": "0.1.0"\n}\n' > .release-please-manifest.json
perl -0pi -e 's/(\[workspace\.package\]\nversion = )"[^"]*"/$1"0.1.0"/' Cargo.toml

cat >> AGENTS.md <<EOT

## Origin

Created from arcana-desktop-rust @ \`${SRC_SHA}\` by \`tools/new-app.sh\` (packages renamed
\`arcana-*\` → \`${APP}-*\`, Rust paths \`arcana_*\` → \`${APP_US}_*\`; crate directories keep
their \`crates/arcana-*\` names on purpose — arch-qube identifies layers and composition roots
by them). The layers, rules and definition of done above apply unchanged. Remove template
features you do not need through \`wiring.rs\` / \`pages/mod.rs\`, not by flattening.
EOT

# Lock file follows the renamed packages (offline: no new dependencies are resolved).
cargo metadata --format-version 1 --offline >/dev/null 2>&1 || cargo metadata --format-version 1 >/dev/null

git init -q
tools/install-hooks.sh >/dev/null
git add -A
# --no-verify only for this import commit: every file counts as "added" here, so the hook
# would judge the template's own legacy files (crate-root modules such as
# crates/arcana-infra/src/services.rs, Impl factories in arcana-data/src/dao.rs) as new
# violations. The template itself passes arch-check; every later commit goes through the hook.
git -c user.name="${GIT_AUTHOR_NAME:-new-app}" -c user.email="${GIT_AUTHOR_EMAIL:-new-app@localhost}" \
    commit -q --no-verify -m "chore: start ${APP} from arcana-desktop-rust ${SRC_SHA}"

echo "created $DEST (${APP}) from arcana-desktop-rust ${SRC_SHA}; hooks: $(git config core.hooksPath)"
echo "next: cd $DEST && tools/new-feature.sh <Name>   # one vertical slice per feature"
