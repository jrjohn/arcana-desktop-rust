//! New (no C# counterpart): field policies, strategy fallbacks, syncable entities and the
//! bookkeeping records.

use arcana_core::{ErrorCode, utc_now};
use arcana_data::SyncOperation;
use arcana_sync::{
    ConflictResolutionStrategy as S, ConflictResolver, FieldDescriptor, FieldPolicy, Mergeable,
    SyncConflictRecord, SyncMeta, SyncMetadata, SyncOperationType, SyncableEntity, VectorClock,
    field,
};
use chrono::TimeDelta;

#[derive(Debug, Clone, PartialEq, Default)]
struct Note {
    id: String,
    title: String,
    draft: String,
    server_rev: i32,
    first: String,
    plain: String,
}

impl Mergeable for Note {
    fn entity_id(&self) -> String {
        self.id.clone()
    }
    fn fields() -> Vec<FieldDescriptor<Self>> {
        vec![
            field!(Note, title),
            field!(Note, draft).policy(FieldPolicy::LocalOnly),
            field!(Note, server_rev).policy(FieldPolicy::RemoteOnly),
            field!(Note, first).policy(FieldPolicy::Merge(S::FirstWriterWins)),
            field!(Note, plain).policy(FieldPolicy::Merge(S::KeepBoth)),
        ]
    }
}

fn note(tag: &str, rev: i32) -> Note {
    Note {
        id: "n".into(),
        title: format!("{tag}-title"),
        draft: format!("{tag}-draft"),
        server_rev: rev,
        first: format!("{tag}-first"),
        plain: format!("{tag}-plain"),
    }
}

fn concurrent() -> (VectorClock, VectorClock) {
    (
        VectorClock::new().increment("a"),
        VectorClock::new().increment("b"),
    )
}

#[test]
fn field_policies_are_honoured() {
    let mut res = ConflictResolver::new("me");
    res.configure::<Note>(S::FieldLevelMerge);
    let (c1, c2) = concurrent();
    let now = utc_now();
    // Remote is newer.
    let r = res.resolve(
        note("L", 1),
        note("R", 2),
        (&c1, &c2),
        (now, now + TimeDelta::seconds(5)),
    );
    let n = r.result;
    assert_eq!(n.title, "R-title", "LWW field takes the newer remote");
    assert_eq!(n.draft, "L-draft", "LocalOnly keeps local");
    assert_eq!(n.server_rev, 2, "RemoteOnly takes remote");
    assert_eq!(n.first, "L-first", "FirstWriterWins keeps the older local");
    assert_eq!(n.plain, "R-plain", "other strategies are LWW per field");
    // Local is newer: RemoteOnly still takes remote, LWW keeps local, FWW takes remote.
    let r = res.resolve(
        note("L", 1),
        note("R", 2),
        (&c1, &c2),
        (now + TimeDelta::seconds(5), now),
    );
    let n = r.result;
    assert_eq!(
        (n.title.as_str(), n.server_rev, n.first.as_str()),
        ("L-title", 2, "R-first")
    );
}

#[test]
fn keep_both_and_custom_without_resolver_fall_back_to_lww() {
    let (c1, c2) = concurrent();
    let now = utc_now();
    for s in [S::KeepBoth, S::Custom] {
        let mut res = ConflictResolver::new("me");
        res.configure::<Note>(s);
        let r = res.resolve(
            note("L", 1),
            note("R", 2),
            (&c1, &c2),
            (now, now + TimeDelta::seconds(1)),
        );
        assert_eq!(r.result.title, "R-title");
        assert_eq!(r.resolution, format!("Resolved using {s}"));
        assert!(r.local_version.is_some() && r.remote_version.is_some());
    }
}

