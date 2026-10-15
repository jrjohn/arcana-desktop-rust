//! C# `PluginVersionRepositoryTests` (23) as store-agnostic async functions, so the same
//! suite runs against the in-memory store (here) and the `SQLite` store (`arcana-infra`,
//! included with `#[path]`). `run_suite!` expands one `#[tokio::test]` per case.
#![allow(clippy::unwrap_used, dead_code)]

use arcana_plugin_api::{HealthState, PluginHealthStatus, PluginVersionInfo};
use arcana_plugin_host::version_store::PluginVersionStore;
use chrono::{DateTime, Duration, Utc};

fn ver(s: &str) -> semver::Version {
    semver::Version::parse(s).unwrap()
}

fn info(id: &str, v: &str, at: DateTime<Utc>, current: bool) -> PluginVersionInfo {
    PluginVersionInfo {
        plugin_id: id.into(),
        version: ver(v),
        installed_at: at,
        backup_path: format!("/backup/{id}/{v}"),
        is_current: current,
        size_bytes: 0,
        release_notes: None,
    }
}

async fn add(s: &dyn PluginVersionStore, id: &str, v: &str, at: DateTime<Utc>, current: bool) {
    s.add_version(&info(id, v, at, current)).await.unwrap();
}

fn snapshot(state: HealthState, at: DateTime<Utc>) -> PluginHealthStatus {
    let mut st = PluginHealthStatus::new("plugin-1", "Test Plugin", state);
    st.checked_at = at;
    st
}

/// `GetVersionsAsync_EmptyDatabase_ShouldReturnEmptyList`
pub async fn get_versions_empty_database_should_return_empty_list(s: &dyn PluginVersionStore) {
    assert!(s.versions("plugin-1").await.unwrap().is_empty());
}

/// `GetVersionsAsync_WithVersions_ShouldReturnOrdered`
pub async fn get_versions_with_versions_should_return_ordered(s: &dyn PluginVersionStore) {
    let now = Utc::now();
    add(s, "plugin-1", "1.0.0", now - Duration::days(2), false).await;
    add(s, "plugin-1", "1.1.0", now - Duration::days(1), false).await;
    add(s, "plugin-1", "2.0.0", now, false).await;
    let v = s.versions("plugin-1").await.unwrap();
    assert_eq!(v.len(), 3);
    assert_eq!(v[0].version, ver("2.0.0"));
    assert_eq!(v[1].version, ver("1.1.0"));
    assert_eq!(v[2].version, ver("1.0.0"));
}

/// `GetVersionsAsync_DifferentPlugins_ShouldOnlyReturnRequestedPlugin`
pub async fn get_versions_different_plugins_should_only_return_requested_plugin(
    s: &dyn PluginVersionStore,
) {
    let now = Utc::now();
    add(s, "plugin-1", "1.0.0", now, false).await;
    add(s, "plugin-2", "1.0.0", now, false).await;
    add(s, "plugin-2", "2.0.0", now, false).await;
    let v = s.versions("plugin-2").await.unwrap();
    assert_eq!(v.len(), 2);
    assert!(v.iter().all(|x| x.plugin_id == "plugin-2"));
}

/// `GetVersionAsync_ExistingVersion_ShouldReturnVersion`
pub async fn get_version_existing_version_should_return_version(s: &dyn PluginVersionStore) {
    add(s, "plugin-1", "1.0.0", Utc::now(), false).await;
    let v = s.version("plugin-1", "1.0.0").await.unwrap().unwrap();
    assert_eq!(v.plugin_id, "plugin-1");
    assert_eq!(v.version, ver("1.0.0"));
}

/// `GetVersionAsync_NonExistingVersion_ShouldReturnNull`
pub async fn get_version_non_existing_version_should_return_null(s: &dyn PluginVersionStore) {
    assert!(s.version("plugin-1", "1.0.0").await.unwrap().is_none());
}

/// `GetVersionAsync_WrongPlugin_ShouldReturnNull`
pub async fn get_version_wrong_plugin_should_return_null(s: &dyn PluginVersionStore) {
    add(s, "plugin-1", "1.0.0", Utc::now(), false).await;
    assert!(s.version("plugin-2", "1.0.0").await.unwrap().is_none());
}

/// `GetCurrentVersionAsync_NoCurrent_ShouldReturnNull`
pub async fn get_current_version_no_current_should_return_null(s: &dyn PluginVersionStore) {
    add(s, "plugin-1", "1.0.0", Utc::now(), false).await;
    assert!(s.current_version("plugin-1").await.unwrap().is_none());
}

