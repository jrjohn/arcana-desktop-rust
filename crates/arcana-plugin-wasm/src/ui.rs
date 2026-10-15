//! Declarative UI tree a WASM plugin returns from `render`; the host draws it.
//!
//! Plugins never touch egui: they describe forms, tables and lists as data, the host
//! validates the tree (depth, size, column count) before any widget is built.

use arcana_core::{AppError, AppResult, ErrorCode};
use serde::{Deserialize, Serialize};

/// Deepest nesting the host will render.
pub const MAX_DEPTH: usize = 16;
/// Most nodes (including table cells) the host will render for one plugin view.
pub const MAX_NODES: usize = 10_000;

/// One node of a plugin-described view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum UiNode {
    /// A titled group of child nodes.
    Form {
        #[serde(default)]
        title: String,
        #[serde(default)]
        children: Vec<UiNode>,
    },
    /// Read-only text.
    Text { text: String },
    /// A text field bound to a plugin state key.
    Input {
        label: String,
        key: String,
        #[serde(default)]
        value: String,
    },
    /// A button that sends `command` back to the plugin's `run-command`.
    Button { label: String, command: String },
    /// A grid; every row must have exactly `columns.len()` cells.
    Table {
        columns: Vec<String>,
        #[serde(default)]
        rows: Vec<Vec<String>>,
    },
    /// A bulleted list.
    List {
        #[serde(default)]
        items: Vec<String>,
    },
}

impl UiNode {
    /// Parse and validate a plugin's `render` output.
    ///
    /// # Errors
    /// `PluginLoadFailed` when the JSON is malformed or the tree breaks a structural rule.
    pub fn parse(json: &str) -> AppResult<Self> {
        let node: Self =
            serde_json::from_str(json).map_err(|e| plugin_err(format!("invalid UI tree: {e}")))?;
        node.validate()?;
        Ok(node)
    }

    /// Check depth, size and table shape.
    ///
    /// # Errors
    /// `PluginLoadFailed` naming the first violation.
    pub fn validate(&self) -> AppResult<()> {
        let mut count = 0;
        self.walk(1, &mut count)
    }

    fn walk(&self, depth: usize, count: &mut usize) -> AppResult<()> {
        if depth > MAX_DEPTH {
            return Err(plugin_err(format!("UI tree deeper than {MAX_DEPTH}")));
        }
        *count += 1;
        match self {
            Self::Form { children, .. } => {
                for c in children {
                    c.walk(depth + 1, count)?;
                }
            }
            Self::Table { columns, rows } => {
                if columns.is_empty() {
                    return Err(plugin_err("table without columns".into()));
                }
                for (i, r) in rows.iter().enumerate() {
                    if r.len() != columns.len() {
                        return Err(plugin_err(format!(
                            "table row {i} has {} cells, expected {}",
                            r.len(),
                            columns.len()
                        )));
                    }
                    *count += r.len();
                }
            }
            Self::List { items } => *count += items.len(),
            Self::Button { command, .. } if command.is_empty() => {
                return Err(plugin_err("button without command".into()));
            }
            Self::Input { key, .. } if key.is_empty() => {
                return Err(plugin_err("input without key".into()));
            }
            _ => {}
        }
        if *count > MAX_NODES {
            return Err(plugin_err(format!(
                "UI tree has more than {MAX_NODES} nodes"
            )));
        }
        Ok(())
    }
}

pub(crate) fn plugin_err(message: String) -> AppError {
    AppError::Plugin {
        code: ErrorCode::PluginLoadFailed,
        message,
        plugin_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_form_with_all_node_kinds() {
        let n = UiNode::parse(
            r#"{"type":"form","title":"T","children":[{"type":"text","text":"x"},
            {"type":"input","label":"L","key":"k"},{"type":"button","label":"B","command":"c"},
            {"type":"table","columns":["a"],"rows":[["1"]]},{"type":"list","items":["i"]}]}"#,
        )
        .unwrap();
        let UiNode::Form { children, .. } = n else {
            panic!("not a form")
        };
        assert_eq!(children.len(), 5);
    }

    #[test]
    fn rejects_ragged_table() {
        let e = UiNode::parse(r#"{"type":"table","columns":["a","b"],"rows":[["1"]]}"#);
        assert!(e.unwrap_err().message().contains("row 0"));
    }

    #[test]
    fn rejects_unknown_node_type() {
        assert!(UiNode::parse(r#"{"type":"script","src":"x"}"#).is_err());
    }

    #[test]
    fn rejects_too_deep_tree() {
        let mut json = r#"{"type":"text","text":"x"}"#.to_string();
        for _ in 0..MAX_DEPTH {
            json = format!(r#"{{"type":"form","children":[{json}]}}"#);
        }
        assert!(
            UiNode::parse(&json)
                .unwrap_err()
                .message()
                .contains("deeper")
        );
    }

    #[test]
    fn rejects_button_without_command() {
        assert!(UiNode::parse(r#"{"type":"button","label":"b","command":""}"#).is_err());
    }
}