#[test]
fn type_without_fields_keeps_local_on_field_merge() {
    #[derive(Debug, Clone, PartialEq)]
    struct Bare(String);
    impl Mergeable for Bare {
        fn entity_id(&self) -> String {
            self.0.clone()
        }
    }
    let mut res = ConflictResolver::new("me");
    res.configure::<Bare>(S::FieldLevelMerge);
    let (c1, c2) = concurrent();
    let now = utc_now();
    let r = res.resolve(
        Bare("l".into()),
        Bare("r".into()),
        (&c1, &c2),
        (now, now + TimeDelta::seconds(1)),
    );
    assert_eq!(r.result, Bare("l".into()));
    assert_eq!(Bare::fields().len(), 0);
}

#[test]
fn descriptors_strategies_and_resolver_format() {
    let d = field!(Note, title);
    assert_eq!(d.policy, FieldPolicy::default());
    assert!(format!("{d:?}").contains("title"));
    assert_eq!(S::default(), S::LastWriterWins);
    assert_eq!(S::FieldLevelMerge.to_string(), "FieldLevelMerge");
    let res = ConflictResolver::new("node-x");
    assert!(format!("{res:?}").contains("node-x"));
}

#[derive(Debug, Clone, Default)]
struct Doc {
    id: String,
    meta: SyncMeta,
}

impl SyncableEntity for Doc {
    fn sync_id(&self) -> String {
        self.id.clone()
    }
    fn meta(&self) -> &SyncMeta {
        &self.meta
    }
    fn meta_mut(&mut self) -> &mut SyncMeta {
        &mut self.meta
    }
}

#[test]
fn syncable_entity_clock_helpers() {
    let mut d = Doc {
        id: "d1".into(),
        ..Doc::default()
    };
    assert_eq!(d.sync_id(), "d1");
    assert_eq!(d.vector_clock().unwrap(), VectorClock::new());
    d.meta_mut().vector_clock_json = Some(String::new());
    assert_eq!(d.vector_clock().unwrap(), VectorClock::new());
    let before = d.meta().modified_at;
    d.increment_clock("n1").unwrap();
    d.increment_clock("n1").unwrap();
    assert_eq!(d.vector_clock().unwrap().get_value("n1"), 2);
    assert_eq!(d.meta().modified_by_node_id.as_deref(), Some("n1"));
    assert!(d.meta().modified_at >= before);
    assert!(!d.meta().has_conflict && d.meta().conflicting_version_json.is_none());
    d.meta_mut().vector_clock_json = Some("not json".into());
    assert_eq!(
        d.increment_clock("n1").unwrap_err().code(),
        ErrorCode::DataCorruption
    );
}

#[test]
fn records_and_operation_types() {
    let c = VectorClock::new().increment("a");
    let mut rec = SyncConflictRecord::new("Doc", "d1", ("{}".into(), &c), ("{}".into(), &c));
    assert!(!rec.is_resolved && rec.resolved_at.is_none());
    assert_eq!(rec.local_clock_json, r#"{"a":1}"#);
    rec.resolve("n1", "LastWriterWins");
    assert!(rec.is_resolved);
    assert_eq!(rec.resolved_by_node_id.as_deref(), Some("n1"));
    assert_eq!(rec.resolution_strategy.as_deref(), Some("LastWriterWins"));
    assert_eq!(rec.id, 0);

    let meta = SyncMetadata {
        entity_type: "Doc".into(),
        entity_id: "d1".into(),
        vector_clock_json: c.serialize(),
        modified_at: utc_now(),
        modified_by_node_id: "n1".into(),
        operation: SyncOperationType::default(),
        is_synced: false,
        synced_at: None,
        retry_count: 0,
        last_error: None,
    };
    assert_eq!(meta.operation, SyncOperationType::Create);
    for (t, d) in [
        (SyncOperationType::Create, SyncOperation::Create),
        (SyncOperationType::Update, SyncOperation::Update),
        (SyncOperationType::Delete, SyncOperation::Delete),
    ] {
        assert_eq!(SyncOperation::from(t), d);
        assert_eq!(SyncOperationType::from(d), t);
    }
}
