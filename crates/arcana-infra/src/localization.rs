//! Localization (C# `LocalizationServiceImpl` and the `LocalizationService` contract from
//! `Arcana.Plugins.Contracts`).
//!
//! The contract lives here until `arcana-plugin-api` gets its real content in milestone 5.
//! Core strings for en-US / zh-TW / ja-JP are the C# `Resources/Strings/*.json` files,
//! embedded at compile time; [`LocalizationServiceImpl::with_resources_dir`] loads them from
//! disk instead (the C# `resourcesPath` constructor argument).

use std::collections::{BTreeMap, HashMap};
use std::fmt::Display;
use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError, RwLock};

use crate::locale::{FALLBACK_CULTURE, SUPPORTED_CULTURES, best_supported, system_locale};

type Strings = HashMap<String, String>;

const EMBEDDED: [(&str, &str); 3] = [
    ("en-US", include_str!("../resources/strings/en-US.json")),
    ("zh-TW", include_str!("../resources/strings/zh-TW.json")),
    ("ja-JP", include_str!("../resources/strings/ja-JP.json")),
];

/// Payload of a culture change (C# `CultureChangedEventArgs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CultureChanged {
    pub old_culture: String,
    pub new_culture: String,
}

pub type CultureChangedHandler = Box<dyn Fn(&CultureChanged) + Send + Sync>;

/// C# `LocalizationService`. `params object[] args` overloads become the `_with` methods.
pub trait LocalizationService: Send + Sync {
    fn current_culture(&self) -> String;
    fn available_cultures(&self) -> Vec<String>;
    fn get(&self, key: &str) -> String;
    fn get_with(&self, key: &str, args: &[&dyn Display]) -> String;
    fn get_for_plugin(&self, plugin_id: &str, key: &str) -> String;
    fn get_for_plugin_with(&self, plugin_id: &str, key: &str, args: &[&dyn Display]) -> String;
    fn set_culture(&self, culture_name: &str);
    fn register_plugin_resources(
        &self,
        plugin_id: &str,
        culture_name: &str,
        resources: HashMap<String, String>,
    );
    fn get_from_any_plugin(&self, key: &str) -> String;
    fn on_culture_changed(&self, handler: CultureChangedHandler);
}

/// .NET `string.Format` for positional placeholders: `{0}`, `{1,10}`, `{0:N2}` (alignment and
/// format spec are ignored), `{{` / `}}` escapes. `None` where .NET throws `FormatException`
/// (unknown index, unbalanced brace).
#[must_use]
pub fn format_template(template: &str, args: &[&dyn Display]) -> Option<String> {
    let mut out = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push('}');
            }
            '{' => {
                let mut spec = String::new();
                loop {
                    match chars.next()? {
                        '}' => break,
                        ch => spec.push(ch),
                    }
                }
                let index: usize = spec.split([',', ':']).next()?.trim().parse().ok()?;
                out.push_str(&args.get(index)?.to_string());
            }
            '}' => return None,
            other => out.push(other),
        }
    }
    Some(out)
}

fn format_or_template(template: String, args: &[&dyn Display]) -> String {
    format_template(&template, args).unwrap_or(template)
}

fn lookup(map: &HashMap<String, Strings>, culture: &str, key: &str) -> Option<String> {
    if key.is_empty() {
        return None;
    }
    map.get(culture)
        .and_then(|r| r.get(key))
        .or_else(|| {
            (culture != FALLBACK_CULTURE)
                .then(|| map.get(FALLBACK_CULTURE).and_then(|r| r.get(key)))
                .flatten()
        })
        .cloned()
}

/// C# `LocalizationServiceImpl`.
pub struct LocalizationServiceImpl {
    core: HashMap<String, Strings>,
    plugins: RwLock<BTreeMap<String, HashMap<String, Strings>>>,
    current: RwLock<String>,
    handlers: Mutex<Vec<CultureChangedHandler>>,
}

impl std::fmt::Debug for LocalizationServiceImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalizationServiceImpl")
            .field("current", &self.current_culture())
            .finish_non_exhaustive()
    }
}

impl Default for LocalizationServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalizationServiceImpl {
    /// Embedded core strings; culture from the system (two-letter match), else en-US.
    #[must_use]
    pub fn new() -> Self {
        let core = EMBEDDED
            .iter()
            .map(|(c, json)| ((*c).to_owned(), parse(c, json).unwrap_or_default()))
            .collect();
        Self::with_core(core)
    }

