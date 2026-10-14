//! Generic repository over any [`TableEntity`] (C# `RepositoryImpl<TEntity, TKey>`).

use std::marker::PhantomData;

use arcana_core::{
    AppError, AppResult, ErrorCode, PageRequest, PagedResult, Predicate, Repository, utc_now,
};
use async_trait::async_trait;
use sqlx::sqlite::SqliteConnection;

use crate::db::{Db, exec_raw};
use crate::row::{SqlValue, TableEntity, ToSql, execute_on, fetch, fetch_one, scalar_i64};

/// Build `ORDER BY` for a caller-supplied property name. Only real columns are accepted, which
/// both mirrors EF (`EF.Property` on an unknown name throws) and rules out SQL injection.
pub(crate) fn order_by<T: TableEntity>(request: &PageRequest) -> AppResult<Option<String>> {
    match request.sort_by.as_deref().filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(col) if T::has_column(col) => Ok(Some(format!(
            " ORDER BY \"{col}\" {}",
            if request.descending { "DESC" } else { "ASC" }
        ))),
        Some(col) => Err(AppError::data(
            ErrorCode::QueryFailed,
            format!("unknown sort column '{col}' for {}", T::TABLE),
        )),
    }
}

pub(crate) fn limit_offset(request: &PageRequest) -> [SqlValue; 2] {
    [
        SqlValue::Int(i64::from(request.page_size)),
        SqlValue::Int(i64::try_from(request.skip()).unwrap_or(i64::MAX)),
    ]
}

pub(crate) fn to_u64(n: i64) -> u64 {
    u64::try_from(n).unwrap_or(0)
}

/// Start an implicit transaction for a multi-statement write unless a unit of work already
/// has one open. Returns whether this call started it.
pub(crate) async fn begin_implicit(db: &Db, conn: &mut SqliteConnection) -> AppResult<bool> {
    if db.in_transaction() {
        Ok(false)
    } else {
        exec_raw(conn, "BEGIN").await?;
        Ok(true)
    }
}

/// Commit (on success) or roll back (on failure) an implicit transaction.
pub(crate) async fn finish_implicit<R>(
    conn: &mut SqliteConnection,
    started: bool,
    result: AppResult<R>,
) -> AppResult<R> {
    if started {
        if result.is_ok() {
            exec_raw(conn, "COMMIT").await?;
        } else {
            // The original error is the one worth reporting.
            let _ = exec_raw(conn, "ROLLBACK").await;
        }
    }
    result
}

/// Insert one row; sets `CreatedAt` (C# `AppDbContext.UpdateAuditFields`) and
/// `IsPendingSync`, honours a caller-chosen non-zero `Id`, and writes back the generated key.
pub(crate) async fn insert_on<T: TableEntity>(
    db: &Db,
    conn: &mut SqliteConnection,
    entity: &mut T,
) -> AppResult<()> {
    if let Some(sync) = entity.sync_mut() {
        sync.is_pending_sync = true;
    }
    if let Some(audit) = entity.audit_mut() {
        audit.created_at = utc_now();
    }
    let mut cols: Vec<&str> = T::COLUMNS.to_vec();
    let mut values = entity.values();
    if entity.id() != 0 {
        cols.insert(0, "Id");
        values.insert(0, entity.id().to_sql());
    }
    let names: Vec<String> = cols.iter().map(|c| format!("\"{c}\"")).collect();
    let marks = vec!["?"; cols.len()].join(", ");
    let sql = format!(
        "INSERT INTO \"{}\" ({}) VALUES ({marks})",
        T::TABLE,
        names.join(", ")
    );
    let rows = execute_on(conn, &sql, values).await?;
    db.record_writes(rows);
    if entity.id() == 0 {
        let id: (i64,) = sqlx::query_as("SELECT last_insert_rowid()")
            .fetch_one(&mut *conn)
            .await
            .map_err(crate::db::db_error)?;
        entity.set_id(
            i32::try_from(id.0).map_err(|_| {
                AppError::data(ErrorCode::ValueOutOfRange, "generated id exceeds i32")
            })?,
        );
    }
    Ok(())
}

