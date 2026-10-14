//! C# `VectorClockTests` (40), one Rust test per C# test (named in each comment).

use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use arcana_sync::{CausalRelation, VectorClock};

fn vc(entries: &[(&str, i64)]) -> VectorClock {
    VectorClock::from_entries(entries.iter().map(|(k, v)| (*k, *v)))
}

fn hash(c: &VectorClock) -> u64 {
    let mut h = DefaultHasher::new();
    c.hash(&mut h);
    h.finish()
}

// Constructor_Default_ShouldCreateEmptyClock
#[test]
fn constructor_default_should_create_empty_clock() {
    assert!(VectorClock::new().entries().is_empty());
}

// Constructor_WithDictionary_ShouldCopyValues
#[test]
fn constructor_with_dictionary_should_copy_values() {
    let c = vc(&[("node1", 5), ("node2", 3)]);
    assert_eq!(c.entries().len(), 2);
    assert_eq!(c.get_value("node1"), 5);
    assert_eq!(c.get_value("node2"), 3);
}

// Constructor_WithDictionary_ShouldNotShareReference
#[test]
fn constructor_with_dictionary_should_not_share_reference() {
    let mut initial = std::collections::BTreeMap::from([("node1".to_owned(), 5)]);
    let c = VectorClock::from_entries(initial.clone());
    initial.insert("node1".into(), 10);
    assert_eq!(c.get_value("node1"), 5);
}

// Increment_NewNode_ShouldSetValueToOne
#[test]
fn increment_new_node_should_set_value_to_one() {
    let c = VectorClock::new();
    let n = c.increment("node1");
    assert_eq!(n.get_value("node1"), 1);
    assert_eq!(c.get_value("node1"), 0);
}

// Increment_ExistingNode_ShouldIncrementValue
#[test]
fn increment_existing_node_should_increment_value() {
    let c = vc(&[("node1", 5)]);
    let n = c.increment("node1");
    assert_eq!(n.get_value("node1"), 6);
    assert_eq!(c.get_value("node1"), 5);
}

// Increment_ShouldPreserveOtherNodes
#[test]
fn increment_should_preserve_other_nodes() {
    let n = vc(&[("node1", 5), ("node2", 3)]).increment("node1");
    assert_eq!(n.get_value("node1"), 6);
    assert_eq!(n.get_value("node2"), 3);
}

// Increment_ShouldReturnNewInstance
#[test]
fn increment_should_return_new_instance() {
    let c = VectorClock::new();
    let n = c.increment("node1");
    assert_ne!(c, n);
    assert!(c.entries().is_empty());
}

// GetValue_ExistingNode_ShouldReturnValue
#[test]
fn get_value_existing_node_should_return_value() {
    assert_eq!(vc(&[("node1", 42)]).get_value("node1"), 42);
}

// GetValue_NonExistingNode_ShouldReturnZero
#[test]
fn get_value_non_existing_node_should_return_zero() {
    assert_eq!(VectorClock::new().get_value("nonexistent"), 0);
}

// Merge_EmptyClocks_ShouldReturnEmptyClock
#[test]
fn merge_empty_clocks_should_return_empty_clock() {
    assert!(
        VectorClock::new()
            .merge(&VectorClock::new())
            .entries()
            .is_empty()
    );
}

// Merge_OneEmptyClock_ShouldReturnOtherClock
#[test]
fn merge_one_empty_clock_should_return_other_clock() {
    assert_eq!(
        vc(&[("node1", 5)])
            .merge(&VectorClock::new())
            .get_value("node1"),
        5
    );
}

// Merge_DisjointNodes_ShouldIncludeAll
#[test]
fn merge_disjoint_nodes_should_include_all() {
    let m = vc(&[("node1", 5)]).merge(&vc(&[("node2", 3)]));
    assert_eq!((m.get_value("node1"), m.get_value("node2")), (5, 3));
}

// Merge_OverlappingNodes_ShouldTakeMaximum
#[test]
fn merge_overlapping_nodes_should_take_maximum() {
    let m = vc(&[("node1", 5), ("node2", 3)]).merge(&vc(&[("node1", 3), ("node2", 7)]));
    assert_eq!((m.get_value("node1"), m.get_value("node2")), (5, 7));
}

// Merge_ShouldBeCommutative
#[test]
fn merge_should_be_commutative() {
    let a = vc(&[("node1", 5)]);
    let b = vc(&[("node2", 3)]);
    assert_eq!(a.merge(&b), b.merge(&a));
}