/// `GetCurrentVersionAsync_WithCurrent_ShouldReturnCurrent`
pub async fn get_current_version_with_current_should_return_current(s: &dyn PluginVersionStore) {
    add(s, "plugin-1", "1.0.0", Utc::now(), true).await;
    let c = s.current_version("plugin-1").await.unwrap().unwrap();
    assert!(c.is_current);
    assert_eq!(c.version, ver("1.0.0"));
}

/// `AddVersionAsync_ShouldAddVersion`
pub async fn add_version_should_add_version(s: &dyn PluginVersionStore) {
    let mut i = info("plugin-1", "1.0.0", Utc::now(), true);
    i.size_bytes = 1024;
    i.release_notes = Some("Initial release".into());
    s.add_version(&i).await.unwrap();
    let v = s.versions("plugin-1").await.unwrap();
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].size_bytes, 1024);
    assert_eq!(v[0].release_notes.as_deref(), Some("Initial release"));
}

/// `AddVersionAsync_NewCurrentVersion_ShouldUnsetPreviousCurrent`
pub async fn add_version_new_current_version_should_unset_previous_current(
    s: &dyn PluginVersionStore,
) {
    let now = Utc::now();
    add(s, "plugin-1", "1.0.0", now - Duration::days(1), true).await;
    add(s, "plugin-1", "2.0.0", now, true).await;
    assert!(
        !s.version("plugin-1", "1.0.0")
            .await
            .unwrap()
            .unwrap()
            .is_current
    );
    assert!(
        s.version("plugin-1", "2.0.0")
            .await
            .unwrap()
            .unwrap()
            .is_current
    );
}

/// `UpdateVersionAsync_ShouldUpdateVersion`
pub async fn update_version_should_update_version(s: &dyn PluginVersionStore) {
    add(s, "plugin-1", "1.0.0", Utc::now(), false).await;
    let mut i = info("plugin-1", "1.0.0", Utc::now(), false);
    i.size_bytes = 2048;
    i.release_notes = Some("Updated notes".into());
    s.update_version(&i).await.unwrap();
    let v = s.version("plugin-1", "1.0.0").await.unwrap().unwrap();
    assert_eq!(v.size_bytes, 2048);
    assert_eq!(v.release_notes.as_deref(), Some("Updated notes"));
}

/// `UpdateVersionAsync_SettingCurrent_ShouldUnsetOthers`
pub async fn update_version_setting_current_should_unset_others(s: &dyn PluginVersionStore) {
    let now = Utc::now();
    add(s, "plugin-1", "1.0.0", now - Duration::days(1), true).await;
    let v2 = info("plugin-1", "2.0.0", now, false);
    s.add_version(&v2).await.unwrap();
    s.update_version(&PluginVersionInfo {
        is_current: true,
        ..v2
    })
    .await
    .unwrap();
    assert!(
        !s.version("plugin-1", "1.0.0")
            .await
            .unwrap()
            .unwrap()
            .is_current
    );
    assert!(
        s.version("plugin-1", "2.0.0")
            .await
            .unwrap()
            .unwrap()
            .is_current
    );
}

/// `UpdateVersionAsync_NonExistingVersion_ShouldNotThrow`
pub async fn update_version_non_existing_version_should_not_throw(s: &dyn PluginVersionStore) {
    assert!(
        s.update_version(&info("plugin-1", "1.0.0", Utc::now(), false))
            .await
            .is_ok()
    );
}

/// `DeleteVersionAsync_ShouldDeleteVersion`
pub async fn delete_version_should_delete_version(s: &dyn PluginVersionStore) {
    let now = Utc::now();
    add(s, "plugin-1", "1.0.0", now, false).await;
    add(s, "plugin-1", "2.0.0", now, false).await;
    s.delete_version("plugin-1", "1.0.0").await.unwrap();
    let v = s.versions("plugin-1").await.unwrap();
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].version, ver("2.0.0"));
}

/// `DeleteVersionAsync_NonExisting_ShouldNotThrow`
pub async fn delete_version_non_existing_should_not_throw(s: &dyn PluginVersionStore) {
    assert!(s.delete_version("plugin-1", "1.0.0").await.is_ok());
}

/// `DeleteAllVersionsAsync_ShouldDeleteAllVersions`
pub async fn delete_all_versions_should_delete_all_versions(s: &dyn PluginVersionStore) {
    let now = Utc::now();
    add(s, "plugin-1", "1.0.0", now, false).await;
    add(s, "plugin-1", "2.0.0", now, false).await;
    add(s, "plugin-2", "1.0.0", now, false).await;
    s.delete_all_versions("plugin-1").await.unwrap();
    assert!(s.versions("plugin-1").await.unwrap().is_empty());
    assert_eq!(s.versions("plugin-2").await.unwrap().len(), 1);
}

