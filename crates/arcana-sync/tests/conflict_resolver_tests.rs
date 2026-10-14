//! C# `ConflictResolverTests` (17) and `SyncConflictTests` (2).

use std::sync::{Arc, Mutex};

use arcana_core::{Timestamp, utc_now};
use arcana_sync::{
    CausalRelation, ConflictResolutionStrategy as S, ConflictResolver, FieldDescriptor, Mergeable,
    SyncConflict, VectorClock, field,
};
use chrono::TimeDelta;

#[derive(Debug, Clone, PartialEq, Default)]
struct TestEntity {
    id: String,
    name: String,
    value: i32,
    description: String,
}

impl TestEntity {
    fn named(name: &str) -> Self {
        Self {
            id: format!("{name}-id"),
            name: name.into(),
            ..Self::default()
        }
    }
}

impl Mergeable for TestEntity {
    fn entity_id(&self) -> String {
        self.id.clone()
    }
    fn fields() -> Vec<FieldDescriptor<Self>> {
        vec![
            field!(TestEntity, id),
            field!(TestEntity, name),
            field!(TestEntity, value),
            field!(TestEntity, description),
        ]
    }
}

#[derive(Debug, Clone, PartialEq)]
struct EntityWithFieldTimestamps {
    id: String,
    title: String,
    title_modified_at: Timestamp,
    content: String,
    content_modified_at: Timestamp,
}

impl Mergeable for EntityWithFieldTimestamps {
    fn entity_id(&self) -> String {
        self.id.clone()
    }
    fn fields() -> Vec<FieldDescriptor<Self>> {
        vec![
            field!(EntityWithFieldTimestamps, title).modified_at(|e| e.title_modified_at),
            field!(EntityWithFieldTimestamps, title_modified_at),
            field!(EntityWithFieldTimestamps, content).modified_at(|e| e.content_modified_at),
            field!(EntityWithFieldTimestamps, content_modified_at),
        ]
    }
}

fn resolver() -> ConflictResolver {
    ConflictResolver::new("testNode")
}

fn inc(n: &str) -> VectorClock {
    VectorClock::new().increment(n)
}

fn secs(n: i64) -> TimeDelta {
    TimeDelta::seconds(n)
}

// Resolve_LocalHappenedAfter_ShouldReturnLocal
#[test]
fn resolve_local_happened_after_should_return_local() {
    let c1 = inc("node1");
    let c2 = c1.increment("node1");
    let local = TestEntity {
        value: 2,
        ..TestEntity::named("Local")
    };
    let now = utc_now();
    let r = resolver().resolve(
        local.clone(),
        TestEntity::named("Remote"),
        (&c2, &c1),
        (now, now - secs(1)),
    );
    assert_eq!(r.result, local);
    assert!(!r.had_conflict);
    assert_eq!(r.resolution, "Local version is newer");
    assert_eq!(r.merged_clock, c2);
}

// Resolve_LocalHappenedBefore_ShouldReturnRemote
#[test]
fn resolve_local_happened_before_should_return_remote() {
    let c1 = inc("node1");
    let c2 = c1.increment("node1");
    let remote = TestEntity::named("Remote");
    let now = utc_now();
    let r = resolver().resolve(
        TestEntity::named("Local"),
        remote.clone(),
        (&c1, &c2),
        (now - secs(1), now),
    );
    assert_eq!(r.result, remote);
    assert!(!r.had_conflict);
    assert_eq!(r.resolution, "Remote version is newer");
}

// Resolve_EqualClocks_ShouldReturnLocal
#[test]
fn resolve_equal_clocks_should_return_local() {
    let c = inc("node1");
    let local = TestEntity::named("Same");
    let now = utc_now();
    let r = resolver().resolve(local.clone(), local.clone(), (&c, &c), (now, now));
    assert_eq!(r.result, local);
    assert!(!r.had_conflict);
    assert_eq!(r.resolution, "Versions are identical");
}

