//! Row mapping between domain structs and `SQLite` (the EF Core model-mapping role).
//!
//! Storage formats follow EF Core's `SQLite` provider: `decimal` → TEXT, `DateTime` → TEXT,
//! `Guid` → TEXT, `bool` / enums → INTEGER. Timestamps use a fixed-width
//! `YYYY-MM-DD HH:MM:SS.fffffffff` text so that text comparison equals time comparison.

use std::str::FromStr;

use arcana_core::{
    AppError, AppResult, AuditInfo, ErrorCode, PageRequest, SoftDeleteInfo, SyncInfo, Timestamp,
};
use rust_decimal::Decimal;
use sqlx::Row;
use sqlx::sqlite::{SqliteArguments, SqliteConnection, SqliteRow};
use uuid::Uuid;

use crate::db::{Db, db_error};

const TIMESTAMP_FORMAT: &str = "%Y-%m-%d %H:%M:%S%.9f";

/// A bindable SQL value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqlValue {
    Null,
    Int(i64),
    Text(String),
}

/// Rust value → [`SqlValue`].
pub trait ToSql {
    fn to_sql(&self) -> SqlValue;
}

/// Column of a row → Rust value.
pub trait FromSql: Sized {
    /// # Errors
    /// `DataCorruption` when the column is missing or holds an unexpected value.
    fn from_sql(row: &SqliteRow, column: &str) -> AppResult<Self>;
}

fn corrupt(column: &str, detail: impl std::fmt::Display) -> AppError {
    AppError::data(
        ErrorCode::DataCorruption,
        format!("column {column}: {detail}"),
    )
}

fn get<'r, T>(row: &'r SqliteRow, column: &str) -> AppResult<T>
where
    T: sqlx::Decode<'r, sqlx::Sqlite> + sqlx::Type<sqlx::Sqlite>,
{
    row.try_get::<T, _>(column).map_err(db_error)
}

impl<T: ToSql> ToSql for Option<T> {
    fn to_sql(&self) -> SqlValue {
        self.as_ref().map_or(SqlValue::Null, ToSql::to_sql)
    }
}

impl ToSql for String {
    fn to_sql(&self) -> SqlValue {
        SqlValue::Text(self.clone())
    }
}
impl FromSql for String {
    fn from_sql(row: &SqliteRow, column: &str) -> AppResult<Self> {
        get(row, column)
    }
}
impl FromSql for Option<String> {
    fn from_sql(row: &SqliteRow, column: &str) -> AppResult<Self> {
        get(row, column)
    }
}

impl ToSql for i32 {
    fn to_sql(&self) -> SqlValue {
        SqlValue::Int(i64::from(*self))
    }
}
impl FromSql for i32 {
    fn from_sql(row: &SqliteRow, column: &str) -> AppResult<Self> {
        get(row, column)
    }
}
impl FromSql for Option<i32> {
    fn from_sql(row: &SqliteRow, column: &str) -> AppResult<Self> {
        get(row, column)
    }
}

impl ToSql for bool {
    fn to_sql(&self) -> SqlValue {
        SqlValue::Int(i64::from(*self))
    }
}
impl FromSql for bool {
    fn from_sql(row: &SqliteRow, column: &str) -> AppResult<Self> {
        get(row, column)
    }
}

impl ToSql for Decimal {
    fn to_sql(&self) -> SqlValue {
        SqlValue::Text(self.to_string())
    }
}
fn parse_decimal(column: &str, s: &str) -> AppResult<Decimal> {
    Decimal::from_str(s).map_err(|e| corrupt(column, e))
}
impl FromSql for Decimal {
    fn from_sql(row: &SqliteRow, column: &str) -> AppResult<Self> {
        parse_decimal(column, &get::<String>(row, column)?)
    }
}
impl FromSql for Option<Decimal> {
    fn from_sql(row: &SqliteRow, column: &str) -> AppResult<Self> {
        get::<Option<String>>(row, column)?
            .map(|s| parse_decimal(column, &s))
            .transpose()
    }
}

