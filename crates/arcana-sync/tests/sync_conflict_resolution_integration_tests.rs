//! C# `SyncConflictResolutionIntegrationTests` (13).

use arcana_core::{Timestamp, utc_now};
use arcana_sync::{
    CausalRelation, ConflictResolutionStrategy as S, ConflictResolver, FieldDescriptor, LwwMap,
    Mergeable, MvRegister, VectorClock, field,
};
use chrono::TimeDelta;

#[derive(Debug, Clone, PartialEq, Default)]
struct Document {
    id: String,
    title: String,
    content: String,
}

impl Mergeable for Document {
    fn entity_id(&self) -> String {
        self.id.clone()
    }
}

#[derive(Debug, Clone, PartialEq)]
struct DocumentWithTimestamps {
    id: String,
    title: String,
    title_modified_at: Timestamp,
    content: String,
    content_modified_at: Timestamp,
}

impl Mergeable for DocumentWithTimestamps {
    fn entity_id(&self) -> String {
        self.id.clone()
    }
    fn fields() -> Vec<FieldDescriptor<Self>> {
        vec![
            field!(DocumentWithTimestamps, title).modified_at(|d| d.title_modified_at),
            field!(DocumentWithTimestamps, content).modified_at(|d| d.content_modified_at),
        ]
    }
}

fn doc(title: &str, content: &str) -> Document {
    Document {
        id: "doc-1".into(),
        title: title.into(),
        content: content.into(),
    }
}

fn secs(n: i64) -> TimeDelta {
    TimeDelta::seconds(n)
}

// TwoClients_SequentialEdits_ShouldMergeCorrectly
#[test]
fn two_clients_sequential_edits_should_merge_correctly() {
    let clock_a = VectorClock::new().increment("client-A");
    let doc_a = doc("Original", "Initial content");
    let clock_b = clock_a.increment("client-B");
    let doc_b = Document {
        content: "B's edit".into(),
        ..doc_a.clone()
    };
    assert_eq!(clock_b.compare_to(&clock_a), CausalRelation::HappenedAfter);
    let t = utc_now();
    let r = ConflictResolver::new("server").resolve(doc_a, doc_b, (&clock_a, &clock_b), (t, t));
    assert_eq!(r.result.content, "B's edit");
}

// TwoClients_ConcurrentEdits_ShouldDetectConflict
#[test]
fn two_clients_concurrent_edits_should_detect_conflict() {
    let init = VectorClock::new();
    let a = init.increment("client-A");
    let b = init.increment("client-B");
    assert_eq!(a.compare_to(&b), CausalRelation::Concurrent);
    assert_eq!(b.compare_to(&a), CausalRelation::Concurrent);
}

// TwoClients_ConcurrentEdits_WithLWWResolution
#[test]
fn two_clients_concurrent_edits_with_lww_resolution() {
    let mut res = ConflictResolver::new("server");
    res.configure::<Document>(S::LastWriterWins);
    let now = utc_now();
    let r = res.resolve(
        doc("A's Title", "A's content"),
        doc("B's Title", "B's content"),
        (
            &VectorClock::new().increment("client-A"),
            &VectorClock::new().increment("client-B"),
        ),
        (now - secs(10), now),
    );
    assert!(r.had_conflict);
    assert_eq!(r.result.title, "B's Title");
    assert_eq!(r.result.content, "B's content");
}

// TwoClients_ConcurrentEdits_WithFieldLevelMerge
#[test]
fn two_clients_concurrent_edits_with_field_level_merge() {
    let mut res = ConflictResolver::new("server");
    res.configure::<DocumentWithTimestamps>(S::FieldLevelMerge);
    let base = utc_now();
    let a = DocumentWithTimestamps {
        id: "doc-1".into(),
        title: "A's Title".into(),
        title_modified_at: base + secs(10),
        content: "A's content".into(),
        content_modified_at: base - secs(10),
    };
    let b = DocumentWithTimestamps {
        id: "doc-1".into(),
        title: "B's Title".into(),
        title_modified_at: base - secs(10),
        content: "B's content".into(),
        content_modified_at: base + secs(10),
    };
    let r = res.resolve(
        a,
        b,
        (
            &VectorClock::new().increment("client-A"),
            &VectorClock::new().increment("client-B"),
        ),
        (base, base),
    );
    assert!(r.had_conflict);
    assert_eq!(r.result.title, "A's Title");
    assert_eq!(r.result.content, "B's content");
}

// ThreeClients_ConcurrentEdits_ShouldCreateThreeWayConflict
#[test]
fn three_clients_concurrent_edits_should_create_three_way_conflict() {
    let base = VectorClock::new();
    let (a, b, c) = (
        base.increment("client-A"),
        base.increment("client-B"),
        base.increment("client-C"),
    );
    assert_eq!(a.compare_to(&b), CausalRelation::Concurrent);
    assert_eq!(a.compare_to(&c), CausalRelation::Concurrent);
    assert_eq!(b.compare_to(&c), CausalRelation::Concurrent);
}

// ThreeClients_WithMVRegister_ShouldPreserveAllVersions
#[test]
fn three_clients_with_mv_register_should_preserve_all_versions() {
    let mut reg = MvRegister::new();
    let ca = VectorClock::new().increment("client-A");
    let cb = VectorClock::new().increment("client-B");
    let cc = VectorClock::new().increment("client-C");
    reg.set("Version A", ca.clone());
    reg.set("Version B", cb.clone());
    reg.set("Version C", cc.clone());
    assert!(reg.has_conflict());
    let mut vals: Vec<_> = reg.values().iter().map(|(v, _)| *v).collect();
    vals.sort_unstable();
    assert_eq!(vals, ["Version A", "Version B", "Version C"]);
    reg.resolve(
        "Merged by user",
        ca.merge(&cb).merge(&cc).increment("resolver"),
    );
    assert!(!reg.has_conflict());
    assert_eq!(*reg.single_value().unwrap(), "Merged by user");
}