/// `SetCurrentVersionAsync_ShouldSetCurrent`
pub async fn set_current_version_should_set_current(s: &dyn PluginVersionStore) {
    let now = Utc::now();
    add(s, "plugin-1", "1.0.0", now - Duration::days(1), true).await;
    add(s, "plugin-1", "2.0.0", now, false).await;
    s.set_current_version("plugin-1", "2.0.0").await.unwrap();
    assert!(
        !s.version("plugin-1", "1.0.0")
            .await
            .unwrap()
            .unwrap()
            .is_current
    );
    assert!(
        s.version("plugin-1", "2.0.0")
            .await
            .unwrap()
            .unwrap()
            .is_current
    );
}

/// `SetCurrentVersionAsync_NonExistingVersion_ShouldUnsetAllCurrents`
pub async fn set_current_version_non_existing_version_should_unset_all_currents(
    s: &dyn PluginVersionStore,
) {
    add(s, "plugin-1", "1.0.0", Utc::now(), true).await;
    s.set_current_version("plugin-1", "nonexistent")
        .await
        .unwrap();
    assert!(
        !s.version("plugin-1", "1.0.0")
            .await
            .unwrap()
            .unwrap()
            .is_current
    );
}

/// `RecordHealthSnapshotAsync_ShouldAddSnapshot`
pub async fn record_health_snapshot_should_add_snapshot(s: &dyn PluginVersionStore) {
    let mut st = snapshot(HealthState::Healthy, Utc::now());
    st.message = Some("All good".into());
    st.response_time = Some(std::time::Duration::from_millis(100));
    st.memory_usage_bytes = 1024 * 1024;
    s.record_health_snapshot("plugin-1", &st).await.unwrap();
    assert_eq!(s.health_snapshot_count("plugin-1").await.unwrap(), 1);
    let h = s
        .health_history("plugin-1", Utc::now() - Duration::days(1), Utc::now())
        .await
        .unwrap();
    assert_eq!(h[0].state, HealthState::Healthy);
    assert_eq!(h[0].message.as_deref(), Some("All good"));
}

/// `RecordHealthSnapshotAsync_ShouldLimitSnapshots` — C# seeds 10 rows through the `DbSet`;
/// here they go through the store (the trimming bound, 1000, is far above 11 either way).
pub async fn record_health_snapshot_should_limit_snapshots(s: &dyn PluginVersionStore) {
    let now = Utc::now();
    for i in 0..10 {
        s.record_health_snapshot(
            "plugin-1",
            &snapshot(HealthState::Healthy, now - Duration::minutes(i)),
        )
        .await
        .unwrap();
    }
    s.record_health_snapshot("plugin-1", &snapshot(HealthState::Healthy, now))
        .await
        .unwrap();
    assert_eq!(s.health_snapshot_count("plugin-1").await.unwrap(), 11);
}

/// `GetHealthHistoryAsync_ShouldReturnFilteredHistory`
pub async fn get_health_history_should_return_filtered_history(s: &dyn PluginVersionStore) {
    let base = Utc::now();
    for i in 0..10 {
        let mut st = snapshot(HealthState::Healthy, base - Duration::hours(i));
        st.response_time = Some(std::time::Duration::from_millis(100 + i.unsigned_abs()));
        s.record_health_snapshot("plugin-1", &st).await.unwrap();
    }
    let (from, to) = (base - Duration::hours(4), base);
    let h = s.health_history("plugin-1", from, to).await.unwrap();
    assert_eq!(h.len(), 5);
    assert!(h.iter().all(|x| x.checked_at >= from && x.checked_at <= to));
}

/// `GetHealthHistoryAsync_EmptyRange_ShouldReturnEmpty`
pub async fn get_health_history_empty_range_should_return_empty(s: &dyn PluginVersionStore) {
    s.record_health_snapshot("plugin-1", &snapshot(HealthState::Healthy, Utc::now()))
        .await
        .unwrap();
    let h = s
        .health_history(
            "plugin-1",
            Utc::now() + Duration::days(1),
            Utc::now() + Duration::days(2),
        )
        .await
        .unwrap();
    assert!(h.is_empty());
}

