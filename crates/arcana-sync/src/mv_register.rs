//! Multi-value register (C# `MVRegister<T>`).

use arcana_core::{AppError, AppResult, ErrorCode};

use crate::{CausalRelation, VectorClock};

/// Keeps every concurrent value until someone resolves them (C# `MVRegister<T>`).
#[derive(Debug, Clone, PartialEq)]
pub struct MvRegister<T> {
    values: Vec<(T, VectorClock)>,
}

impl<T> Default for MvRegister<T> {
    fn default() -> Self {
        Self { values: Vec::new() }
    }
}

impl<T: Clone> MvRegister<T> {
    /// An empty register.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A register holding one value.
    #[must_use]
    pub fn with_value(value: T, clock: VectorClock) -> Self {
        Self {
            values: vec![(value, clock)],
        }
    }

    #[must_use]
    pub fn values(&self) -> &[(T, VectorClock)] {
        &self.values
    }

    /// More than one concurrent value.
    #[must_use]
    pub fn has_conflict(&self) -> bool {
        self.values.len() > 1
    }

    /// The only value (C# `SingleValue`).
    ///
    /// # Errors
    /// `ConflictError` when the register is empty or holds concurrent values. (C# throws
    /// `InvalidOperationException`; it also throws for the empty case.)
    pub fn single_value(&self) -> AppResult<&T> {
        match self.values.as_slice() {
            [(v, _)] => Ok(v),
            _ => Err(AppError::data(
                ErrorCode::ConflictError,
                "Multiple concurrent values exist",
            )),
        }
    }

    /// Drop values the new clock dominates; add it unless an existing value dominates it.
    pub fn set(&mut self, value: T, clock: VectorClock) {
        self.values
            .retain(|(_, c)| c.compare_to(&clock) != CausalRelation::HappenedBefore);
        if !self
            .values
            .iter()
            .any(|(_, c)| c.compare_to(&clock) == CausalRelation::HappenedAfter)
        {
            self.values.push((value, clock));
        }
    }

    /// Keep every value not dominated by another, without duplicate clocks.
    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        let all: Vec<&(T, VectorClock)> = self.values.iter().chain(&other.values).collect();
        let mut result = Self::new();
        for (value, clock) in all.iter().map(|p| (&p.0, &p.1)) {
            let dominated = all
                .iter()
                .any(|(_, c)| c.compare_to(clock) == CausalRelation::HappenedAfter);
            if !dominated && !result.values.iter().any(|(_, c)| c == clock) {
                result.values.push((value.clone(), clock.clone()));
            }
        }
        result
    }

    /// Replace everything with the chosen value.
    pub fn resolve(&mut self, resolved_value: T, merged_clock: VectorClock) {
        self.values.clear();
        self.values.push((resolved_value, merged_clock));
    }
}