// OfflineClient_MultipleEdits_ThenSync
#[test]
fn offline_client_multiple_edits_then_sync() {
    let base = VectorClock::new().increment("online");
    let online = base.increment("online").increment("online");
    let offline = base
        .increment("offline")
        .increment("offline")
        .increment("offline");
    assert_eq!(online.compare_to(&offline), CausalRelation::Concurrent);
    let merged = online.merge(&offline);
    assert_eq!(merged.compare_to(&online), CausalRelation::HappenedAfter);
    assert_eq!(merged.compare_to(&offline), CausalRelation::HappenedAfter);
}

// OfflineClient_SyncsWithServer_ThenContinuesEditing
#[test]
fn offline_client_syncs_with_server_then_continues_editing() {
    let server = VectorClock::new().increment("server");
    let client = server.increment("client");
    let server = server.increment("server");
    assert_eq!(client.compare_to(&server), CausalRelation::Concurrent);
    let after = server.merge(&client).increment("client");
    assert_eq!(after.compare_to(&server), CausalRelation::HappenedAfter);
    assert_eq!(after.compare_to(&client), CausalRelation::HappenedAfter);
}

// LWWMap_MultipleFieldsEditedByConcurrentClients
#[test]
fn lww_map_multiple_fields_edited_by_concurrent_clients() {
    let base = utc_now();
    let mut a = LwwMap::new();
    a.set("title", &"A's Title", base + secs(10), "client-A");
    a.set("author", &"A's Author", base + secs(5), "client-A");
    a.set("content", &"A's Content", base - secs(10), "client-A");
    let mut b = LwwMap::new();
    b.set("title", &"B's Title", base - secs(10), "client-B");
    b.set("author", &"B's Author", base + secs(20), "client-B");
    b.set("content", &"B's Content", base + secs(10), "client-B");
    let m = a.merge(&b);
    assert_eq!(m.get::<String>("title").as_deref(), Some("A's Title"));
    assert_eq!(m.get::<String>("author").as_deref(), Some("B's Author"));
    assert_eq!(m.get::<String>("content").as_deref(), Some("B's Content"));
}

// LWWMap_ContinuousMerging_ShouldConverge
#[test]
fn lww_map_continuous_merging_should_converge() {
    let base = utc_now();
    let (mut m1, mut m2, mut m3) = (LwwMap::new(), LwwMap::new(), LwwMap::new());
    m1.set("field", &"value1", base, "client1");
    m2.set("field", &"value2", base + secs(1), "client2");
    m3.set("field", &"value3", base + secs(2), "client3");
    let merged = m1.merge(&m2).merge(&m3);
    assert_eq!(merged.get::<String>("field").as_deref(), Some("value3"));
    let alt = m2.merge(&m3).merge(&m1);
    assert_eq!(alt.get::<String>("field").as_deref(), Some("value3"));
}

// CustomResolver_MergeStrategies
#[test]
fn custom_resolver_merge_strategies() {
    let mut res = ConflictResolver::new("resolver");
    res.configure_custom::<Document>(|c| {
        let title = if c.local_version.title.len() >= c.remote_version.title.len() {
            c.local_version.title.clone()
        } else {
            c.remote_version.title.clone()
        };
        Document {
            id: c.entity_id.clone(),
            title,
            content: format!(
                "{}\n---\n{}",
                c.local_version.content, c.remote_version.content
            ),
        }
    });
    let t = utc_now();
    let r = res.resolve(
        doc("Short", "Content A"),
        doc("Much Longer Title", "Content B"),
        (
            &VectorClock::new().increment("A"),
            &VectorClock::new().increment("B"),
        ),
        (t, t),
    );
    assert!(r.had_conflict);
    assert_eq!(r.result.title, "Much Longer Title");
    assert!(r.result.content.contains("Content A") && r.result.content.contains("Content B"));
}

// VectorClock_ChainOfCausality
#[test]
fn vector_clock_chain_of_causality() {
    let a = VectorClock::new().increment("A");
    let b = a.merge(&VectorClock::new()).increment("B");
    let c = b.merge(&VectorClock::new()).increment("C");
    assert_eq!(a.compare_to(&b), CausalRelation::HappenedBefore);
    assert_eq!(b.compare_to(&c), CausalRelation::HappenedBefore);
    assert_eq!(a.compare_to(&c), CausalRelation::HappenedBefore);
    assert_eq!(c.compare_to(&a), CausalRelation::HappenedAfter);
}

// VectorClock_PartialOrder
#[test]
fn vector_clock_partial_order() {
    let a1 = VectorClock::new().increment("A");
    let b1 = VectorClock::new().increment("B");
    assert_eq!(a1.compare_to(&b1), CausalRelation::Concurrent);
    let c1 = a1.merge(&VectorClock::new()).increment("C");
    assert_eq!(c1.compare_to(&a1), CausalRelation::HappenedAfter);
    assert_eq!(c1.compare_to(&b1), CausalRelation::Concurrent);
    let d1 = b1.merge(&VectorClock::new()).increment("D");
    assert_eq!(d1.compare_to(&b1), CausalRelation::HappenedAfter);
    assert_eq!(d1.compare_to(&a1), CausalRelation::Concurrent);
    assert_eq!(d1.compare_to(&c1), CausalRelation::Concurrent);
}
