//! Repository and unit-of-work abstractions (C# `Repository.cs`, `UnitOfWork.cs`).
//!
//! Differences from C#, all forced by the absence of LINQ expression trees / an ORM change
//! tracker:
//! - Predicates are plain closures evaluated in memory after loading the (query-filtered) rows.
//!   Storage-specific repositories expose dedicated finders for anything that must run in SQL.
//! - `Query()` (`IQueryable`) has no equivalent and is omitted.
//! - There is no `SaveChangesAsync`: like the C# implementation (whose `AddAsync`/`UpdateAsync`
//!   call `SaveChanges` internally) every write is persisted when the call returns.
//! - `GetRepository<T>()` is generic over arbitrary types; that is not expressible on a trait
//!   object, so concrete units of work expose typed repository accessors instead.
//! - `CancellationToken` parameters are dropped: dropping a Rust future cancels it.

use async_trait::async_trait;
use uuid::Uuid;

use crate::{AppResult, PageRequest, PagedResult};

/// A borrowed, thread-safe predicate (C# `Expression<Func<TEntity, bool>>`).
pub type Predicate<'a, T> = &'a (dyn Fn(&T) -> bool + Send + Sync);

/// Generic repository (C# `Repository<TEntity, TKey>`; `Repository<TEntity>` is `K = i32`).
#[async_trait]
pub trait Repository<T, K = i32>: Send + Sync
where
    T: Send + Sync + 'static,
    K: Send + Sync + 'static,
{
    async fn get_by_id(&self, id: K) -> AppResult<Option<T>>;
    async fn get_all(&self) -> AppResult<Vec<T>>;
    async fn get_paged(&self, request: &PageRequest) -> AppResult<PagedResult<T>>;
    async fn find(&self, predicate: Predicate<'_, T>) -> AppResult<Vec<T>>;
    async fn first_or_default(&self, predicate: Predicate<'_, T>) -> AppResult<Option<T>>;
    async fn any(&self, predicate: Predicate<'_, T>) -> AppResult<bool>;
    async fn count(&self, predicate: Option<Predicate<'_, T>>) -> AppResult<u64>;
    /// Insert and return the entity with its generated key and audit fields filled in.
    async fn add(&self, entity: T) -> AppResult<T>;
    async fn add_range(&self, entities: Vec<T>) -> AppResult<()>;
    async fn update(&self, entity: &mut T) -> AppResult<()>;
    /// Soft-deletes when the entity supports it, otherwise removes the row.
    async fn delete(&self, entity: &mut T) -> AppResult<()>;
    async fn delete_by_id(&self, id: K) -> AppResult<()>;
}

/// An explicit transaction (C# `TransactionScope`). Dropping an active scope rolls it back.
#[async_trait]
pub trait TransactionScope: Send {
    fn transaction_id(&self) -> Uuid;
    fn is_active(&self) -> bool;
    async fn commit(&mut self) -> AppResult<()>;
    async fn rollback(&mut self) -> AppResult<()>;
}

/// C# `UnitOfWork`. Dropping an uncommitted unit with an open transaction rolls it back.
#[async_trait]
pub trait UnitOfWork: Send {
    fn has_changes(&self) -> bool;
    fn is_committed(&self) -> bool;
    fn is_rolled_back(&self) -> bool;
    /// Commit; returns the number of rows written inside the transaction.
    async fn commit(&mut self) -> AppResult<u64>;
    async fn rollback(&mut self) -> AppResult<()>;
    async fn begin_transaction(&mut self) -> AppResult<Box<dyn TransactionScope>>;
}

/// C# `UnitOfWorkFactory`.
#[async_trait]
pub trait UnitOfWorkFactory: Send + Sync {
    type Unit: UnitOfWork;
    fn create(&self) -> Self::Unit;
    async fn create_with_transaction(&self) -> AppResult<Self::Unit>;
}

/// Outcome summary of a unit of work (C# `record UnitOfWorkResult`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnitOfWorkResult {
    pub success: bool,
    pub affected_rows: u64,
    pub error: Option<String>,
}

impl UnitOfWorkResult {
    #[must_use]
    pub fn succeeded(affected_rows: u64) -> Self {
        Self {
            success: true,
            affected_rows,
            error: None,
        }
    }

    #[must_use]
    pub fn failed(error: impl Into<String>) -> Self {
        Self {
            success: false,
            affected_rows: 0,
            error: Some(error.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_of_work_result_constructors() {
        assert_eq!(
            UnitOfWorkResult::succeeded(3),
            UnitOfWorkResult {
                success: true,
                affected_rows: 3,
                error: None
            }
        );
        let f = UnitOfWorkResult::failed("boom");
        assert!(!f.success);
        assert_eq!(f.error.as_deref(), Some("boom"));
    }
}
