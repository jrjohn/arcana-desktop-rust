//! Audit log writing (the `LogAuditAsync` helpers of C# `AuthServiceImpl` and the
//! access-denied entry of `AuthorizationServiceImpl`).

use arcana_core::{AppResult, Repository, utc_now};
use arcana_data::{Db, SqliteRepository};
use arcana_domain::{AuditEventType, AuditLog};

/// One audit entry to record.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuditEntry {
    pub event_type: AuditEventType,
    pub user_id: Option<i32>,
    pub username: Option<String>,
    pub resource: Option<String>,
    /// Defaults to the event type name (C# `eventType.ToString()`).
    pub action: Option<String>,
    pub details: Option<String>,
    pub is_success: bool,
}

/// Writes rows to `AuditLogs`.
#[derive(Clone)]
pub struct AuditLogger {
    repo: SqliteRepository<AuditLog>,
}

impl std::fmt::Debug for AuditLogger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AuditLogger")
    }
}

impl AuditLogger {
    #[must_use]
    pub fn new(db: Db) -> Self {
        Self {
            repo: SqliteRepository::new(db),
        }
    }

    /// Persist `entry`, stamped now, and return the stored row.
    ///
    /// # Errors
    /// Storage errors from the repository.
    pub async fn write(&self, entry: AuditEntry) -> AppResult<AuditLog> {
        self.repo
            .add(AuditLog {
                event_type: entry.event_type,
                user_id: entry.user_id,
                username: entry.username,
                client_info: None,
                resource: entry.resource,
                action: entry
                    .action
                    .unwrap_or_else(|| format!("{:?}", entry.event_type)),
                details: entry.details,
                is_success: entry.is_success,
                error_message: None,
                timestamp: utc_now(),
                ..AuditLog::default()
            })
            .await
    }

    /// Every entry, oldest first.
    ///
    /// # Errors
    /// Storage errors from the repository.
    pub async fn entries(&self) -> AppResult<Vec<AuditLog>> {
        let mut all = self.repo.get_all().await?;
        all.sort_by_key(|a| a.base.id);
        Ok(all)
    }
}
