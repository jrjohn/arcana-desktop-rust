//! Vector clocks (C# `VectorClock`, `CausalRelation`).

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::hash::{Hash, Hasher};

use arcana_core::{AppError, AppResult, ErrorCode};
use serde::{Deserialize, Serialize};

/// Causal relationship between two vector clocks (C# `CausalRelation`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CausalRelation {
    /// This clock happened after the other.
    HappenedAfter,
    /// This clock happened before the other.
    HappenedBefore,
    /// No causal relationship.
    Concurrent,
    /// Identical clocks.
    Equal,
}

/// Immutable vector clock: every operation returns a new clock (C# `VectorClock`).
///
/// Entries are kept in a `BTreeMap`, so `Display`/serialisation are ordered by node id
/// (C# `Dictionary` order is insertion order; nothing depended on it).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VectorClock {
    clock: BTreeMap<String, i64>,
}

impl VectorClock {
    /// An empty clock.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A clock with a copy of `entries`.
    #[must_use]
    pub fn from_entries<I, S>(entries: I) -> Self
    where
        I: IntoIterator<Item = (S, i64)>,
        S: Into<String>,
    {
        Self {
            clock: entries.into_iter().map(|(k, v)| (k.into(), v)).collect(),
        }
    }

    /// The raw entries (C# `Clock`).
    #[must_use]
    pub fn entries(&self) -> &BTreeMap<String, i64> {
        &self.clock
    }

    /// A copy with `node_id` incremented by one.
    #[must_use]
    pub fn increment(&self, node_id: &str) -> Self {
        let mut clock = self.clock.clone();
        clock.insert(node_id.to_owned(), self.get_value(node_id) + 1);
        Self { clock }
    }

    /// The value for `node_id`, 0 when absent.
    #[must_use]
    pub fn get_value(&self, node_id: &str) -> i64 {
        self.clock.get(node_id).copied().unwrap_or(0)
    }

    /// Component-wise maximum.
    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        let mut clock = self.clock.clone();
        for (k, v) in &other.clock {
            let e = clock.entry(k.clone()).or_insert(*v);
            *e = (*e).max(*v);
        }
        Self { clock }
    }

    /// The causal relation of `self` to `other` (C# `CompareTo(VectorClock)`).
    #[must_use]
    pub fn compare_to(&self, other: &Self) -> CausalRelation {
        let mut this_greater = false;
        let mut other_greater = false;
        for key in self.clock.keys().chain(other.clock.keys()) {
            let a = self.get_value(key);
            let b = other.get_value(key);
            this_greater |= a > b;
            other_greater |= b > a;
        }
        match (this_greater, other_greater) {
            (true, true) => CausalRelation::Concurrent,
            (true, false) => CausalRelation::HappenedAfter,
            (false, true) => CausalRelation::HappenedBefore,
            (false, false) => CausalRelation::Equal,
        }
    }

    /// JSON object `{"node":n,...}` (C# `Serialize`).
    #[must_use]
    pub fn serialize(&self) -> String {
        // A string→i64 map always serialises.
        serde_json::to_string(&self.clock).unwrap_or_else(|_| "{}".to_owned())
    }

    /// Parse [`VectorClock::serialize`] output (C# `Deserialize`; `null` gives an empty clock).
    ///
    /// # Errors
    /// `DataCorruption` when `json` is not a string→integer object. (C# throws `JsonException`.)
    pub fn deserialize(json: &str) -> AppResult<Self> {
        let clock: Option<BTreeMap<String, i64>> = serde_json::from_str(json)
            .map_err(|e| AppError::data(ErrorCode::DataCorruption, e.to_string()))?;
        Ok(Self {
            clock: clock.unwrap_or_default(),
        })
    }
}

/// Equality is causal equality, as in C# (`{a:0}` equals `{}`).
impl PartialEq for VectorClock {
    fn eq(&self, other: &Self) -> bool {
        self.compare_to(other) == CausalRelation::Equal
    }
}

impl Eq for VectorClock {}

/// Hashes the non-zero entries only, so it agrees with causal equality. (C# hashes every
/// entry, which breaks the hash/equality contract for zero entries.)
impl Hash for VectorClock {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for (k, v) in self.clock.iter().filter(|(_, v)| **v != 0) {
            k.hash(state);
            v.hash(state);
        }
    }
}

/// The C# `IComparable<VectorClock>` view. C# maps `Concurrent` to `0`; a partial order is
/// the honest Rust equivalent, so concurrent clocks compare as `None`.
impl PartialOrd for VectorClock {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.compare_to(other) {
            CausalRelation::HappenedAfter => Some(Ordering::Greater),
            CausalRelation::HappenedBefore => Some(Ordering::Less),
            CausalRelation::Equal => Some(Ordering::Equal),
            CausalRelation::Concurrent => None,
        }
    }
}

impl fmt::Display for VectorClock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<String> = self.clock.iter().map(|(k, v)| format!("{k}:{v}")).collect();
        write!(f, "VectorClock({})", parts.join(", "))
    }
}
