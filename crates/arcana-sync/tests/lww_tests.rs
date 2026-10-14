//! C# `LWWRegisterTests` (15) and `LWWMapTests` (13).

use arcana_core::utc_now;
use arcana_sync::{LwwMap, LwwRegister};
use chrono::TimeDelta;

fn secs(n: i64) -> TimeDelta {
    TimeDelta::seconds(n)
}

// ---- LWWRegisterTests ----

// Constructor_ShouldSetInitialValues
#[test]
fn constructor_should_set_initial_values() {
    let t = utc_now();
    let r = LwwRegister::new("test", t, "node1");
    assert_eq!(
        (*r.value(), r.timestamp(), r.node_id()),
        ("test", t, "node1")
    );
}

// Constructor_WithNullValue_ShouldAllowNull
#[test]
fn constructor_with_null_value_should_allow_null() {
    let r = LwwRegister::<Option<String>>::new(None, utc_now(), "node1");
    assert_eq!(r.value(), &None);
}

// Update_NewerTimestamp_ShouldUpdateValue
#[test]
fn update_newer_timestamp_should_update_value() {
    let old = utc_now();
    let new = old + secs(1);
    let mut r = LwwRegister::new("old", old, "node1");
    assert!(r.update("new", new, "node2"));
    assert_eq!(
        (*r.value(), r.timestamp(), r.node_id()),
        ("new", new, "node2")
    );
}

// Update_OlderTimestamp_ShouldNotUpdate
#[test]
fn update_older_timestamp_should_not_update() {
    let new = utc_now();
    let mut r = LwwRegister::new("current", new, "node1");
    assert!(!r.update("old", new - secs(1), "node2"));
    assert_eq!(
        (*r.value(), r.timestamp(), r.node_id()),
        ("current", new, "node1")
    );
}

// Update_SameTimestampHigherNodeId_ShouldUpdate
#[test]
fn update_same_timestamp_higher_node_id_should_update() {
    let t = utc_now();
    let mut r = LwwRegister::new("old", t, "nodeA");
    assert!(r.update("new", t, "nodeZ"));
    assert_eq!((*r.value(), r.node_id()), ("new", "nodeZ"));
}

// Update_SameTimestampLowerNodeId_ShouldNotUpdate
#[test]
fn update_same_timestamp_lower_node_id_should_not_update() {
    let t = utc_now();
    let mut r = LwwRegister::new("current", t, "nodeZ");
    assert!(!r.update("new", t, "nodeA"));
    assert_eq!((*r.value(), r.node_id()), ("current", "nodeZ"));
}

// Update_SameTimestampSameNodeId_ShouldNotUpdate
#[test]
fn update_same_timestamp_same_node_id_should_not_update() {
    let t = utc_now();
    let mut r = LwwRegister::new("current", t, "node1");
    assert!(!r.update("new", t, "node1"));
    assert_eq!(*r.value(), "current");
}

// Update_WithNullValue_ShouldUpdateToNull
#[test]
fn update_with_null_value_should_update_to_null() {
    let t = utc_now();
    let mut r = LwwRegister::new(Some("value"), t, "node1");
    assert!(r.update(None, t + secs(1), "node2"));
    assert_eq!(r.value(), &None);
}

// Merge_OtherNewer_ShouldReturnOther
#[test]
fn merge_other_newer_should_return_other() {
    let old = utc_now();
    let new = old + secs(1);
    let m = LwwRegister::new("old", old, "node1").merge(&LwwRegister::new("new", new, "node2"));
    assert_eq!(
        (*m.value(), m.timestamp(), m.node_id()),
        ("new", new, "node2")
    );
}

// Merge_ThisNewer_ShouldReturnThis
#[test]
fn merge_this_newer_should_return_this() {
    let old = utc_now();
    let new = old + secs(1);
    let m = LwwRegister::new("new", new, "node1").merge(&LwwRegister::new("old", old, "node2"));
    assert_eq!(
        (*m.value(), m.timestamp(), m.node_id()),
        ("new", new, "node1")
    );
}

