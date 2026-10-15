//! Plugin tables (C# `Arcana.Plugins.Data`: `PluginVersionEntity`, `PluginMetadataEntity`,
//! `PluginHealthSnapshotEntity` and the queries of `PluginVersionRepositoryImpl`).
//!
//! The rows are plain storage types — this crate must not see plugin contracts. Mapping to
//! `arcana-plugin-api` types happens in `arcana-infra::plugin_store`.

use arcana_core::{AppResult, Timestamp};
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use crate::db::{Db, db_error};
use crate::row::{FromSql, SqlValue, ToSql, execute_on, scalar_i64};

/// Snapshots kept per plugin (C# "Keep only last 1000 snapshots per plugin").
pub const MAX_HEALTH_SNAPSHOTS: i64 = 1000;

/// C# `PluginVersionEntity` (without the surrogate `Id`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginVersionRow {
    pub plugin_id: String,
    pub version: String,
    pub installed_at: Timestamp,
    pub backup_path: String,
    pub is_current: bool,
    pub size_bytes: i64,
    pub release_notes: Option<String>,
    pub checksum: Option<String>,
}

/// C# `PluginMetadataEntity`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginMetadataRow {
    pub plugin_id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub plugin_type: i64,
    pub state: i64,
    pub icon_path: Option<String>,
    pub install_path: String,
    pub installed_at: Timestamp,
    pub last_activated_at: Option<Timestamp>,
    pub is_built_in: bool,
    pub dependencies_json: Option<String>,
    pub metadata_json: Option<String>,
    pub error_count: i64,
    pub last_error_at: Option<Timestamp>,
    pub last_error: Option<String>,
}

/// C# `PluginHealthSnapshotEntity`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginHealthSnapshotRow {
    pub plugin_id: String,
    pub checked_at: Timestamp,
    pub health_state: i64,
    pub message: Option<String>,
    pub response_time_ms: i64,
    pub memory_usage_bytes: i64,
    pub error_count: i64,
    pub details_json: Option<String>,
}

fn opt<T: ToSql>(v: Option<&T>) -> SqlValue {
    v.map_or(SqlValue::Null, ToSql::to_sql)
}

fn int(row: &SqliteRow, col: &str) -> AppResult<i64> {
    row.try_get::<i64, _>(col).map_err(db_error)
}

fn version_from_row(r: &SqliteRow) -> AppResult<PluginVersionRow> {
    Ok(PluginVersionRow {
        plugin_id: FromSql::from_sql(r, "PluginId")?,
        version: FromSql::from_sql(r, "Version")?,
        installed_at: FromSql::from_sql(r, "InstalledAt")?,
        backup_path: FromSql::from_sql(r, "BackupPath")?,
        is_current: FromSql::from_sql(r, "IsCurrent")?,
        size_bytes: int(r, "SizeBytes")?,
        release_notes: FromSql::from_sql(r, "ReleaseNotes")?,
        checksum: FromSql::from_sql(r, "Checksum")?,
    })
}

fn metadata_from_row(r: &SqliteRow) -> AppResult<PluginMetadataRow> {
    Ok(PluginMetadataRow {
        plugin_id: FromSql::from_sql(r, "PluginId")?,
        name: FromSql::from_sql(r, "Name")?,
        version: FromSql::from_sql(r, "Version")?,
        description: FromSql::from_sql(r, "Description")?,
        author: FromSql::from_sql(r, "Author")?,
        plugin_type: int(r, "Type")?,
        state: int(r, "State")?,
        icon_path: FromSql::from_sql(r, "IconPath")?,
        install_path: FromSql::from_sql(r, "InstallPath")?,
        installed_at: FromSql::from_sql(r, "InstalledAt")?,
        last_activated_at: FromSql::from_sql(r, "LastActivatedAt")?,
        is_built_in: FromSql::from_sql(r, "IsBuiltIn")?,
        dependencies_json: FromSql::from_sql(r, "DependenciesJson")?,
        metadata_json: FromSql::from_sql(r, "MetadataJson")?,
        error_count: int(r, "ErrorCount")?,
        last_error_at: FromSql::from_sql(r, "LastErrorAt")?,
        last_error: FromSql::from_sql(r, "LastError")?,
    })
}

fn snapshot_from_row(r: &SqliteRow) -> AppResult<PluginHealthSnapshotRow> {
    Ok(PluginHealthSnapshotRow {
        plugin_id: FromSql::from_sql(r, "PluginId")?,
        checked_at: FromSql::from_sql(r, "CheckedAt")?,
        health_state: int(r, "HealthState")?,
        message: FromSql::from_sql(r, "Message")?,
        response_time_ms: int(r, "ResponseTimeMs")?,
        memory_usage_bytes: int(r, "MemoryUsageBytes")?,
        error_count: int(r, "ErrorCount")?,
        details_json: FromSql::from_sql(r, "DetailsJson")?,
    })
}