/// `GetHealthHistoryAsync_ShouldOrderByNewestFirst`
pub async fn get_health_history_should_order_by_newest_first(s: &dyn PluginVersionStore) {
    let base = Utc::now();
    s.record_health_snapshot(
        "plugin-1",
        &snapshot(HealthState::Healthy, base - Duration::hours(2)),
    )
    .await
    .unwrap();
    s.record_health_snapshot("plugin-1", &snapshot(HealthState::Degraded, base))
        .await
        .unwrap();
    let h = s
        .health_history(
            "plugin-1",
            base - Duration::days(1),
            base + Duration::days(1),
        )
        .await
        .unwrap();
    assert_eq!(h.len(), 2);
    assert_eq!(h[0].state, HealthState::Degraded);
}

/// Rust addition: duplicate `(plugin, version)` is rejected (C# unique index).
pub async fn add_version_duplicate_is_rejected(s: &dyn PluginVersionStore) {
    add(s, "plugin-1", "1.0.0", Utc::now(), false).await;
    let e = s
        .add_version(&info("plugin-1", "1.0.0", Utc::now(), false))
        .await
        .unwrap_err();
    assert_eq!(e.code(), arcana_core::ErrorCode::DuplicateEntry);
}

/// Rust addition: rollback target is the version installed just before the current one.
pub async fn rollback_target_is_previous_version(s: &dyn PluginVersionStore) {
    let now = Utc::now();
    assert!(s.rollback_target("plugin-1").await.unwrap().is_none());
    add(s, "plugin-1", "1.0.0", now - Duration::days(2), false).await;
    add(s, "plugin-1", "1.1.0", now - Duration::days(1), false).await;
    add(s, "plugin-1", "2.0.0", now, true).await;
    let t = s.rollback_target("plugin-1").await.unwrap().unwrap();
    assert_eq!(t.version, ver("1.1.0"));
    s.set_current_version("plugin-1", "1.0.0").await.unwrap();
    assert!(s.rollback_target("plugin-1").await.unwrap().is_none());
}

/// Rust addition: metadata upsert / read / list / delete round-trip.
pub async fn metadata_round_trip(s: &dyn PluginVersionStore) {
    use arcana_plugin_api::{PluginState, PluginType};
    use arcana_plugin_host::version_store::PluginMetadataRecord;
    let mut m = PluginMetadataRecord::new("p", "P", "1.0.0", "/plugins/p");
    m.plugin_type = PluginType::View;
    m.state = PluginState::Active;
    m.dependencies = vec!["core@^1.0.0".into()];
    m.metadata.insert("k".into(), "v".into());
    m.installed_at = chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap();
    s.upsert_metadata(&m).await.unwrap();
    m.name = "P2".into();
    s.upsert_metadata(&m).await.unwrap();
    assert_eq!(s.metadata("p").await.unwrap(), Some(m.clone()));
    assert_eq!(s.all_metadata().await.unwrap(), vec![m]);
    s.delete_metadata("p").await.unwrap();
    assert!(s.metadata("p").await.unwrap().is_none());
}

/// Expand one `#[tokio::test]` per suite case; `$make` builds a fresh store (async block).
#[macro_export]
macro_rules! run_suite {
    ($make:expr; $($case:ident),+ $(,)?) => {
        $(
            #[tokio::test]
            async fn $case() {
                let store = $make;
                suite::$case(&*store).await;
            }
        )+
    };
}

#[macro_export]
macro_rules! all_cases {
    ($mac:ident, $make:expr) => {
        $mac!($make;
            get_versions_empty_database_should_return_empty_list,
            get_versions_with_versions_should_return_ordered,
            get_versions_different_plugins_should_only_return_requested_plugin,
            get_version_existing_version_should_return_version,
            get_version_non_existing_version_should_return_null,
            get_version_wrong_plugin_should_return_null,
            get_current_version_no_current_should_return_null,
            get_current_version_with_current_should_return_current,
            add_version_should_add_version,
            add_version_new_current_version_should_unset_previous_current,
            update_version_should_update_version,
            update_version_setting_current_should_unset_others,
            update_version_non_existing_version_should_not_throw,
            delete_version_should_delete_version,
            delete_version_non_existing_should_not_throw,
            delete_all_versions_should_delete_all_versions,
            set_current_version_should_set_current,
            set_current_version_non_existing_version_should_unset_all_currents,
            record_health_snapshot_should_add_snapshot,
            record_health_snapshot_should_limit_snapshots,
            get_health_history_should_return_filtered_history,
            get_health_history_empty_range_should_return_empty,
            get_health_history_should_order_by_newest_first,
            add_version_duplicate_is_rejected,
            rollback_target_is_previous_version,
            metadata_round_trip,
        );
    };
}