// Merge_SameTimestampHigherNodeId_ShouldReturnHigherNode
#[test]
fn merge_same_timestamp_higher_node_id_should_return_higher_node() {
    let t = utc_now();
    let m = LwwRegister::new("value1", t, "nodeA").merge(&LwwRegister::new("value2", t, "nodeZ"));
    assert_eq!((*m.value(), m.node_id()), ("value2", "nodeZ"));
}

// Merge_ShouldReturnNewInstance
#[test]
fn merge_should_return_new_instance() {
    let t = utc_now();
    let a = LwwRegister::new("value1".to_owned(), t, "node1");
    let b = LwwRegister::new("value2".to_owned(), t - secs(1), "node2");
    let mut m = a.merge(&b);
    m.update("changed".into(), t + secs(5), "node3");
    assert_eq!(a.value(), "value1");
    assert_eq!(b.value(), "value2");
}

// Merge_ShouldBeCommutative
#[test]
fn merge_should_be_commutative() {
    let t1 = utc_now();
    let a = LwwRegister::new("value1", t1, "node1");
    let b = LwwRegister::new("value2", t1 + secs(1), "node2");
    assert_eq!(a.merge(&b), b.merge(&a));
}

// LWWRegister_WithComplexType_ShouldWork
#[test]
fn lww_register_with_complex_type_should_work() {
    #[derive(Debug, Clone, PartialEq)]
    struct TestEntity {
        id: i32,
        name: String,
    }
    let r = LwwRegister::new(
        Some(TestEntity {
            id: 1,
            name: "Test".into(),
        }),
        utc_now(),
        "node1",
    );
    let v = r.value().as_ref().unwrap();
    assert_eq!((v.id, v.name.as_str()), (1, "Test"));
}

// LWWRegister_WithValueType_ShouldWork
#[test]
fn lww_register_with_value_type_should_work() {
    assert_eq!(*LwwRegister::new(42, utc_now(), "node1").value(), 42);
}

// ---- LWWMapTests ----

// Set_NewField_ShouldAddField
#[test]
fn set_new_field_should_add_field() {
    let mut m = LwwMap::new();
    m.set("field1", &"value1", utc_now(), "node1");
    assert!(m.fields().contains_key("field1"));
    assert_eq!(m.get::<String>("field1").as_deref(), Some("value1"));
}

// Set_ExistingFieldNewerTimestamp_ShouldUpdate
#[test]
fn set_existing_field_newer_timestamp_should_update() {
    let mut m = LwwMap::new();
    let t = utc_now();
    m.set("field1", &"old", t, "node1");
    m.set("field1", &"new", t + secs(1), "node2");
    assert_eq!(m.get::<String>("field1").as_deref(), Some("new"));
}

// Set_ExistingFieldOlderTimestamp_ShouldNotUpdate
#[test]
fn set_existing_field_older_timestamp_should_not_update() {
    let mut m = LwwMap::new();
    let t = utc_now();
    m.set("field1", &"current", t, "node1");
    m.set("field1", &"old", t - secs(1), "node2");
    assert_eq!(m.get::<String>("field1").as_deref(), Some("current"));
}

// Set_MultipleFields_ShouldStoreSeparately
#[test]
fn set_multiple_fields_should_store_separately() {
    let mut m = LwwMap::new();
    let t = utc_now();
    for i in 1..=3 {
        m.set(&format!("field{i}"), &format!("value{i}"), t, "node1");
    }
    assert_eq!(m.fields().len(), 3);
    for i in 1..=3 {
        assert_eq!(
            m.get::<String>(&format!("field{i}")),
            Some(format!("value{i}"))
        );
    }
}

// Get_NonExistingField_ShouldReturnDefault
#[test]
fn get_non_existing_field_should_return_default() {
    assert_eq!(LwwMap::new().get::<String>("nonexistent"), None);
}