/// Queries over the three plugin tables, sharing the app's [`Db`] connection.
#[derive(Clone)]
pub struct PluginStoreDao {
    db: Db,
}

impl PluginStoreDao {
    #[must_use]
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    async fn rows<T>(
        &self,
        sql: &str,
        params: Vec<SqlValue>,
        map: fn(&SqliteRow) -> AppResult<T>,
    ) -> AppResult<Vec<T>> {
        let mut conn = self.db.lock().await?;
        let rows = crate::row::bind_all(sqlx::query(sql), params)
            .fetch_all(&mut *conn)
            .await
            .map_err(db_error)?;
        rows.iter().map(map).collect()
    }

    async fn exec(&self, sql: &str, params: Vec<SqlValue>) -> AppResult<u64> {
        let mut conn = self.db.lock().await?;
        execute_on(&mut conn, sql, params).await
    }

    /// C# `GetVersionsAsync`: newest `InstalledAt` first.
    ///
    /// # Errors
    /// Database failure.
    pub async fn versions(&self, plugin_id: &str) -> AppResult<Vec<PluginVersionRow>> {
        self.rows(
            "SELECT * FROM \"PluginVersions\" WHERE \"PluginId\" = ? ORDER BY \"InstalledAt\" DESC, \"Id\" DESC",
            vec![plugin_id.to_owned().to_sql()],
            version_from_row,
        )
        .await
    }

    /// C# `GetVersionAsync`.
    ///
    /// # Errors
    /// Database failure.
    pub async fn version(
        &self,
        plugin_id: &str,
        version: &str,
    ) -> AppResult<Option<PluginVersionRow>> {
        Ok(self
            .rows(
                "SELECT * FROM \"PluginVersions\" WHERE \"PluginId\" = ? AND \"Version\" = ? LIMIT 1",
                vec![plugin_id.to_owned().to_sql(), version.to_owned().to_sql()],
                version_from_row,
            )
            .await?
            .into_iter()
            .next())
    }

    /// C# `GetCurrentVersionAsync`.
    ///
    /// # Errors
    /// Database failure.
    pub async fn current_version(&self, plugin_id: &str) -> AppResult<Option<PluginVersionRow>> {
        Ok(self
            .rows(
                "SELECT * FROM \"PluginVersions\" WHERE \"PluginId\" = ? AND \"IsCurrent\" = 1 LIMIT 1",
                vec![plugin_id.to_owned().to_sql()],
                version_from_row,
            )
            .await?
            .into_iter()
            .next())
    }