// Resolve_LWW_ConcurrentRemoteNewer_ShouldReturnRemote
#[test]
fn resolve_lww_concurrent_remote_newer_should_return_remote() {
    let mut res = resolver();
    res.configure::<TestEntity>(S::LastWriterWins);
    let now = utc_now();
    let r = res.resolve(
        TestEntity::named("Local"),
        TestEntity::named("Remote"),
        (&inc("node1"), &inc("node2")),
        (now - secs(10), now),
    );
    assert_eq!(r.result.name, "Remote");
    assert!(r.had_conflict);
    assert!(r.resolution.contains("LastWriterWins"));
}

// Resolve_LWW_ConcurrentLocalNewer_ShouldReturnLocal
#[test]
fn resolve_lww_concurrent_local_newer_should_return_local() {
    let mut res = resolver();
    res.configure::<TestEntity>(S::LastWriterWins);
    let now = utc_now();
    let r = res.resolve(
        TestEntity::named("Local"),
        TestEntity::named("Remote"),
        (&inc("node1"), &inc("node2")),
        (now, now - secs(10)),
    );
    assert_eq!(r.result.name, "Local");
    assert!(r.had_conflict);
}

// Resolve_LWW_SameTimestamp_ShouldUseDeterministicTiebreaker
// C# only asserts the conflict; the tie-break (greater entity id wins) is asserted here too.
#[test]
fn resolve_lww_same_timestamp_should_use_deterministic_tiebreaker() {
    let mut res = resolver();
    res.configure::<TestEntity>(S::LastWriterWins);
    let t = utc_now();
    let local = TestEntity {
        id: "A".into(),
        ..TestEntity::named("Local")
    };
    let remote = TestEntity {
        id: "Z".into(),
        ..TestEntity::named("Remote")
    };
    let clocks = (&inc("node1"), &inc("node2"));
    let r = res.resolve(local.clone(), remote.clone(), clocks, (t, t));
    assert!(r.had_conflict);
    assert_eq!(r.result.name, "Remote");
    let r = res.resolve(remote, local, clocks, (t, t));
    assert_eq!(r.result.name, "Remote");
}

// Resolve_FWW_ConcurrentRemoteOlder_ShouldReturnRemote
#[test]
fn resolve_fww_concurrent_remote_older_should_return_remote() {
    let mut res = resolver();
    res.configure::<TestEntity>(S::FirstWriterWins);
    let now = utc_now();
    let r = res.resolve(
        TestEntity::named("Local"),
        TestEntity::named("Remote"),
        (&inc("node1"), &inc("node2")),
        (now, now - secs(10)),
    );
    assert_eq!(r.result.name, "Remote");
    assert!(r.had_conflict);
    assert!(r.resolution.contains("FirstWriterWins"));
}

// Resolve_FWW_ConcurrentLocalOlder_ShouldReturnLocal
#[test]
fn resolve_fww_concurrent_local_older_should_return_local() {
    let mut res = resolver();
    res.configure::<TestEntity>(S::FirstWriterWins);
    let now = utc_now();
    let r = res.resolve(
        TestEntity::named("Local"),
        TestEntity::named("Remote"),
        (&inc("node1"), &inc("node2")),
        (now - secs(10), now),
    );
    assert_eq!(r.result.name, "Local");
    assert!(r.had_conflict);
    // Same timestamp: smaller id wins.
    let t = utc_now();
    let r = res.resolve(
        TestEntity::named("B"),
        TestEntity::named("A"),
        (&inc("node1"), &inc("node2")),
        (t, t),
    );
    assert_eq!(r.result.name, "A");
}

