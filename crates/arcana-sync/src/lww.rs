//! Last-writer-wins register and map (C# `LWWRegister<T>`, `LWWMap`).

use std::collections::BTreeMap;

use arcana_core::Timestamp;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Whether a write at (`ts`, `node`) beats the current (`cur_ts`, `cur_node`): later timestamp
/// wins, ties go to the ordinally greater node id.
fn wins(ts: Timestamp, node: &str, cur_ts: Timestamp, cur_node: &str) -> bool {
    ts > cur_ts || (ts == cur_ts && node > cur_node)
}

/// Last-writer-wins register (C# `LWWRegister<T>`). `T` is usually an `Option<_>` when the
/// value may be absent (C# `T?`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LwwRegister<T> {
    value: T,
    timestamp: Timestamp,
    node_id: String,
}

impl<T: Clone> LwwRegister<T> {
    #[must_use]
    pub fn new(value: T, timestamp: Timestamp, node_id: impl Into<String>) -> Self {
        Self {
            value,
            timestamp,
            node_id: node_id.into(),
        }
    }

    #[must_use]
    pub fn value(&self) -> &T {
        &self.value
    }

    #[must_use]
    pub fn timestamp(&self) -> Timestamp {
        self.timestamp
    }

    #[must_use]
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Take the write when it is more recent; returns whether it was taken.
    pub fn update(&mut self, new_value: T, timestamp: Timestamp, node_id: &str) -> bool {
        if wins(timestamp, node_id, self.timestamp, &self.node_id) {
            self.value = new_value;
            self.timestamp = timestamp;
            node_id.clone_into(&mut self.node_id);
            true
        } else {
            false
        }
    }

    /// A new register holding the more recent of the two.
    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        if wins(
            other.timestamp,
            &other.node_id,
            self.timestamp,
            &self.node_id,
        ) {
            other.clone()
        } else {
            self.clone()
        }
    }
}

/// Field-level last-writer-wins map (C# `LWWMap`). C# stores `object?`; values here are JSON
/// values so any serde type can be stored and read back with a type check.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LwwMap {
    fields: BTreeMap<String, LwwRegister<Value>>,
}

impl LwwMap {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Every field register (C# `Fields`).
    #[must_use]
    pub fn fields(&self) -> &BTreeMap<String, LwwRegister<Value>> {
        &self.fields
    }

    /// Write a field; an older write than the stored one is ignored. A value that cannot be
    /// represented as JSON is stored as `null`.
    pub fn set<V: Serialize>(
        &mut self,
        field_name: &str,
        value: &V,
        timestamp: Timestamp,
        node_id: &str,
    ) {
        let value = serde_json::to_value(value).unwrap_or(Value::Null);
        if let Some(existing) = self.fields.get_mut(field_name) {
            existing.update(value, timestamp, node_id);
        } else {
            self.fields.insert(
                field_name.to_owned(),
                LwwRegister::new(value, timestamp, node_id),
            );
        }
    }

    /// The field as `V`; `None` when missing or of another type (C# returns `default`).
    #[must_use]
    pub fn get<V: DeserializeOwned>(&self, field_name: &str) -> Option<V> {
        self.fields
            .get(field_name)
            .and_then(|r| serde_json::from_value(r.value().clone()).ok())
    }

    /// Union of fields, each resolved by [`LwwRegister::merge`].
    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        let mut fields = self.fields.clone();
        for (k, theirs) in &other.fields {
            let merged = match fields.get(k) {
                Some(ours) => ours.merge(theirs),
                None => theirs.clone(),
            };
            fields.insert(k.clone(), merged);
        }
        Self { fields }
    }
}
