//! Unit of work over one [`Db`] (C# `UnitOfWorkImpl`, `TransactionScopeImpl`,
//! `UnitOfWorkFactoryImpl`).
//!
//! Semantics kept from C#: committing twice or after a rollback is an error; a failed commit
//! rolls back; rollback is idempotent; only one transaction at a time; dropping an active
//! transaction (or an uncommitted unit that began one) rolls it back.
//!
//! Difference: C#'s factory opens a new `DbContext` per unit. Here units created by one factory
//! share the factory's connection (an in-memory `SQLite` database exists only on its own
//! connection), so they serialise; opening a second transaction while one is open fails just
//! like C#'s `BeginTransactionAsync` does.

use arcana_core::{
    AppError, AppResult, ErrorCode, TransactionScope, UnitOfWork, UnitOfWorkFactory,
};
use async_trait::async_trait;
use uuid::Uuid;

use crate::db::{Db, exec_raw};
use crate::repository::{
    CustomerRepository, OrderRepository, ProductRepository, SqliteRepository, customer_repository,
    order_repository, product_repository,
};
use crate::row::TableEntity;

fn invalid(message: &str) -> AppError {
    AppError::data(ErrorCode::DatabaseError, message)
}

/// C# `UnitOfWorkImpl`.
pub struct SqliteUnitOfWork {
    db: Db,
    committed: bool,
    rolled_back: bool,
    began: bool,
}

impl SqliteUnitOfWork {
    #[must_use]
    pub fn new(db: Db) -> Self {
        Self {
            db,
            committed: false,
            rolled_back: false,
            began: false,
        }
    }

    /// Generic repository for any mapped table (C# `GetRepository<T>()` fallback).
    #[must_use]
    pub fn repository<T: TableEntity>(&self) -> SqliteRepository<T> {
        SqliteRepository::new(self.db.clone())
    }

    /// The specialised customer repository (C# DI-resolved `GetRepository<Customer>()`).
    #[must_use]
    pub fn customers(&self) -> impl CustomerRepository + Clone {
        customer_repository(self.db.clone())
    }

    /// The specialised product repository.
    #[must_use]
    pub fn products(&self) -> impl ProductRepository + Clone {
        product_repository(self.db.clone())
    }

    /// The specialised order repository.
    #[must_use]
    pub fn orders(&self) -> impl OrderRepository + Clone {
        order_repository(self.db.clone())
    }

    async fn begin(&mut self) -> AppResult<()> {
        if self.db.in_transaction() {
            return Err(invalid("A transaction is already in progress"));
        }
        let mut conn = self.db.lock().await?;
        exec_raw(&mut conn, "BEGIN").await?;
        self.db.set_in_transaction(true);
        self.began = true;
        Ok(())
    }

    async fn commit_inner(&mut self) -> AppResult<u64> {
        let rows = self.db.pending_writes();
        if self.db.in_transaction() {
            let mut conn = self.db.lock().await?;
            exec_raw(&mut conn, "COMMIT").await?;
            self.db.set_in_transaction(false);
        }
        Ok(rows)
    }
}

#[async_trait]
impl UnitOfWork for SqliteUnitOfWork {
    fn has_changes(&self) -> bool {
        self.db.pending_writes() > 0
    }

    fn is_committed(&self) -> bool {
        self.committed
    }

    fn is_rolled_back(&self) -> bool {
        self.rolled_back
    }

    async fn commit(&mut self) -> AppResult<u64> {
        if self.committed {
            return Err(invalid("This unit of work has already been committed"));
        }
        if self.rolled_back {
            return Err(invalid("This unit of work has been rolled back"));
        }
        match self.commit_inner().await {
            Ok(rows) => {
                self.committed = true;
                Ok(rows)
            }
            Err(e) => {
                let _ = self.rollback().await;
                Err(e)
            }
        }
    }

    async fn rollback(&mut self) -> AppResult<()> {
        if self.rolled_back {
            return Ok(());
        }
        self.rolled_back = true;
        if self.db.in_transaction() {
            self.db.set_in_transaction(false);
            let mut conn = self.db.lock().await?;
            exec_raw(&mut conn, "ROLLBACK").await?;
        }
        Ok(())
    }

    async fn begin_transaction(&mut self) -> AppResult<Box<dyn TransactionScope>> {
        self.begin().await?;
        Ok(Box::new(SqliteTransactionScope {
            db: self.db.clone(),
            id: Uuid::new_v4(),
            active: true,
        }))
    }
}

impl Drop for SqliteUnitOfWork {
    fn drop(&mut self) {
        if self.began && !self.committed && !self.rolled_back && self.db.in_transaction() {
            self.db.defer_rollback();
        }
    }
}

/// C# `TransactionScopeImpl`.
pub struct SqliteTransactionScope {
    db: Db,
    id: Uuid,
    active: bool,
}

#[async_trait]
impl TransactionScope for SqliteTransactionScope {
    fn transaction_id(&self) -> Uuid {
        self.id
    }

    fn is_active(&self) -> bool {
        self.active
    }

    async fn commit(&mut self) -> AppResult<()> {
        if !self.active {
            return Err(invalid("Transaction is no longer active"));
        }
        let mut conn = self.db.lock().await?;
        exec_raw(&mut conn, "COMMIT").await?;
        self.active = false;
        self.db.set_in_transaction(false);
        Ok(())
    }

    async fn rollback(&mut self) -> AppResult<()> {
        if !self.active {
            return Ok(());
        }
        let mut conn = self.db.lock().await?;
        exec_raw(&mut conn, "ROLLBACK").await?;
        self.active = false;
        self.db.set_in_transaction(false);
        Ok(())
    }
}

impl Drop for SqliteTransactionScope {
    fn drop(&mut self) {
        if self.active && self.db.in_transaction() {
            self.db.defer_rollback();
        }
    }
}

/// C# `UnitOfWorkFactoryImpl`.
#[derive(Clone)]
pub struct SqliteUnitOfWorkFactory {
    db: Db,
}

impl SqliteUnitOfWorkFactory {
    #[must_use]
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

#[async_trait]
impl UnitOfWorkFactory for SqliteUnitOfWorkFactory {
    type Unit = SqliteUnitOfWork;

    fn create(&self) -> SqliteUnitOfWork {
        SqliteUnitOfWork::new(self.db.clone())
    }

    async fn create_with_transaction(&self) -> AppResult<SqliteUnitOfWork> {
        let mut uow = self.create();
        uow.begin().await?;
        Ok(uow)
    }
}
