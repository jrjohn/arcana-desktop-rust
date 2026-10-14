//! Paging (C# `PagedResult<T>` and `PageRequest`).

use std::fmt;

/// One page of a larger result set (C# `PagedResult<T>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagedResult<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub total_count: u64,
}

impl<T> PagedResult<T> {
    #[must_use]
    pub fn new(items: Vec<T>, page: u32, page_size: u32, total_count: u64) -> Self {
        Self {
            items,
            page,
            page_size,
            total_count,
        }
    }

    /// An empty page (C# `PagedResult<T>.Empty(page = 1, pageSize = 10)`).
    #[must_use]
    pub fn empty(page: u32, page_size: u32) -> Self {
        Self::new(Vec::new(), page, page_size, 0)
    }

    /// `ceil(total_count / page_size)`. C# divides by zero here (yielding garbage); we return 0.
    #[must_use]
    pub fn total_pages(&self) -> u64 {
        if self.page_size == 0 {
            0
        } else {
            self.total_count.div_ceil(u64::from(self.page_size))
        }
    }

    #[must_use]
    pub fn has_previous_page(&self) -> bool {
        self.page > 1
    }

    #[must_use]
    pub fn has_next_page(&self) -> bool {
        u64::from(self.page) < self.total_pages()
    }

    /// Transform the items, keeping the paging numbers (C# `Map`).
    #[must_use]
    pub fn map<U>(self, f: impl FnMut(T) -> U) -> PagedResult<U> {
        PagedResult::new(
            self.items.into_iter().map(f).collect(),
            self.page,
            self.page_size,
            self.total_count,
        )
    }
}

impl<T> fmt::Display for PagedResult<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "page {}/{} ({} items)",
            self.page,
            self.total_pages(),
            self.total_count
        )
    }
}

/// Paging + sorting request (C# `record PageRequest(Page = 1, PageSize = 10, SortBy, Descending)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageRequest {
    pub page: u32,
    pub page_size: u32,
    pub sort_by: Option<String>,
    pub descending: bool,
}

impl Default for PageRequest {
    fn default() -> Self {
        Self::new(1, 10)
    }
}

impl PageRequest {
    #[must_use]
    pub fn new(page: u32, page_size: u32) -> Self {
        Self {
            page,
            page_size,
            sort_by: None,
            descending: false,
        }
    }

    /// Rows to skip: `(page - 1) * page_size`, saturating at 0 for page 0.
    #[must_use]
    pub fn skip(&self) -> u64 {
        u64::from(self.page.saturating_sub(1)) * u64::from(self.page_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn total_pages_rounds_up_and_handles_zero_page_size() {
        assert_eq!(PagedResult::<u8>::new(vec![], 1, 10, 21).total_pages(), 3);
        assert_eq!(PagedResult::<u8>::new(vec![], 1, 0, 21).total_pages(), 0);
    }

    #[test]
    fn previous_and_next_flags() {
        let first = PagedResult::<u8>::new(vec![], 1, 10, 21);
        assert!(!first.has_previous_page());
        assert!(first.has_next_page());
        let last = PagedResult::<u8>::new(vec![], 3, 10, 21);
        assert!(last.has_previous_page());
        assert!(!last.has_next_page());
    }

    #[test]
    fn empty_and_map_and_display() {
        let e = PagedResult::<u8>::empty(1, 10);
        assert!(e.items.is_empty());
        assert_eq!(e.total_count, 0);
        let m = PagedResult::new(vec![1, 2], 2, 2, 5).map(|x| x * 10);
        assert_eq!(m, PagedResult::new(vec![10, 20], 2, 2, 5));
        assert_eq!(m.to_string(), "page 2/3 (5 items)");
    }

    #[test]
    fn page_request_defaults_and_skip() {
        let d = PageRequest::default();
        assert_eq!(
            (d.page, d.page_size, d.sort_by, d.descending),
            (1, 10, None, false)
        );
        assert_eq!(PageRequest::new(3, 20).skip(), 40);
        assert_eq!(PageRequest::new(0, 20).skip(), 0);
    }
}
