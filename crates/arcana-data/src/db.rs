//! Connection handle, migrations and error mapping (the C# `AppDbContext` connection role).
//!
//! A [`Db`] owns one `SQLite` connection behind an async mutex. Every DAO, repository and unit
//! of work created from the same `Db` shares that connection, so a transaction begun by a unit
//! of work covers all of them — the same sharing EF Core gets from one `DbContext`.

use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use arcana_core::{AppError, AppResult, ErrorCode};
use sqlx::Connection;
use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection};
use tokio::sync::{Mutex, MutexGuard};

/// Embedded migrations (`crates/arcana-data/migrations`).
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Shared database handle. Cloning is cheap and shares the connection.
#[derive(Clone)]
pub struct Db {
    inner: Arc<Inner>,
}

struct Inner {
    conn: Mutex<SqliteConnection>,
    in_transaction: AtomicBool,
    pending_writes: AtomicU64,
    rollback_pending: AtomicBool,
}

impl Db {
    /// Open (creating if missing) the database at `url`, enable foreign keys and run every
    /// pending migration. C# used `EnsureCreatedAsync`; this uses real migrations.
    ///
    /// # Errors
    /// `ConnectionFailed` when the URL is invalid or the file cannot be opened,
    /// `MigrationFailed` when a migration does not apply.
    pub async fn connect(url: &str) -> AppResult<Self> {
        let options = SqliteConnectOptions::from_str(url)
            .map_err(|e| data_error(ErrorCode::ConnectionFailed, &e))?
            .create_if_missing(true)
            .foreign_keys(true);
        let mut conn = SqliteConnection::connect_with(&options)
            .await
            .map_err(|e| data_error(ErrorCode::ConnectionFailed, &e))?;
        MIGRATOR
            .run(&mut conn)
            .await
            .map_err(|e| data_error(ErrorCode::MigrationFailed, &e))?;
        Ok(Self {
            inner: Arc::new(Inner {
                conn: Mutex::new(conn),
                in_transaction: AtomicBool::new(false),
                pending_writes: AtomicU64::new(0),
                rollback_pending: AtomicBool::new(false),
            }),
        })
    }

    /// A fresh, migrated in-memory database (`sqlite::memory:`).
    ///
    /// # Errors
    /// See [`Db::connect`].
    pub async fn in_memory() -> AppResult<Self> {
        Self::connect("sqlite::memory:").await
    }

    /// Exclusive access to the connection, for code that needs raw SQL (the C# tests reach
    /// into the `DbContext` the same way). A rollback deferred by a dropped transaction is
    /// executed first.
    ///
    /// # Errors
    /// `DatabaseError` when the deferred rollback fails.
    pub async fn lock(&self) -> AppResult<MutexGuard<'_, SqliteConnection>> {
        let mut guard = self.inner.conn.lock().await;
        if self.inner.rollback_pending.swap(false, Ordering::SeqCst) {
            exec_raw(&mut guard, "ROLLBACK").await?;
        }
        Ok(guard)
    }

    /// Whether an explicit (unit-of-work) transaction is open.
    #[must_use]
    pub fn in_transaction(&self) -> bool {
        self.inner.in_transaction.load(Ordering::SeqCst)
    }

    pub(crate) fn set_in_transaction(&self, value: bool) {
        self.inner.in_transaction.store(value, Ordering::SeqCst);
        self.inner.pending_writes.store(0, Ordering::SeqCst);
    }

    /// Rows written inside the open explicit transaction.
    pub(crate) fn pending_writes(&self) -> u64 {
        self.inner.pending_writes.load(Ordering::SeqCst)
    }

    pub(crate) fn record_writes(&self, rows: u64) {
        if self.in_transaction() {
            self.inner.pending_writes.fetch_add(rows, Ordering::SeqCst);
        }
    }

    /// Roll the open transaction back on the next [`Db::lock`] (used from `Drop`, which cannot
    /// await).
    pub(crate) fn defer_rollback(&self) {
        self.set_in_transaction(false);
        self.inner.rollback_pending.store(true, Ordering::SeqCst);
    }
}

pub(crate) async fn exec_raw(conn: &mut SqliteConnection, sql: &str) -> AppResult<u64> {
    sqlx::query(sql)
        .execute(conn)
        .await
        .map(|r| r.rows_affected())
        .map_err(db_error)
}

fn data_error(code: ErrorCode, e: &dyn std::error::Error) -> AppError {
    AppError::Data {
        code,
        message: e.to_string(),
        inner: None,
    }
}

/// Map a driver error to `AppError::Data` with the closest C# `ErrorCode`.
#[allow(
    clippy::needless_pass_by_value,
    reason = "shaped for `map_err(db_error)`"
)]
pub(crate) fn db_error(e: sqlx::Error) -> AppError {
    let code = match &e {
        sqlx::Error::Database(d) => match d.kind() {
            sqlx::error::ErrorKind::UniqueViolation => ErrorCode::DuplicateEntry,
            sqlx::error::ErrorKind::ForeignKeyViolation => ErrorCode::ReferentialIntegrity,
            _ => ErrorCode::DatabaseError,
        },
        sqlx::Error::ColumnNotFound(_)
        | sqlx::Error::ColumnDecode { .. }
        | sqlx::Error::Decode(_) => ErrorCode::DataCorruption,
        _ => ErrorCode::QueryFailed,
    };
    data_error(code, &e)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bad_url_is_connection_failed() {
        let Err(e) = Db::connect("postgres://nope").await else {
            panic!("expected error")
        };
        assert_eq!(e.code(), ErrorCode::ConnectionFailed);
    }

    #[tokio::test]
    async fn unopenable_file_is_connection_failed() {
        let Err(e) = Db::connect("sqlite:///nonexistent-dir/x/y.db").await else {
            panic!("expected error")
        };
        assert_eq!(e.code(), ErrorCode::ConnectionFailed);
    }

    #[tokio::test]
    async fn error_mapping() {
        let db = Db::in_memory().await.unwrap();
        let mut c = db.lock().await.unwrap();
        let e = exec_raw(&mut c, "SELECT * FROM nope").await.unwrap_err();
        assert_eq!(e.code(), ErrorCode::DatabaseError);
        let e = db_error(sqlx::Error::ColumnNotFound("x".into()));
        assert_eq!(e.code(), ErrorCode::DataCorruption);
        let e = db_error(sqlx::Error::PoolTimedOut);
        assert_eq!(e.code(), ErrorCode::QueryFailed);
    }

    #[tokio::test]
    async fn migrations_create_the_thirteen_tables() {
        let db = Db::in_memory().await.unwrap();
        let mut c = db.lock().await.unwrap();
        let names: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type = 'table' \
             AND name NOT LIKE 'sqlite_%' AND name <> '_sqlx_migrations' ORDER BY name",
        )
        .fetch_all(&mut *c)
        .await
        .unwrap();
        let names: Vec<_> = names.into_iter().map(|(n,)| n).collect();
        assert_eq!(
            names,
            [
                "AuditLogs",
                "Customers",
                "OrderItems",
                "Orders",
                "Permissions",
                "ProductCategories",
                "Products",
                "RolePermissions",
                "Roles",
                "SyncQueue",
                "UserPermissions",
                "UserRoles",
                "Users"
            ]
        );
    }
}