impl ToSql for Timestamp {
    fn to_sql(&self) -> SqlValue {
        SqlValue::Text(self.format(TIMESTAMP_FORMAT).to_string())
    }
}
fn parse_timestamp(column: &str, s: &str) -> AppResult<Timestamp> {
    Timestamp::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f").map_err(|e| corrupt(column, e))
}
impl FromSql for Timestamp {
    fn from_sql(row: &SqliteRow, column: &str) -> AppResult<Self> {
        parse_timestamp(column, &get::<String>(row, column)?)
    }
}
impl FromSql for Option<Timestamp> {
    fn from_sql(row: &SqliteRow, column: &str) -> AppResult<Self> {
        get::<Option<String>>(row, column)?
            .map(|s| parse_timestamp(column, &s))
            .transpose()
    }
}

impl ToSql for Uuid {
    fn to_sql(&self) -> SqlValue {
        SqlValue::Text(self.to_string())
    }
}
impl FromSql for Uuid {
    fn from_sql(row: &SqliteRow, column: &str) -> AppResult<Self> {
        Uuid::from_str(&get::<String>(row, column)?).map_err(|e| corrupt(column, e))
    }
}

/// Implement [`ToSql`]/[`FromSql`] for an `int_enum!` type (stored as its integer value).
macro_rules! sql_int_enum {
    ($($t:ty),+ $(,)?) => {$(
        impl $crate::row::ToSql for $t {
            fn to_sql(&self) -> $crate::row::SqlValue {
                $crate::row::SqlValue::Int(i64::from(self.value()))
            }
        }
        impl $crate::row::FromSql for $t {
            fn from_sql(
                row: &sqlx::sqlite::SqliteRow,
                column: &str,
            ) -> arcana_core::AppResult<Self> {
                let v = <i32 as $crate::row::FromSql>::from_sql(row, column)?;
                <$t>::from_value(v).ok_or_else(|| {
                    arcana_core::AppError::data(
                        arcana_core::ErrorCode::DataCorruption,
                        format!("column {column}: unknown {} value {v}", stringify!($t)),
                    )
                })
            }
        }
    )+};
}
pub(crate) use sql_int_enum;

/// A struct persisted in one table with an `INTEGER` `Id` primary key.
///
/// The capability accessors mirror the C# `entity is AuditableEntity` / `SoftDeletable` /
/// `Syncable` checks in `RepositoryImpl`; the defaults say "not supported".
pub trait TableEntity: Clone + Send + Sync + Unpin + 'static {
    /// Table name.
    const TABLE: &'static str;
    /// Every column except `Id`, in the order of [`TableEntity::values`].
    const COLUMNS: &'static [&'static str];
    /// Whether reads apply the EF global query filter `IsDeleted = 0`.
    const QUERY_FILTER: bool;

    fn id(&self) -> i32;
    fn set_id(&mut self, id: i32);
    /// Values for [`TableEntity::COLUMNS`], in order.
    fn values(&self) -> Vec<SqlValue>;
    /// Build from a row selected with [`TableEntity::select_sql`].
    ///
    /// # Errors
    /// `DataCorruption` when a column is missing or malformed.
    fn from_row(row: &SqliteRow) -> AppResult<Self>;

    fn audit_mut(&mut self) -> Option<&mut AuditInfo> {
        None
    }
    fn soft_delete_mut(&mut self) -> Option<&mut SoftDeleteInfo> {
        None
    }
    fn sync_mut(&mut self) -> Option<&mut SyncInfo> {
        None
    }

    /// `SELECT "Id", <columns> FROM "<table>"`.
    #[must_use]
    fn select_sql() -> String {
        let cols: Vec<String> = Self::COLUMNS.iter().map(|c| format!("\"{c}\"")).collect();
        format!(
            "SELECT \"Id\", {} FROM \"{}\"",
            cols.join(", "),
            Self::TABLE
        )
    }

    /// `WHERE` clause body for the global query filter (`1` when there is none).
    #[must_use]
    fn filter_sql() -> &'static str {
        if Self::QUERY_FILTER {
            "\"IsDeleted\" = 0"
        } else {
            "1"
        }
    }

    /// Whether `column` may be used in `ORDER BY` (it must be a real column of the table).
    #[must_use]
    fn has_column(column: &str) -> bool {
        column == "Id" || Self::COLUMNS.contains(&column)
    }
}