// Resolve_FieldLevel_ShouldMergeFieldByField
#[test]
fn resolve_field_level_should_merge_field_by_field() {
    let mut res = resolver();
    res.configure::<TestEntity>(S::FieldLevelMerge);
    let local = TestEntity {
        id: "entity1".into(),
        name: "Local Name".into(),
        value: 100,
        description: "Local Description".into(),
    };
    let remote = TestEntity {
        id: "entity1".into(),
        name: "Remote Name".into(),
        value: 200,
        description: "Remote Description".into(),
    };
    let now = utc_now();
    let r = res.resolve(
        local,
        remote,
        (&inc("node1"), &inc("node2")),
        (now, now - secs(10)),
    );
    assert!(r.had_conflict);
    assert_eq!(
        (r.result.name.as_str(), r.result.value),
        ("Local Name", 100)
    );
    assert!(r.resolution.contains("FieldLevelMerge"));
}

// Resolve_FieldLevel_WithFieldTimestamps_ShouldUseFieldTimestamps
#[test]
fn resolve_field_level_with_field_timestamps_should_use_field_timestamps() {
    let mut res = resolver();
    res.configure::<EntityWithFieldTimestamps>(S::FieldLevelMerge);
    let base = utc_now();
    let local = EntityWithFieldTimestamps {
        id: "entity1".into(),
        title: "Local Title".into(),
        title_modified_at: base + secs(10),
        content: "Local Content".into(),
        content_modified_at: base - secs(10),
    };
    let remote = EntityWithFieldTimestamps {
        id: "entity1".into(),
        title: "Remote Title".into(),
        title_modified_at: base - secs(10),
        content: "Remote Content".into(),
        content_modified_at: base + secs(10),
    };
    let r = res.resolve(local, remote, (&inc("node1"), &inc("node2")), (base, base));
    assert!(r.had_conflict);
    assert_eq!(r.result.title, "Local Title");
    assert_eq!(r.result.content, "Remote Content");
}

// Resolve_FieldLevel_SameFieldValues_ShouldUseEither
#[test]
fn resolve_field_level_same_field_values_should_use_either() {
    let mut res = resolver();
    res.configure::<TestEntity>(S::FieldLevelMerge);
    let e = TestEntity {
        id: "same".into(),
        name: "Same Name".into(),
        value: 42,
        ..TestEntity::default()
    };
    let t = utc_now();
    let r = res.resolve(e.clone(), e, (&inc("node1"), &inc("node2")), (t, t));
    assert_eq!((r.result.name.as_str(), r.result.value), ("Same Name", 42));
}

// Resolve_CustomResolver_ShouldUseProvidedFunction
#[test]
fn resolve_custom_resolver_should_use_provided_function() {
    let mut res = resolver();
    res.configure_custom::<TestEntity>(|c| TestEntity {
        id: c.entity_id.clone(),
        name: format!("{} + {}", c.local_version.name, c.remote_version.name),
        value: c.local_version.value + c.remote_version.value,
        ..TestEntity::default()
    });
    let t = utc_now();
    let r = res.resolve(
        TestEntity {
            id: "e1".into(),
            name: "A".into(),
            value: 10,
            ..TestEntity::default()
        },
        TestEntity {
            id: "e1".into(),
            name: "B".into(),
            value: 20,
            ..TestEntity::default()
        },
        (&inc("node1"), &inc("node2")),
        (t, t),
    );
    assert!(r.had_conflict);
    assert_eq!((r.result.name.as_str(), r.result.value), ("A + B", 30));
    assert!(r.resolution.contains("Custom"));
    assert_eq!(r.result.id, "e1");
}

// Resolve_CustomResolver_ShouldReceiveConflictDetails
#[test]
fn resolve_custom_resolver_should_receive_conflict_details() {
    let captured: Arc<Mutex<Option<SyncConflict<TestEntity>>>> = Arc::default();
    let sink = captured.clone();
    let mut res = resolver();
    res.configure_custom::<TestEntity>(move |c| {
        *sink.lock().unwrap() = Some(c.clone());
        c.local_version.clone()
    });
    let (c1, c2) = (inc("node1"), inc("node2"));
    let local = TestEntity {
        id: "test-id".into(),
        ..TestEntity::named("Local")
    };
    let remote = TestEntity {
        id: "test-id".into(),
        ..TestEntity::named("Remote")
    };
    let t = utc_now();
    let _ = res.resolve(local.clone(), remote.clone(), (&c1, &c2), (t, t));
    let c = captured.lock().unwrap().clone().unwrap();
    assert_eq!(c.local_version, local);
    assert_eq!(c.remote_version, remote);
    assert_eq!(c.local_clock, c1);
    assert_eq!(c.remote_clock, c2);
    assert_eq!(c.relation, CausalRelation::Concurrent);
    assert_eq!(c.entity_id, "test-id");
}

