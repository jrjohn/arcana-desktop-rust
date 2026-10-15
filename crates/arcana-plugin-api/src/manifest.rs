//! `plugin.json` (C# `PluginManifest` and its contribution records). Field names are the C#
//! `JsonPropertyName`s, so the C# manifests deserialize unchanged.
//!
//! Rust additions (absent in C#, ignored by it): `permissions` — what a third-party (WASM)
//! plugin asks for — and `abiVersion` for WASM components.

use std::collections::BTreeMap;

use arcana_core::{AppError, AppResult, ErrorCode};
use serde::{Deserialize, Serialize};

use crate::permission::PluginPermission;
use crate::plugin::PluginType;

fn default_type() -> String {
    "Module".to_string()
}
fn default_view_type() -> String {
    "Page".to_string()
}

/// C# `PluginManifest`. `Default` keeps the C# initializer `Type = "Module"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_class: Option<String>,
    #[serde(rename = "type", default = "default_type")]
    pub plugin_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activation_events: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contributes: Option<ManifestContributions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub l10n: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Rust addition: requested permissions, by C# flag name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<String>,
    /// Rust addition: WIT world ABI version a WASM plugin was built against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abi_version: Option<u32>,
}

impl Default for PluginManifest {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            version: String::new(),
            description: None,
            author: None,
            main: None,
            plugin_class: None,
            plugin_type: default_type(),
            dependencies: None,
            activation_events: None,
            contributes: None,
            l10n: None,
            icon: None,
            permissions: Vec::new(),
            abi_version: None,
        }
    }
}

impl Default for ManifestViewDefinition {
    fn default() -> Self {
        Self {
            id: String::new(),
            title_key: String::new(),
            title: None,
            icon: None,
            view_type: default_view_type(),
            view_class: None,
            view_model_class: None,
            category: None,
            category_key: None,
            order: 0,
            can_have_multiple_instances: false,
            module_id: None,
            is_module_default_tab: false,
            module_tab_order: 0,
        }
    }
}

impl PluginManifest {
    /// # Errors
    /// Malformed JSON or missing `id` / `name` / `version`.
    pub fn from_json(json: &str) -> AppResult<Self> {
        let m: Self = serde_json::from_str(json).map_err(|e| {
            AppError::validation(format!("Invalid plugin manifest: {e}"), vec![e.to_string()])
        })?;
        Ok(m)
    }

    /// # Errors
    /// Serialization failure (not expected for this type).
    pub fn to_json(&self) -> AppResult<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| AppError::data(ErrorCode::DataCorruption, e.to_string()))
    }

    /// `Enum.TryParse(Type)`, falling back to `Module` like C# `ManifestService`.
    #[must_use]
    pub fn parsed_type(&self) -> PluginType {
        PluginType::parse(&self.plugin_type).unwrap_or_default()
    }

    /// Union of the declared `permissions`; unknown names are reported.
    ///
    /// # Errors
    /// The first unknown permission name.
    pub fn requested_permissions(&self) -> AppResult<PluginPermission> {
        let mut out = PluginPermission::NONE;
        for p in &self.permissions {
            out |= PluginPermission::parse(p).ok_or_else(|| {
                AppError::validation(format!("Unknown permission '{p}'"), vec![p.clone()])
            })?;
        }
        Ok(out)
    }

    #[must_use]
    pub fn activation_events(&self) -> &[String] {
        self.activation_events.as_deref().unwrap_or_default()
    }
}

/// C# `ManifestContributions`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManifestContributions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub views: Option<Vec<ManifestViewDefinition>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub menus: Option<Vec<ManifestMenuDefinition>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commands: Option<Vec<ManifestCommandDefinition>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toolbars: Option<Vec<ManifestToolbarDefinition>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keybindings: Option<Vec<ManifestKeybindingDefinition>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration: Option<ManifestConfigurationDefinition>,
}

/// C# `ManifestViewDefinition`. `Default` keeps the C# initializer `Type = "Page"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestViewDefinition {
    pub id: String,
    #[serde(default)]
    pub title_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(rename = "type", default = "default_view_type")]
    pub view_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_model_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category_key: Option<String>,
    #[serde(default)]
    pub order: i32,
    #[serde(default)]
    pub can_have_multiple_instances: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module_id: Option<String>,
    #[serde(default)]
    pub is_module_default_tab: bool,
    #[serde(default)]
    pub module_tab_order: i32,
}

/// C# `ManifestMenuDefinition`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManifestMenuDefinition {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub location: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default)]
    pub order: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
    #[serde(default)]
    pub is_separator: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<ManifestMenuDefinition>>,
}

/// C# `ManifestCommandDefinition`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManifestCommandDefinition {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enablement: Option<String>,
}

/// C# `ManifestToolbarDefinition`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManifestToolbarDefinition {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<ManifestToolbarItemDefinition>>,
}

/// C# `ManifestToolbarItemDefinition`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManifestToolbarItemDefinition {
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// C# `ManifestKeybindingDefinition`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManifestKeybindingDefinition {
    pub command: String,
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub win: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linux: Option<String>,
}

/// C# `ManifestConfigurationDefinition` — also the plugin's settings schema (see
/// [`crate::schema`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManifestConfigurationDefinition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub properties: Option<BTreeMap<String, ManifestConfigurationProperty>>,
}

/// C# `ManifestConfigurationProperty`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManifestConfigurationProperty {
    #[serde(rename = "type")]
    pub prop_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_key: Option<String>,
    #[serde(rename = "enum", default, skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enum_descriptions: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum: Option<f64>,
}
