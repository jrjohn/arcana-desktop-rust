//! C# `ManifestService` / `ManifestServiceImpl`: load, discover and validate `plugin.json`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use arcana_plugin_api::{ActivationEventType, PluginManifest, activation_events};

/// Manifest file names, newest format first (C# checks `plugin.manifest.json` then `plugin.json`).
pub const MANIFEST_FILES: [&str; 2] = ["plugin.manifest.json", "plugin.json"];

/// C# `ManifestValidationResult`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ManifestValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Default)]
struct Inner {
    manifests: BTreeMap<String, PluginManifest>,
    paths: BTreeMap<String, PathBuf>,
}

/// C# `ManifestServiceImpl`.
#[derive(Default)]
pub struct ManifestService {
    inner: Mutex<Inner>,
}

impl std::fmt::Debug for ManifestService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManifestService")
            .field("count", &self.lock().manifests.len())
            .finish()
    }
}

/// C# `JsonCommentHandling.Skip`: drop `//` and `/* */` comments outside strings.
fn strip_comments(json: &str) -> String {
    let mut out = String::with_capacity(json.len());
    let mut chars = json.chars().peekable();
    let mut in_str = false;
    while let Some(c) = chars.next() {
        if in_str {
            out.push(c);
            if c == '\\' {
                if let Some(n) = chars.next() {
                    out.push(n);
                }
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match (c, chars.peek()) {
            ('"', _) => {
                in_str = true;
                out.push(c);
            }
            ('/', Some('/')) => {
                for n in chars.by_ref() {
                    if n == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            ('/', Some('*')) => {
                chars.next();
                let mut prev = ' ';
                for n in chars.by_ref() {
                    if prev == '*' && n == '/' {
                        break;
                    }
                    prev = n;
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// C# `AllowTrailingCommas`: remove a `,` that only precedes whitespace and `}` / `]`.
fn strip_trailing_commas(json: &str) -> String {
    let chars: Vec<char> = json.chars().collect();
    let mut out = String::with_capacity(json.len());
    let mut in_str = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_str {
            out.push(c);
            if c == '\\' && i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 1;
            } else if c == '"' {
                in_str = false;
            }
        } else if c == '"' {
            in_str = true;
            out.push(c);
        } else if c == ',' {
            let next = chars[i + 1..].iter().find(|c| !c.is_whitespace());
            if !matches!(next, Some('}' | ']')) {
                out.push(c);
            }
        } else {
            out.push(c);
        }
        i += 1;
    }
    out
}

/// Case-insensitive property names (C# `PropertyNameCaseInsensitive`): lower-camel every key
/// that differs from a known field only by case.
fn parse_lenient(json: &str) -> Option<PluginManifest> {
    let cleaned = strip_trailing_commas(&strip_comments(json));
    let value: serde_json::Value = serde_json::from_str(&cleaned).ok()?;
    serde_json::from_value(normalize_keys(value)).ok()
}

fn normalize_keys(v: serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(k, v)| {
                    let mut c = k.chars();
                    let key = c.next().map_or_else(String::new, |f| {
                        f.to_lowercase().collect::<String>() + c.as_str()
                    });
                    (key, normalize_keys(v))
                })
                .collect(),
        ),
        serde_json::Value::Array(a) => {
            serde_json::Value::Array(a.into_iter().map(normalize_keys).collect())
        }
        other => other,
    }
}

impl ManifestService {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// C# `LoadManifestAsync`: `None` on a missing file, bad JSON or missing id/name/version.
    #[must_use]
    pub fn load_manifest(&self, path: &Path) -> Option<PluginManifest> {
        let json = std::fs::read_to_string(path).ok()?;
        let Some(m) = parse_lenient(&json) else {
            tracing::warn!(path = %path.display(), "invalid plugin manifest JSON");
            return None;
        };
        if m.id.is_empty() || m.name.is_empty() || m.version.is_empty() {
            tracing::warn!(path = %path.display(), "plugin manifest missing id/name/version");
            return None;
        }
        let mut inner = self.lock();
        inner.manifests.insert(m.id.clone(), m.clone());
        inner.paths.insert(m.id.clone(), path.to_path_buf());
        Some(m)
    }

    /// The manifest file inside a plugin directory, if any.
    #[must_use]
    pub fn manifest_path(dir: &Path) -> Option<PathBuf> {
        MANIFEST_FILES
            .iter()
            .map(|f| dir.join(f))
            .find(|p| p.is_file())
    }

    /// C# `DiscoverManifestsAsync`: one manifest per sub-directory, sorted by directory name.
    #[must_use]
    pub fn discover_manifests(&self, plugins_dir: &Path) -> Vec<PluginManifest> {
        let Ok(entries) = std::fs::read_dir(plugins_dir) else {
            return Vec::new();
        };
        let mut dirs: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort();
        dirs.iter()
            .filter_map(|d| Self::manifest_path(d))
            .filter_map(|p| self.load_manifest(&p))
            .collect()
    }

    #[must_use]
    pub fn manifest(&self, plugin_id: &str) -> Option<PluginManifest> {
        self.lock().manifests.get(plugin_id).cloned()
    }

    #[must_use]
    pub fn all_manifests(&self) -> Vec<PluginManifest> {
        self.lock().manifests.values().cloned().collect()
    }

    #[must_use]
    pub fn manifest_directory(&self, plugin_id: &str) -> Option<PathBuf> {
        self.lock()
            .paths
            .get(plugin_id)
            .and_then(|p| p.parent().map(Path::to_path_buf))
    }

    /// C# `RegisterManifest`.
    pub fn register_manifest(&self, manifest: PluginManifest, base_path: Option<&Path>) {
        let mut inner = self.lock();
        if let Some(base) = base_path {
            inner
                .paths
                .insert(manifest.id.clone(), base.join(MANIFEST_FILES[0]));
        }
        inner.manifests.insert(manifest.id.clone(), manifest);
    }

    pub fn remove(&self, plugin_id: &str) {
        let mut inner = self.lock();
        inner.manifests.remove(plugin_id);
        inner.paths.remove(plugin_id);
    }

    /// C# `ShouldActivateOnStartup`: unknown plugin, no events, `onStartup` or `*`.
    #[must_use]
    pub fn should_activate_on_startup(&self, plugin_id: &str) -> bool {
        let Some(m) = self.manifest(plugin_id) else {
            return true;
        };
        let events = m.activation_events();
        events.is_empty()
            || events
                .iter()
                .any(|e| e == activation_events::ON_STARTUP || e == activation_events::STAR)
    }

    #[must_use]
    pub fn activation_events(&self, plugin_id: &str) -> Vec<String> {
        self.manifest(plugin_id)
            .map(|m| m.activation_events().to_vec())
            .unwrap_or_default()
    }

    /// C# `GetPluginsForActivationEvent`.
    #[must_use]
    pub fn plugins_for_activation_event(
        &self,
        kind: ActivationEventType,
        argument: Option<&str>,
    ) -> Vec<String> {
        self.lock()
            .manifests
            .iter()
            .filter(|(_, m)| event_matches(m.activation_events(), kind, argument))
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// C# `ValidateManifest`.
    #[must_use]
    pub fn validate_manifest(&self, m: &PluginManifest) -> ManifestValidationResult {
        validate_manifest(m)
    }
}

/// Shared matcher (C# `ActivationEventService.GetPluginsToActivate` /
/// `GetPluginsForActivationEvent`): `*` matches anything; same type matches when either
/// argument is absent or both are equal.
#[must_use]
pub fn event_matches(events: &[String], kind: ActivationEventType, argument: Option<&str>) -> bool {
    events.iter().any(|e| {
        let (t, arg) = activation_events::parse(e);
        t == ActivationEventType::Star
            || (t == kind && (arg.is_none() || argument.is_none() || arg.as_deref() == argument))
    })
}

/// .NET `Version.TryParse`: 2–4 dot-separated non-negative integers.
#[must_use]
pub fn is_dotnet_version(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    (2..=4).contains(&parts.len())
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.trim() == *p && p.parse::<u32>().is_ok())
}

/// C# `ManifestServiceImpl.ValidateManifest`.
#[must_use]
pub fn validate_manifest(m: &PluginManifest) -> ManifestValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    if m.id.is_empty() {
        errors.push("Missing required field: id".to_string());
    }
    if m.name.is_empty() {
        errors.push("Missing required field: name".to_string());
    }
    if m.version.is_empty() {
        errors.push("Missing required field: version".to_string());
    } else if !is_dotnet_version(&m.version) {
        warnings.push(format!("Invalid version format: {}", m.version));
    }
    if let Some(c) = &m.contributes {
        for v in c.views.iter().flatten() {
            if v.id.is_empty() {
                errors.push("View missing required field: id".to_string());
            }
            if v.title_key.is_empty() && v.title.as_deref().is_none_or(str::is_empty) {
                warnings.push(format!("View '{}' has no title or titleKey", v.id));
            }
        }
        for menu in c.menus.iter().flatten() {
            if menu.id.is_empty() {
                errors.push("Menu missing required field: id".to_string());
            }
            if menu.location.is_empty() {
                errors.push(format!(
                    "Menu '{}' missing required field: location",
                    menu.id
                ));
            }
        }
        for cmd in c.commands.iter().flatten() {
            if cmd.id.is_empty() {
                errors.push("Command missing required field: id".to_string());
            }
        }
    }
    for e in m.activation_events() {
        if activation_events::parse(e).0 == ActivationEventType::Unknown {
            warnings.push(format!("Unknown activation event: {e}"));
        }
    }
    ManifestValidationResult {
        is_valid: errors.is_empty(),
        errors,
        warnings,
    }
}