/// Update every column of one row by `Id`; sets `ModifiedAt` and `IsPendingSync`.
/// Updating a row that does not exist is a `ConflictError` (EF throws
/// `DbUpdateConcurrencyException`).
pub(crate) async fn update_on<T: TableEntity>(
    db: &Db,
    conn: &mut SqliteConnection,
    entity: &mut T,
) -> AppResult<()> {
    if let Some(sync) = entity.sync_mut() {
        sync.is_pending_sync = true;
    }
    if let Some(audit) = entity.audit_mut() {
        audit.modified_at = Some(utc_now());
    }
    let sets: Vec<String> = T::COLUMNS.iter().map(|c| format!("\"{c}\" = ?")).collect();
    let sql = format!(
        "UPDATE \"{}\" SET {} WHERE \"Id\" = ?",
        T::TABLE,
        sets.join(", ")
    );
    let mut values = entity.values();
    values.push(entity.id().to_sql());
    let rows = execute_on(conn, &sql, values).await?;
    if rows == 0 {
        return Err(AppError::data(
            ErrorCode::ConflictError,
            format!("{} row {} does not exist", T::TABLE, entity.id()),
        ));
    }
    db.record_writes(rows);
    Ok(())
}

/// C# `RepositoryImpl<TEntity>` on `SQLite`.
pub struct SqliteRepository<T> {
    db: Db,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Clone for SqliteRepository<T> {
    fn clone(&self) -> Self {
        Self::new(self.db.clone())
    }
}

impl<T> SqliteRepository<T> {
    #[must_use]
    pub fn new(db: Db) -> Self {
        Self {
            db,
            _marker: PhantomData,
        }
    }

    #[must_use]
    pub fn db(&self) -> &Db {
        &self.db
    }
}

impl<T: TableEntity> SqliteRepository<T> {
    fn select_filtered(extra: &str) -> String {
        format!("{} WHERE {}{extra}", T::select_sql(), T::filter_sql())
    }
}

#[async_trait]
impl<T: TableEntity> Repository<T> for SqliteRepository<T> {
    async fn get_by_id(&self, id: i32) -> AppResult<Option<T>> {
        let sql = Self::select_filtered(" AND \"Id\" = ?");
        fetch_one(&self.db, &sql, vec![id.to_sql()]).await
    }

    async fn get_all(&self) -> AppResult<Vec<T>> {
        fetch(&self.db, &Self::select_filtered(""), vec![]).await
    }

    async fn get_paged(&self, request: &PageRequest) -> AppResult<PagedResult<T>> {
        let order = order_by::<T>(request)?.unwrap_or_default();
        let count_sql = format!(
            "SELECT COUNT(*) FROM \"{}\" WHERE {}",
            T::TABLE,
            T::filter_sql()
        );
        let total = scalar_i64(&self.db, &count_sql, vec![]).await?;
        let sql = Self::select_filtered(&format!("{order} LIMIT ? OFFSET ?"));
        let items = fetch(&self.db, &sql, limit_offset(request).to_vec()).await?;
        Ok(PagedResult::new(
            items,
            request.page,
            request.page_size,
            to_u64(total),
        ))
    }

    async fn find(&self, predicate: Predicate<'_, T>) -> AppResult<Vec<T>> {
        Ok(self
            .get_all()
            .await?
            .into_iter()
            .filter(|e| predicate(e))
            .collect())
    }

    async fn first_or_default(&self, predicate: Predicate<'_, T>) -> AppResult<Option<T>> {
        Ok(self.get_all().await?.into_iter().find(|e| predicate(e)))
    }

    async fn any(&self, predicate: Predicate<'_, T>) -> AppResult<bool> {
        Ok(self.get_all().await?.iter().any(predicate))
    }

    async fn count(&self, predicate: Option<Predicate<'_, T>>) -> AppResult<u64> {
        match predicate {
            None => {
                let sql = format!(
                    "SELECT COUNT(*) FROM \"{}\" WHERE {}",
                    T::TABLE,
                    T::filter_sql()
                );
                Ok(to_u64(scalar_i64(&self.db, &sql, vec![]).await?))
            }
            Some(p) => Ok(self.find(p).await?.len() as u64),
        }
    }

