//! `LocalizationService` contract (moved from `arcana-infra`, M3 interim location).

use std::collections::HashMap;
use std::fmt::Display;

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