    /// Core strings from `<dir>/<culture>.json`; a missing file gives an empty table and an
    /// unreadable one is logged and skipped, as in C#.
    #[must_use]
    pub fn with_resources_dir(dir: &Path) -> Self {
        let mut core = HashMap::new();
        for culture in SUPPORTED_CULTURES {
            let file = dir.join(format!("{culture}.json"));
            match std::fs::read_to_string(&file) {
                Ok(text) => {
                    if let Some(strings) = parse(culture, &text) {
                        core.insert(culture.to_owned(), strings);
                    }
                }
                Err(_) => {
                    core.insert(culture.to_owned(), Strings::new());
                }
            }
        }
        Self::with_core(core)
    }

    fn with_core(core: HashMap<String, Strings>) -> Self {
        // C# matches the system culture by two-letter language only.
        let sys = system_locale();
        let lang = sys
            .as_deref()
            .and_then(|s| s.split('-').next())
            .unwrap_or_default();
        let current = best_supported(Some(lang));
        Self {
            core,
            plugins: RwLock::new(BTreeMap::new()),
            current: RwLock::new(current.to_owned()),
            handlers: Mutex::new(Vec::new()),
        }
    }

    fn culture(&self) -> String {
        self.current
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn handlers(&self) -> MutexGuard<'_, Vec<CultureChangedHandler>> {
        self.handlers.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn plugin_string(&self, plugin_id: &str, culture: &str, key: &str) -> Option<String> {
        let plugins = self.plugins.read().unwrap_or_else(PoisonError::into_inner);
        plugins.get(plugin_id).and_then(|m| lookup(m, culture, key))
    }

    /// Number of core strings loaded for `culture` (diagnostics).
    #[must_use]
    pub fn core_count(&self, culture: &str) -> usize {
        self.core.get(culture).map_or(0, HashMap::len)
    }
}

fn parse(culture: &str, json: &str) -> Option<Strings> {
    match serde_json::from_str::<Strings>(json) {
        Ok(s) => {
            tracing::debug!("Loaded {} core resources for culture {culture}", s.len());
            Some(s)
        }
        Err(e) => {
            tracing::error!("Failed to load resources for culture {culture}: {e}");
            None
        }
    }
}

impl LocalizationService for LocalizationServiceImpl {
    fn current_culture(&self) -> String {
        self.culture()
    }

    fn available_cultures(&self) -> Vec<String> {
        SUPPORTED_CULTURES.iter().map(|c| (*c).to_owned()).collect()
    }

    fn get(&self, key: &str) -> String {
        if key.is_empty() {
            tracing::warn!("Localization key is null or empty");
            return String::new();
        }
        lookup(&self.core, &self.culture(), key).unwrap_or_else(|| key.to_owned())
    }

    fn get_with(&self, key: &str, args: &[&dyn Display]) -> String {
        format_or_template(self.get(key), args)
    }

    fn get_for_plugin(&self, plugin_id: &str, key: &str) -> String {
        if key.is_empty() {
            tracing::warn!("GetForPlugin called with empty key for plugin {plugin_id}");
            return String::new();
        }
        self.plugin_string(plugin_id, &self.culture(), key)
            .unwrap_or_else(|| self.get(key))
    }

    fn get_for_plugin_with(&self, plugin_id: &str, key: &str, args: &[&dyn Display]) -> String {
        format_or_template(self.get_for_plugin(plugin_id, key), args)
    }

    fn set_culture(&self, culture_name: &str) {
        if !SUPPORTED_CULTURES.contains(&culture_name) {
            tracing::warn!("Culture not available: {culture_name}");
            return;
        }
        let old = {
            let mut cur = self.current.write().unwrap_or_else(PoisonError::into_inner);
            if *cur == culture_name {
                return;
            }
            std::mem::replace(&mut *cur, culture_name.to_owned())
        };
        tracing::info!("Culture changed from {old} to {culture_name}");
        let e = CultureChanged {
            old_culture: old,
            new_culture: culture_name.to_owned(),
        };
        for h in self.handlers().iter() {
            h(&e);
        }
    }

    fn register_plugin_resources(
        &self,
        plugin_id: &str,
        culture_name: &str,
        resources: HashMap<String, String>,
    ) {
        tracing::debug!(
            "Registered {} resources for plugin {plugin_id} culture {culture_name}",
            resources.len()
        );
        self.plugins
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(plugin_id.to_owned())
            .or_default()
            .insert(culture_name.to_owned(), resources);
    }

    fn get_from_any_plugin(&self, key: &str) -> String {
        if key.is_empty() {
            tracing::warn!("GetFromAnyPlugin called with empty key");
            return String::new();
        }
        let culture = self.culture();
        if let Some(v) = lookup(&self.core, &culture, key) {
            return v;
        }
        let plugins = self.plugins.read().unwrap_or_else(PoisonError::into_inner);
        plugins
            .values()
            .find_map(|m| lookup(m, &culture, key))
            .unwrap_or_else(|| key.to_owned())
    }

    fn on_culture_changed(&self, handler: CultureChangedHandler) {
        self.handlers().push(handler);
    }
}