    async fn add(&self, mut entity: T) -> AppResult<T> {
        let mut conn = self.db.lock().await?;
        insert_on(&self.db, &mut conn, &mut entity).await?;
        Ok(entity)
    }

    /// All rows or none, like the single `SaveChanges` in C#.
    async fn add_range(&self, entities: Vec<T>) -> AppResult<()> {
        let mut conn = self.db.lock().await?;
        let started = begin_implicit(&self.db, &mut conn).await?;
        let result = async {
            for mut e in entities {
                insert_on(&self.db, &mut conn, &mut e).await?;
            }
            Ok(())
        }
        .await;
        finish_implicit(&mut conn, started, result).await
    }

    async fn update(&self, entity: &mut T) -> AppResult<()> {
        let mut conn = self.db.lock().await?;
        update_on(&self.db, &mut conn, entity).await
    }

    async fn delete(&self, entity: &mut T) -> AppResult<()> {
        if let Some(sd) = entity.soft_delete_mut() {
            sd.is_deleted = true;
            sd.deleted_at = Some(utc_now());
            return self.update(entity).await;
        }
        let mut conn = self.db.lock().await?;
        let sql = format!("DELETE FROM \"{}\" WHERE \"Id\" = ?", T::TABLE);
        let rows = execute_on(&mut conn, &sql, vec![entity.id().to_sql()]).await?;
        self.db.record_writes(rows);
        Ok(())
    }

    async fn delete_by_id(&self, id: i32) -> AppResult<()> {
        if let Some(mut e) = self.get_by_id(id).await? {
            self.delete(&mut e).await?;
        }
        Ok(())
    }
}

/// Implement `Repository<$entity>` for a specialised repository by delegating to its
/// `base: SqliteRepository<$entity>` field (C# specialised repositories inherit
/// `RepositoryImpl<T>`).
macro_rules! delegate_repository {
    ($repo:ty, $entity:ty) => {
        #[async_trait::async_trait]
        impl arcana_core::Repository<$entity> for $repo {
            async fn get_by_id(&self, id: i32) -> arcana_core::AppResult<Option<$entity>> {
                self.base.get_by_id(id).await
            }
            async fn get_all(&self) -> arcana_core::AppResult<Vec<$entity>> {
                self.base.get_all().await
            }
            async fn get_paged(
                &self,
                request: &arcana_core::PageRequest,
            ) -> arcana_core::AppResult<arcana_core::PagedResult<$entity>> {
                self.base.get_paged(request).await
            }
            async fn find(
                &self,
                predicate: arcana_core::Predicate<'_, $entity>,
            ) -> arcana_core::AppResult<Vec<$entity>> {
                self.base.find(predicate).await
            }
            async fn first_or_default(
                &self,
                predicate: arcana_core::Predicate<'_, $entity>,
            ) -> arcana_core::AppResult<Option<$entity>> {
                self.base.first_or_default(predicate).await
            }
            async fn any(
                &self,
                predicate: arcana_core::Predicate<'_, $entity>,
            ) -> arcana_core::AppResult<bool> {
                self.base.any(predicate).await
            }
            async fn count(
                &self,
                predicate: Option<arcana_core::Predicate<'_, $entity>>,
            ) -> arcana_core::AppResult<u64> {
                self.base.count(predicate).await
            }
            async fn add(&self, entity: $entity) -> arcana_core::AppResult<$entity> {
                self.add_impl(entity).await
            }
            async fn add_range(&self, entities: Vec<$entity>) -> arcana_core::AppResult<()> {
                self.add_range_impl(entities).await
            }
            async fn update(&self, entity: &mut $entity) -> arcana_core::AppResult<()> {
                self.update_impl(entity).await
            }
            async fn delete(&self, entity: &mut $entity) -> arcana_core::AppResult<()> {
                self.base.delete(entity).await
            }
            async fn delete_by_id(&self, id: i32) -> arcana_core::AppResult<()> {
                self.base.delete_by_id(id).await
            }
        }
    };
}
pub(crate) use delegate_repository;