// Resolve_UnconfiguredType_ShouldDefaultToLWW
#[test]
fn resolve_unconfigured_type_should_default_to_lww() {
    let res = resolver();
    assert_eq!(res.strategy_for::<TestEntity>(), S::LastWriterWins);
    let now = utc_now();
    let r = res.resolve(
        TestEntity::named("Local"),
        TestEntity::named("Remote"),
        (&inc("node1"), &inc("node2")),
        (now - secs(10), now),
    );
    assert_eq!(r.result.name, "Remote");
    assert!(r.had_conflict);
}

// Resolve_Conflict_ShouldReturnMergedClock
#[test]
fn resolve_conflict_should_return_merged_clock() {
    let c1 = inc("node1").increment("node1");
    let c2 = inc("node2").increment("node2").increment("node2");
    let t = utc_now();
    let res = resolver();
    let r = res.resolve(
        TestEntity::named("Local"),
        TestEntity::named("Remote"),
        (&c1, &c2),
        (t, t),
    );
    assert_eq!(r.merged_clock.get_value("node1"), 2);
    assert_eq!(r.merged_clock.get_value("node2"), 3);
    assert_eq!(r.merged_clock.get_value(res.node_id()), 1);
}

// Result_WhenConflict_ShouldIncludeBothVersions
#[test]
fn result_when_conflict_should_include_both_versions() {
    let (local, remote) = (TestEntity::named("Local"), TestEntity::named("Remote"));
    let t = utc_now();
    let r = resolver().resolve(
        local.clone(),
        remote.clone(),
        (&inc("node1"), &inc("node2")),
        (t, t),
    );
    assert!(r.had_conflict);
    assert_eq!(r.local_version, Some(local));
    assert_eq!(r.remote_version, Some(remote));
}

// Result_WhenNoConflict_ShouldNotIncludeVersions
#[test]
fn result_when_no_conflict_should_not_include_versions() {
    let c1 = inc("node1");
    let c2 = c1.increment("node1");
    let t = utc_now();
    let r = resolver().resolve(
        TestEntity::named("Local"),
        TestEntity::named("Remote"),
        (&c2, &c1),
        (t, t),
    );
    assert!(!r.had_conflict);
    assert_eq!((r.local_version, r.remote_version), (None, None));
}

// SyncConflict_Resolve_ShouldSetResolvedVersion
#[test]
fn sync_conflict_resolve_should_set_resolved_version() {
    let mut c = SyncConflict::new(
        "test",
        (TestEntity::named("Local"), VectorClock::new()),
        (TestEntity::named("Remote"), VectorClock::new()),
        CausalRelation::Concurrent,
    );
    assert!(!c.is_resolved());
    assert_eq!(c.resolved_version(), None);
    let resolved = TestEntity::named("Resolved");
    c.resolve(resolved.clone());
    assert!(c.is_resolved());
    assert_eq!(c.resolved_version(), Some(&resolved));
}

// SyncConflict_DetectedAt_ShouldBeSetAutomatically
#[test]
fn sync_conflict_detected_at_should_be_set_automatically() {
    let before = utc_now();
    let c = SyncConflict::new(
        "test",
        (TestEntity::default(), VectorClock::new()),
        (TestEntity::default(), VectorClock::new()),
        CausalRelation::Concurrent,
    );
    let after = utc_now();
    assert!(c.detected_at >= before && c.detected_at <= after);
}