pub(crate) type SqliteQuery<'q> = sqlx::query::Query<'q, sqlx::Sqlite, SqliteArguments<'q>>;

pub(crate) fn bind_all(mut q: SqliteQuery<'_>, values: Vec<SqlValue>) -> SqliteQuery<'_> {
    for v in values {
        q = match v {
            SqlValue::Null => q.bind(None::<String>),
            SqlValue::Int(i) => q.bind(i),
            SqlValue::Text(s) => q.bind(s),
        };
    }
    q
}

/// Run a `SELECT` and map every row.
pub(crate) async fn fetch_on<T: TableEntity>(
    conn: &mut SqliteConnection,
    sql: &str,
    params: Vec<SqlValue>,
) -> AppResult<Vec<T>> {
    let rows = bind_all(sqlx::query(sql), params)
        .fetch_all(conn)
        .await
        .map_err(db_error)?;
    rows.iter().map(T::from_row).collect()
}

pub(crate) async fn fetch<T: TableEntity>(
    db: &Db,
    sql: &str,
    params: Vec<SqlValue>,
) -> AppResult<Vec<T>> {
    let mut conn = db.lock().await?;
    fetch_on(&mut conn, sql, params).await
}

pub(crate) async fn fetch_one<T: TableEntity>(
    db: &Db,
    sql: &str,
    params: Vec<SqlValue>,
) -> AppResult<Option<T>> {
    Ok(fetch(db, sql, params).await?.into_iter().next())
}

pub(crate) async fn scalar_i64(db: &Db, sql: &str, params: Vec<SqlValue>) -> AppResult<i64> {
    let mut conn = db.lock().await?;
    let row = bind_all(sqlx::query(sql), params)
        .fetch_one(&mut *conn)
        .await
        .map_err(db_error)?;
    row.try_get::<i64, _>(0).map_err(db_error)
}

pub(crate) async fn execute_on(
    conn: &mut SqliteConnection,
    sql: &str,
    params: Vec<SqlValue>,
) -> AppResult<u64> {
    bind_all(sqlx::query(sql), params)
        .execute(conn)
        .await
        .map(|r| r.rows_affected())
        .map_err(db_error)
}

// Paging / sorting helpers shared by the DAOs and the generic repository. They live here, at
// the DAO level, so the DAO layer never reaches up into `repository`.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn malformed_values_are_data_corruption() {
        let db = Db::in_memory().await.unwrap();
        let mut c = db.lock().await.unwrap();
        let row = sqlx::query("SELECT 'x' AS d, 'not-a-date' AS t, 'nope' AS g, NULL AS n")
            .fetch_one(&mut *c)
            .await
            .unwrap();
        for e in [
            Decimal::from_sql(&row, "d").unwrap_err(),
            Option::<Decimal>::from_sql(&row, "d").unwrap_err(),
            Timestamp::from_sql(&row, "t").unwrap_err(),
            Option::<Timestamp>::from_sql(&row, "t").unwrap_err(),
            Uuid::from_sql(&row, "g").unwrap_err(),
            String::from_sql(&row, "missing").unwrap_err(),
        ] {
            assert_eq!(e.code(), ErrorCode::DataCorruption, "{e}");
        }
        assert_eq!(Option::<Decimal>::from_sql(&row, "n").unwrap(), None);
        assert_eq!(Option::<Timestamp>::from_sql(&row, "n").unwrap(), None);
        assert_eq!(None::<i32>.to_sql(), SqlValue::Null);
    }

    #[test]
    fn timestamps_are_fixed_width_text() {
        let t = chrono::NaiveDate::from_ymd_opt(2024, 1, 2)
            .unwrap()
            .and_hms_opt(3, 4, 5)
            .unwrap();
        assert_eq!(
            t.to_sql(),
            SqlValue::Text("2024-01-02 03:04:05.000000000".into())
        );
    }
}
