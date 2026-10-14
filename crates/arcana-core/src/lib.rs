//! Core building blocks shared by every layer (C# `Arcana.Core`).
//!
//! Nothing here depends on storage, UI or plugins.

use std::fmt;

/// Application error. Mirrors C# `AppError`: a stable code plus a human message.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AppError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    /// Stable machine-readable code, used by the UI and logs.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "NOT_FOUND",
            Self::Validation(_) => "VALIDATION",
            Self::Conflict(_) => "CONFLICT",
            Self::Unauthorized(_) => "UNAUTHORIZED",
            Self::Internal(_) => "INTERNAL",
        }
    }
}

/// Result alias used across the workspace (C# `Result<T>`).
pub type AppResult<T> = Result<T, AppError>;

/// One page of a larger result set (C# `PagedResult<T>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagedResult<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub total: u64,
}

impl<T> PagedResult<T> {
    /// Number of pages needed for `total` items; 0 when `page_size` is 0.
    #[must_use]
    pub fn total_pages(&self) -> u64 {
        if self.page_size == 0 {
            0
        } else {
            self.total.div_ceil(u64::from(self.page_size))
        }
    }
}

impl<T> fmt::Display for PagedResult<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "page {}/{} ({} items)",
            self.page,
            self.total_pages(),
            self.total
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_stable() {
        assert_eq!(AppError::NotFound("x".into()).code(), "NOT_FOUND");
        assert_eq!(
            AppError::Validation("x".into()).to_string(),
            "validation failed: x"
        );
    }

    #[test]
    fn total_pages_rounds_up_and_handles_zero_page_size() {
        let p = PagedResult::<u8> {
            items: vec![],
            page: 1,
            page_size: 10,
            total: 21,
        };
        assert_eq!(p.total_pages(), 3);
        let z = PagedResult::<u8> {
            items: vec![],
            page: 1,
            page_size: 0,
            total: 21,
        };
        assert_eq!(z.total_pages(), 0);
    }
}