// Merge_ShouldBeAssociative
#[test]
fn merge_should_be_associative() {
    let (a, b, c) = (
        vc(&[("node1", 1)]),
        vc(&[("node2", 2)]),
        vc(&[("node3", 3)]),
    );
    assert_eq!(a.merge(&b).merge(&c), a.merge(&b.merge(&c)));
}

// Merge_ShouldBeIdempotent
#[test]
fn merge_should_be_idempotent() {
    let a = vc(&[("node1", 5)]);
    assert_eq!(a.merge(&a), a);
}

// CompareTo_EqualClocks_ShouldReturnEqual
#[test]
fn compare_to_equal_clocks_should_return_equal() {
    assert_eq!(
        vc(&[("node1", 5)]).compare_to(&vc(&[("node1", 5)])),
        CausalRelation::Equal
    );
}

// CompareTo_EmptyClocks_ShouldReturnEqual
#[test]
fn compare_to_empty_clocks_should_return_equal() {
    assert_eq!(
        VectorClock::new().compare_to(&VectorClock::new()),
        CausalRelation::Equal
    );
}

// CompareTo_ThisHappenedAfter_ShouldReturnHappenedAfter
#[test]
fn compare_to_this_happened_after_should_return_happened_after() {
    assert_eq!(
        vc(&[("node1", 5), ("node2", 3)]).compare_to(&vc(&[("node1", 3), ("node2", 2)])),
        CausalRelation::HappenedAfter
    );
}

// CompareTo_ThisHappenedBefore_ShouldReturnHappenedBefore
#[test]
fn compare_to_this_happened_before_should_return_happened_before() {
    assert_eq!(
        vc(&[("node1", 3), ("node2", 2)]).compare_to(&vc(&[("node1", 5), ("node2", 3)])),
        CausalRelation::HappenedBefore
    );
}

// CompareTo_Concurrent_ShouldReturnConcurrent
#[test]
fn compare_to_concurrent_should_return_concurrent() {
    assert_eq!(
        vc(&[("node1", 5), ("node2", 2)]).compare_to(&vc(&[("node1", 3), ("node2", 4)])),
        CausalRelation::Concurrent
    );
}

// CompareTo_DifferentNodeSets_ShouldHandleCorrectly
#[test]
fn compare_to_different_node_sets_should_handle_correctly() {
    assert_eq!(
        vc(&[("node1", 5), ("node2", 3)]).compare_to(&vc(&[("node1", 5)])),
        CausalRelation::HappenedAfter
    );
}

// IComparable_CompareTo_Null_ShouldReturnPositive
// Rust has no null reference: `Option<&VectorClock>` makes "compare with nothing" a
// `None` that the caller handles; the comparable view is `PartialOrd`.
#[test]
fn icomparable_compare_to_null_should_return_positive() {
    let c = VectorClock::new();
    assert_eq!(Some(&c).partial_cmp(&None), Some(Ordering::Greater));
}

// IComparable_CompareTo_ShouldMapHappenedAfterToPositive
#[test]
fn icomparable_compare_to_should_map_happened_after_to_positive() {
    assert_eq!(
        vc(&[("node1", 5)]).partial_cmp(&vc(&[("node1", 3)])),
        Some(Ordering::Greater)
    );
}

// IComparable_CompareTo_ShouldMapHappenedBeforeToNegative
#[test]
fn icomparable_compare_to_should_map_happened_before_to_negative() {
    assert_eq!(
        vc(&[("node1", 3)]).partial_cmp(&vc(&[("node1", 5)])),
        Some(Ordering::Less)
    );
}

// IComparable_CompareTo_ShouldMapConcurrentToZero
// Deviation: C# maps Concurrent to 0; as a partial order it is `None` (incomparable).
#[test]
fn icomparable_compare_to_should_map_concurrent_to_zero() {
    let a = vc(&[("node1", 5), ("node2", 2)]);
    let b = vc(&[("node1", 3), ("node2", 4)]);
    assert_eq!(a.partial_cmp(&b), None);
    assert_eq!(a.partial_cmp(&a.clone()), Some(Ordering::Equal));
}

// Equals_SameClock_ShouldReturnTrue
#[test]
fn equals_same_clock_should_return_true() {
    assert_eq!(vc(&[("node1", 5)]), vc(&[("node1", 5)]));
}

// Equals_DifferentClock_ShouldReturnFalse
#[test]
fn equals_different_clock_should_return_false() {
    assert_ne!(vc(&[("node1", 5)]), vc(&[("node1", 3)]));
}

