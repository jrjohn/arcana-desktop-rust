//! Plugin parameter schema → settings form.
//!
//! The schema is the manifest's `contributes.configuration.properties` (C# has the records but
//! never renders them). [`settings_form`] turns it into a flat, UI-agnostic field list; the UI
//! draws one widget per [`SettingField`] and validates edits with [`SettingField::validate`].

use arcana_core::{AppError, AppResult};
use serde_json::Value;

use crate::manifest::{ManifestConfigurationProperty, PluginManifest};

/// Widget kind derived from the JSON-schema-like `type` / `enum`.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingKind {
    Bool,
    Integer {
        min: Option<f64>,
        max: Option<f64>,
    },
    Number {
        min: Option<f64>,
        max: Option<f64>,
    },
    Text,
    /// `(value, label)` pairs.
    Choice(Vec<(Value, String)>),
}

/// One generated form field.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingField {
    pub key: String,
    pub kind: SettingKind,
    pub default: Value,
    pub description: Option<String>,
    pub description_key: Option<String>,
}

impl SettingField {
    fn from_property(key: &str, p: &ManifestConfigurationProperty) -> Self {
        let kind = if let Some(values) = &p.enum_values {
            let labels = p.enum_descriptions.clone().unwrap_or_default();
            SettingKind::Choice(
                values
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        let label = labels.get(i).cloned().unwrap_or_else(|| match v {
                            Value::String(s) => s.clone(),
                            other => other.to_string(),
                        });
                        (v.clone(), label)
                    })
                    .collect(),
            )
        } else {
            match p.prop_type.as_str() {
                "boolean" => SettingKind::Bool,
                "integer" => SettingKind::Integer {
                    min: p.minimum,
                    max: p.maximum,
                },
                "number" => SettingKind::Number {
                    min: p.minimum,
                    max: p.maximum,
                },
                _ => SettingKind::Text,
            }
        };
        let default = p.default.clone().unwrap_or(match &kind {
            SettingKind::Bool => Value::Bool(false),
            SettingKind::Integer { .. } | SettingKind::Number { .. } => Value::from(0),
            SettingKind::Text => Value::String(String::new()),
            SettingKind::Choice(c) => c.first().map_or(Value::Null, |(v, _)| v.clone()),
        });
        Self {
            key: key.to_string(),
            kind,
            default,
            description: p.description.clone(),
            description_key: p.description_key.clone(),
        }
    }

    /// Check a value against the field's type and bounds.
    ///
    /// # Errors
    /// `AppError::Validation` naming the field.
    pub fn validate(&self, value: &Value) -> AppResult<()> {
        let bad = |why: &str| {
            Err(AppError::validation(
                format!("{}: {why}", self.key),
                vec![self.key.clone()],
            ))
        };
        let in_range = |n: f64, min: Option<f64>, max: Option<f64>| {
            min.is_none_or(|m| n >= m) && max.is_none_or(|m| n <= m)
        };
        match (&self.kind, value) {
            (SettingKind::Bool, Value::Bool(_)) | (SettingKind::Text, Value::String(_)) => Ok(()),
            (SettingKind::Integer { min, max }, Value::Number(n)) if n.is_i64() || n.is_u64() => {
                if in_range(n.as_f64().unwrap_or_default(), *min, *max) {
                    Ok(())
                } else {
                    bad("out of range")
                }
            }
            (SettingKind::Number { min, max }, Value::Number(n)) => {
                if in_range(n.as_f64().unwrap_or_default(), *min, *max) {
                    Ok(())
                } else {
                    bad("out of range")
                }
            }
            (SettingKind::Choice(c), v) if c.iter().any(|(x, _)| x == v) => Ok(()),
            _ => bad("wrong type"),
        }
    }
}

/// Fields in key order; empty when the manifest declares no configuration.
#[must_use]
pub fn settings_form(manifest: &PluginManifest) -> Vec<SettingField> {
    manifest
        .contributes
        .as_ref()
        .and_then(|c| c.configuration.as_ref())
        .and_then(|c| c.properties.as_ref())
        .map(|props| {
            props
                .iter()
                .map(|(k, p)| SettingField::from_property(k, p))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> PluginManifest {
        PluginManifest::from_json(
            r#"{"id":"x","name":"X","version":"1.0.0","contributes":{"configuration":{"properties":{
              "grid.size":{"type":"integer","default":20,"minimum":5,"maximum":100},
              "grid.show":{"type":"boolean","default":true},
              "theme":{"type":"string","enum":["light","dark"],"enumDescriptions":["Light","Dark"]},
              "name":{"type":"string"}}}}}"#,
        )
        .unwrap_or_default()
    }

    #[test]
    fn generates_one_field_per_property_in_key_order() {
        let f = settings_form(&manifest());
        let keys: Vec<_> = f.iter().map(|f| f.key.as_str()).collect();
        assert_eq!(keys, ["grid.show", "grid.size", "name", "theme"]);
        assert_eq!(f[0].kind, SettingKind::Bool);
        assert_eq!(f[2].default, Value::String(String::new()));
        assert!(matches!(&f[3].kind, SettingKind::Choice(c) if c[1].1 == "Dark"));
        assert_eq!(f[3].default, Value::from("light"));
    }

    #[test]
    fn validates_type_and_range() {
        let f = settings_form(&manifest());
        let size = &f[1];
        assert!(size.validate(&Value::from(20)).is_ok());
        assert!(size.validate(&Value::from(1)).is_err());
        assert!(size.validate(&Value::from(2.5)).is_err());
        assert!(f[0].validate(&Value::from("x")).is_err());
        assert!(f[3].validate(&Value::from("dark")).is_ok());
        assert!(f[3].validate(&Value::from("blue")).is_err());
    }

    #[test]
    fn no_configuration_means_no_fields() {
        let m =
            PluginManifest::from_json(r#"{"id":"x","name":"X","version":"1"}"#).unwrap_or_default();
        assert_eq!(settings_form(&m).len(), 0);
    }
}