// Get_WrongType_ShouldReturnDefault
#[test]
fn get_wrong_type_should_return_default() {
    let mut m = LwwMap::new();
    m.set("field1", &"string value", utc_now(), "node1");
    assert_eq!(m.get::<i32>("field1"), None);
    assert_eq!(m.get::<i32>("field1").unwrap_or_default(), 0);
}

// Get_CorrectType_ShouldReturnValue
#[test]
fn get_correct_type_should_return_value() {
    let mut m = LwwMap::new();
    m.set("field1", &42, utc_now(), "node1");
    assert_eq!(m.get::<i32>("field1"), Some(42));
}

// Merge_DisjointFields_ShouldIncludeAll
#[test]
fn merge_disjoint_fields_should_include_all() {
    let t = utc_now();
    let mut a = LwwMap::new();
    a.set("field1", &"value1", t, "node1");
    let mut b = LwwMap::new();
    b.set("field2", &"value2", t, "node2");
    let m = a.merge(&b);
    assert_eq!(m.fields().len(), 2);
    assert_eq!(m.get::<String>("field1").as_deref(), Some("value1"));
    assert_eq!(m.get::<String>("field2").as_deref(), Some("value2"));
}

// Merge_OverlappingFields_ShouldTakeNewer
#[test]
fn merge_overlapping_fields_should_take_newer() {
    let t = utc_now();
    let mut a = LwwMap::new();
    a.set("field1", &"old", t, "node1");
    let mut b = LwwMap::new();
    b.set("field1", &"new", t + secs(1), "node2");
    assert_eq!(a.merge(&b).get::<String>("field1").as_deref(), Some("new"));
}

// Merge_ShouldReturnNewInstance
#[test]
fn merge_should_return_new_instance_map() {
    let a = LwwMap::new();
    let b = LwwMap::new();
    let mut m = a.merge(&b);
    m.set("x", &1, utc_now(), "n");
    assert!(a.fields().is_empty() && b.fields().is_empty());
}

// Merge_ShouldBeCommutative
#[test]
fn merge_should_be_commutative_map() {
    let t = utc_now();
    let mut a = LwwMap::new();
    a.set("field1", &"value1", t, "node1");
    let mut b = LwwMap::new();
    b.set("field1", &"value2", t + secs(1), "node2");
    assert_eq!(
        a.merge(&b).get::<String>("field1"),
        b.merge(&a).get::<String>("field1")
    );
    assert_eq!(a.merge(&b), b.merge(&a));
}

// Merge_MixedFieldTimestamps_ShouldMergeFieldByField
#[test]
fn merge_mixed_field_timestamps_should_merge_field_by_field() {
    let t1 = utc_now();
    let t2 = t1 + secs(1);
    let mut a = LwwMap::new();
    a.set("field1", &"map1-newer", t2, "node1");
    a.set("field2", &"map1-older", t1, "node1");
    let mut b = LwwMap::new();
    b.set("field1", &"map2-older", t1, "node2");
    b.set("field2", &"map2-newer", t2, "node2");
    let m = a.merge(&b);
    assert_eq!(m.get::<String>("field1").as_deref(), Some("map1-newer"));
    assert_eq!(m.get::<String>("field2").as_deref(), Some("map2-newer"));
}

// Scenario_ConcurrentEditsToSameDocument
#[test]
fn scenario_concurrent_edits_to_same_document() {
    let base = utc_now();
    let mut a = LwwMap::new();
    a.set("title", &"User A's Title", base + secs(1), "userA");
    a.set("content", &"Original content", base, "system");
    let mut b = LwwMap::new();
    b.set("title", &"Original Title", base, "system");
    b.set("content", &"User B's Content", base + secs(2), "userB");
    let m = a.merge(&b);
    assert_eq!(m.get::<String>("title").as_deref(), Some("User A's Title"));
    assert_eq!(
        m.get::<String>("content").as_deref(),
        Some("User B's Content")
    );
}