// Equals_Null_ShouldReturnFalse
#[test]
fn equals_null_should_return_false() {
    assert_ne!(Some(VectorClock::new()), None);
}

// Equals_Object_ShouldWorkCorrectly
#[test]
fn equals_object_should_work_correctly() {
    let any: &dyn std::any::Any = &vc(&[("node1", 5)]);
    assert_eq!(
        any.downcast_ref::<VectorClock>(),
        Some(&vc(&[("node1", 5)]))
    );
}

// GetHashCode_EqualClocks_ShouldBeEqual
#[test]
fn get_hash_code_equal_clocks_should_be_equal() {
    let a = vc(&[("node1", 5), ("node2", 3)]);
    let b = vc(&[("node2", 3), ("node1", 5)]);
    assert_eq!(hash(&a), hash(&b));
    // Zero entries do not change equality, so they must not change the hash either.
    assert_eq!(hash(&vc(&[("x", 0)])), hash(&VectorClock::new()));
}

// Serialize_ShouldProduceValidJson
#[test]
fn serialize_should_produce_valid_json() {
    let json = vc(&[("node1", 5), ("node2", 3)]).serialize();
    assert_eq!(json, r#"{"node1":5,"node2":3}"#);
}

// Deserialize_ValidJson_ShouldReconstructClock
#[test]
fn deserialize_valid_json_should_reconstruct_clock() {
    let original = vc(&[("node1", 5), ("node2", 3)]);
    assert_eq!(
        VectorClock::deserialize(&original.serialize()).unwrap(),
        original
    );
}

// Deserialize_EmptyObject_ShouldReturnEmptyClock
#[test]
fn deserialize_empty_object_should_return_empty_clock() {
    assert!(VectorClock::deserialize("{}").unwrap().entries().is_empty());
    assert!(
        VectorClock::deserialize("null")
            .unwrap()
            .entries()
            .is_empty()
    );
    assert_eq!(
        VectorClock::deserialize("[1]").unwrap_err().code(),
        arcana_core::ErrorCode::DataCorruption
    );
}

// RoundTrip_ShouldPreserveData
#[test]
fn round_trip_should_preserve_data() {
    let r = VectorClock::deserialize(&vc(&[("node1", 5), ("node2", 3), ("node3", 10)]).serialize())
        .unwrap();
    assert_eq!(
        (
            r.get_value("node1"),
            r.get_value("node2"),
            r.get_value("node3")
        ),
        (5, 3, 10)
    );
    // serde round trip uses the same shape.
    let s = serde_json::to_string(&r).unwrap();
    assert_eq!(serde_json::from_str::<VectorClock>(&s).unwrap(), r);
}

// ToString_EmptyClock_ShouldReturnEmptyFormat
#[test]
fn to_string_empty_clock_should_return_empty_format() {
    assert_eq!(VectorClock::new().to_string(), "VectorClock()");
}

// ToString_WithValues_ShouldIncludeAllPairs
#[test]
fn to_string_with_values_should_include_all_pairs() {
    assert_eq!(
        vc(&[("node1", 5), ("a", 1)]).to_string(),
        "VectorClock(a:1, node1:5)"
    );
}

// Scenario_TwoNodesEditingConcurrently_ShouldDetectConcurrency
#[test]
fn scenario_two_nodes_editing_concurrently_should_detect_concurrency() {
    let init = VectorClock::new();
    let a = init.increment("nodeA");
    let b = init.increment("nodeB");
    assert_eq!(a.compare_to(&b), CausalRelation::Concurrent);
    assert_eq!(b.compare_to(&a), CausalRelation::Concurrent);
}

// Scenario_SequentialEdits_ShouldDetectCausality
#[test]
fn scenario_sequential_edits_should_detect_causality() {
    let a1 = VectorClock::new().increment("nodeA");
    let b1 = a1.merge(&VectorClock::new()).increment("nodeB");
    assert_eq!(b1.compare_to(&a1), CausalRelation::HappenedAfter);
    assert_eq!(a1.compare_to(&b1), CausalRelation::HappenedBefore);
}

// Scenario_MergeAfterConflict_ShouldHaveCorrectClock
#[test]
fn scenario_merge_after_conflict_should_have_correct_clock() {
    let init = VectorClock::new();
    let a = init.increment("nodeA");
    let b = init.increment("nodeB");
    let m = a.merge(&b);
    assert_eq!(m.compare_to(&a), CausalRelation::HappenedAfter);
    assert_eq!(m.compare_to(&b), CausalRelation::HappenedAfter);
}