    /// C# `AddVersionAsync`: a new current version clears the old current flag. Both writes
    /// run in one transaction.
    ///
    /// # Errors
    /// `DuplicateEntry` for an existing `(PluginId, Version)`.
    pub async fn add_version(&self, row: &PluginVersionRow) -> AppResult<()> {
        let mut conn = self.db.lock().await?;
        crate::db::exec_raw(&mut conn, "SAVEPOINT plugin_add_version").await?;
        let result = async {
            if row.is_current {
                execute_on(
                    &mut conn,
                    "UPDATE \"PluginVersions\" SET \"IsCurrent\" = 0 WHERE \"PluginId\" = ? AND \"IsCurrent\" = 1",
                    vec![row.plugin_id.clone().to_sql()],
                )
                .await?;
            }
            execute_on(
                &mut conn,
                "INSERT INTO \"PluginVersions\" (\"PluginId\", \"Version\", \"InstalledAt\", \"BackupPath\", \
                 \"IsCurrent\", \"SizeBytes\", \"ReleaseNotes\", \"Checksum\") VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                vec![
                    row.plugin_id.clone().to_sql(),
                    row.version.clone().to_sql(),
                    row.installed_at.to_sql(),
                    row.backup_path.clone().to_sql(),
                    row.is_current.to_sql(),
                    SqlValue::Int(row.size_bytes),
                    opt(row.release_notes.as_ref()),
                    opt(row.checksum.as_ref()),
                ],
            )
            .await
        }
        .await;
        finish_savepoint(&mut conn, "plugin_add_version", result).await
    }

    /// C# `UpdateVersionAsync` (no-op when missing): updates `IsCurrent`, size and notes.
    ///
    /// # Errors
    /// Database failure.
    pub async fn update_version(&self, row: &PluginVersionRow) -> AppResult<()> {
        let mut conn = self.db.lock().await?;
        crate::db::exec_raw(&mut conn, "SAVEPOINT plugin_update_version").await?;
        let result = async {
            if row.is_current {
                execute_on(
                    &mut conn,
                    "UPDATE \"PluginVersions\" SET \"IsCurrent\" = 0 WHERE \"PluginId\" = ? AND \"Version\" <> ?",
                    vec![row.plugin_id.clone().to_sql(), row.version.clone().to_sql()],
                )
                .await?;
            }
            execute_on(
                &mut conn,
                "UPDATE \"PluginVersions\" SET \"IsCurrent\" = ?, \"SizeBytes\" = ?, \"ReleaseNotes\" = ? \
                 WHERE \"PluginId\" = ? AND \"Version\" = ?",
                vec![
                    row.is_current.to_sql(),
                    SqlValue::Int(row.size_bytes),
                    opt(row.release_notes.as_ref()),
                    row.plugin_id.clone().to_sql(),
                    row.version.clone().to_sql(),
                ],
            )
            .await
        }
        .await;
        finish_savepoint(&mut conn, "plugin_update_version", result).await
    }

    /// C# `DeleteVersionAsync`.
    ///
    /// # Errors
    /// Database failure.
    pub async fn delete_version(&self, plugin_id: &str, version: &str) -> AppResult<()> {
        self.exec(
            "DELETE FROM \"PluginVersions\" WHERE \"PluginId\" = ? AND \"Version\" = ?",
            vec![plugin_id.to_owned().to_sql(), version.to_owned().to_sql()],
        )
        .await
        .map(|_| ())
    }

    /// C# `DeleteAllVersionsAsync`.
    ///
    /// # Errors
    /// Database failure.
    pub async fn delete_all_versions(&self, plugin_id: &str) -> AppResult<()> {
        self.exec(
            "DELETE FROM \"PluginVersions\" WHERE \"PluginId\" = ?",
            vec![plugin_id.to_owned().to_sql()],
        )
        .await
        .map(|_| ())
    }

    /// C# `SetCurrentVersionAsync`: exactly the named version becomes current (none when it
    /// does not exist). One statement, so atomic without an explicit transaction.
    ///
    /// # Errors
    /// Database failure.
    pub async fn set_current_version(&self, plugin_id: &str, version: &str) -> AppResult<()> {
        self.exec(
            "UPDATE \"PluginVersions\" SET \"IsCurrent\" = CASE WHEN \"Version\" = ? THEN 1 ELSE 0 END \
             WHERE \"PluginId\" = ?",
            vec![version.to_owned().to_sql(), plugin_id.to_owned().to_sql()],
        )
        .await
        .map(|_| ())
    }

    /// C# `RecordHealthSnapshotAsync`, trimming to the newest [`MAX_HEALTH_SNAPSHOTS`].
    ///
    /// # Errors
    /// Database failure.
    pub async fn record_health_snapshot(&self, row: &PluginHealthSnapshotRow) -> AppResult<()> {
        self.exec(
            "INSERT INTO \"PluginHealthSnapshots\" (\"PluginId\", \"CheckedAt\", \"HealthState\", \"Message\", \
             \"ResponseTimeMs\", \"MemoryUsageBytes\", \"ErrorCount\", \"DetailsJson\") VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            vec![
                row.plugin_id.clone().to_sql(),
                row.checked_at.to_sql(),
                SqlValue::Int(row.health_state),
                opt(row.message.as_ref()),
                SqlValue::Int(row.response_time_ms),
                SqlValue::Int(row.memory_usage_bytes),
                SqlValue::Int(row.error_count),
                opt(row.details_json.as_ref()),
            ],
        )
        .await?;
        self.exec(
            "DELETE FROM \"PluginHealthSnapshots\" WHERE \"PluginId\" = ? AND \"Id\" NOT IN (\
             SELECT \"Id\" FROM \"PluginHealthSnapshots\" WHERE \"PluginId\" = ? \
             ORDER BY \"CheckedAt\" DESC, \"Id\" DESC LIMIT ?)",
            vec![
                row.plugin_id.clone().to_sql(),
                row.plugin_id.clone().to_sql(),
                SqlValue::Int(MAX_HEALTH_SNAPSHOTS),
            ],
        )
        .await
        .map(|_| ())
    }

    /// C# `GetHealthHistoryAsync`: `from <= CheckedAt <= to`, newest first.
    ///
    /// # Errors
    /// Database failure.
    pub async fn health_history(
        &self,
        plugin_id: &str,
        from: Timestamp,
        to: Timestamp,
    ) -> AppResult<Vec<PluginHealthSnapshotRow>> {
        self.rows(
            "SELECT * FROM \"PluginHealthSnapshots\" WHERE \"PluginId\" = ? AND \"CheckedAt\" >= ? \
             AND \"CheckedAt\" <= ? ORDER BY \"CheckedAt\" DESC, \"Id\" DESC",
            vec![plugin_id.to_owned().to_sql(), from.to_sql(), to.to_sql()],
            snapshot_from_row,
        )
        .await
    }

    /// Number of stored snapshots for a plugin.
    ///
    /// # Errors
    /// Database failure.
    pub async fn health_snapshot_count(&self, plugin_id: &str) -> AppResult<i64> {
        scalar_i64(
            &self.db,
            "SELECT COUNT(*) FROM \"PluginHealthSnapshots\" WHERE \"PluginId\" = ?",
            vec![plugin_id.to_owned().to_sql()],
        )
        .await
    }

    /// Insert or replace the metadata row of a plugin (C# declared the table but never wrote
    /// it).
    ///
    /// # Errors
    /// Database failure.
    pub async fn upsert_metadata(&self, m: &PluginMetadataRow) -> AppResult<()> {
        self.exec(
            "INSERT INTO \"PluginMetadata\" (\"PluginId\", \"Name\", \"Version\", \"Description\", \"Author\", \
             \"Type\", \"State\", \"IconPath\", \"InstallPath\", \"InstalledAt\", \"LastActivatedAt\", \
             \"IsBuiltIn\", \"DependenciesJson\", \"MetadataJson\", \"ErrorCount\", \"LastErrorAt\", \"LastError\") \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(\"PluginId\") DO UPDATE SET \"Name\" = excluded.\"Name\", \"Version\" = excluded.\"Version\", \
             \"Description\" = excluded.\"Description\", \"Author\" = excluded.\"Author\", \"Type\" = excluded.\"Type\", \
             \"State\" = excluded.\"State\", \"IconPath\" = excluded.\"IconPath\", \"InstallPath\" = excluded.\"InstallPath\", \
             \"InstalledAt\" = excluded.\"InstalledAt\", \"LastActivatedAt\" = excluded.\"LastActivatedAt\", \
             \"IsBuiltIn\" = excluded.\"IsBuiltIn\", \"DependenciesJson\" = excluded.\"DependenciesJson\", \
             \"MetadataJson\" = excluded.\"MetadataJson\", \"ErrorCount\" = excluded.\"ErrorCount\", \
             \"LastErrorAt\" = excluded.\"LastErrorAt\", \"LastError\" = excluded.\"LastError\"",
            vec![
                m.plugin_id.clone().to_sql(),
                m.name.clone().to_sql(),
                m.version.clone().to_sql(),
                opt(m.description.as_ref()),
                opt(m.author.as_ref()),
                SqlValue::Int(m.plugin_type),
                SqlValue::Int(m.state),
                opt(m.icon_path.as_ref()),
                m.install_path.clone().to_sql(),
                m.installed_at.to_sql(),
                opt(m.last_activated_at.as_ref()),
                m.is_built_in.to_sql(),
                opt(m.dependencies_json.as_ref()),
                opt(m.metadata_json.as_ref()),
                SqlValue::Int(m.error_count),
                opt(m.last_error_at.as_ref()),
                opt(m.last_error.as_ref()),
            ],
        )
        .await
        .map(|_| ())
    }

    /// # Errors
    /// Database failure.
    pub async fn metadata(&self, plugin_id: &str) -> AppResult<Option<PluginMetadataRow>> {
        Ok(self
            .rows(
                "SELECT * FROM \"PluginMetadata\" WHERE \"PluginId\" = ? LIMIT 1",
                vec![plugin_id.to_owned().to_sql()],
                metadata_from_row,
            )
            .await?
            .into_iter()
            .next())
    }

    /// # Errors
    /// Database failure.
    pub async fn all_metadata(&self) -> AppResult<Vec<PluginMetadataRow>> {
        self.rows(
            "SELECT * FROM \"PluginMetadata\" ORDER BY \"PluginId\"",
            vec![],
            metadata_from_row,
        )
        .await
    }

    /// # Errors
    /// Database failure.
    pub async fn delete_metadata(&self, plugin_id: &str) -> AppResult<()> {
        self.exec(
            "DELETE FROM \"PluginMetadata\" WHERE \"PluginId\" = ?",
            vec![plugin_id.to_owned().to_sql()],
        )
        .await
        .map(|_| ())
    }
}

async fn finish_savepoint(
    conn: &mut sqlx::SqliteConnection,
    name: &str,
    result: AppResult<u64>,
) -> AppResult<()> {
    match result {
        Ok(_) => crate::db::exec_raw(conn, &format!("RELEASE {name}"))
            .await
            .map(|_| ()),
        Err(e) => {
            // Best effort: the original error is what the caller needs to see.
            let _ = crate::db::exec_raw(conn, &format!("ROLLBACK TO {name}")).await;
            let _ = crate::db::exec_raw(conn, &format!("RELEASE {name}")).await;
            Err(e)
        }
    }
}
