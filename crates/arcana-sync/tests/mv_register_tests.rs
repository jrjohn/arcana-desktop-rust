//! C# `MVRegisterTests` (20).

use arcana_sync::{MvRegister, VectorClock};

fn inc(node: &str) -> VectorClock {
    VectorClock::new().increment(node)
}

fn vals(r: &MvRegister<&'static str>) -> Vec<&'static str> {
    let mut v: Vec<_> = r.values().iter().map(|(v, _)| *v).collect();
    v.sort_unstable();
    v
}

// Constructor_Default_ShouldBeEmpty
#[test]
fn constructor_default_should_be_empty() {
    let r = MvRegister::<String>::new();
    assert!(r.values().is_empty());
    assert!(!r.has_conflict());
}

// Constructor_WithValue_ShouldContainSingleValue
#[test]
fn constructor_with_value_should_contain_single_value() {
    let r = MvRegister::with_value("value", inc("node1"));
    assert_eq!(vals(&r), ["value"]);
    assert!(!r.has_conflict());
}

// Set_ToEmpty_ShouldAddValue
#[test]
fn set_to_empty_should_add_value() {
    let mut r = MvRegister::new();
    r.set("value", inc("node1"));
    assert_eq!(vals(&r), ["value"]);
}

// Set_NewerValue_ShouldReplacePrevious
#[test]
fn set_newer_value_should_replace_previous() {
    let c1 = inc("node1");
    let c2 = c1.increment("node1");
    let mut r = MvRegister::with_value("old", c1);
    r.set("new", c2);
    assert_eq!(vals(&r), ["new"]);
    assert!(!r.has_conflict());
}

// Set_ConcurrentValues_ShouldCreateConflict
#[test]
fn set_concurrent_values_should_create_conflict() {
    let mut r = MvRegister::new();
    r.set("value1", inc("node1"));
    r.set("value2", inc("node2"));
    assert_eq!(r.values().len(), 2);
    assert!(r.has_conflict());
}

// Set_OlderValue_ShouldNotAdd
#[test]
fn set_older_value_should_not_add() {
    let c1 = inc("node1");
    let c2 = c1.increment("node1");
    let mut r = MvRegister::with_value("new", c2);
    r.set("old", c1);
    assert_eq!(vals(&r), ["new"]);
}

// Set_NewValueSupersedingMultiple_ShouldReplaceAll
#[test]
fn set_new_value_superseding_multiple_should_replace_all() {
    let (c1, c2) = (inc("node1"), inc("node2"));
    let mut r = MvRegister::new();
    r.set("value1", c1.clone());
    r.set("value2", c2.clone());
    assert!(r.has_conflict());
    r.set("resolved", c1.merge(&c2).increment("node1"));
    assert_eq!(vals(&r), ["resolved"]);
    assert!(!r.has_conflict());
}

// SingleValue_WithOneValue_ShouldReturnValue
#[test]
fn single_value_with_one_value_should_return_value() {
    assert_eq!(
        *MvRegister::with_value("single", inc("node1"))
            .single_value()
            .unwrap(),
        "single"
    );
}

// SingleValue_WithMultipleValues_ShouldThrow
#[test]
fn single_value_with_multiple_values_should_throw() {
    let mut r = MvRegister::new();
    r.set("value1", inc("node1"));
    r.set("value2", inc("node2"));
    assert_eq!(
        r.single_value().unwrap_err().code(),
        arcana_core::ErrorCode::ConflictError
    );
    assert!(MvRegister::<u8>::new().single_value().is_err());
}

// Merge_EmptyRegisters_ShouldReturnEmpty
#[test]
fn merge_empty_registers_should_return_empty() {
    assert!(
        MvRegister::<u8>::new()
            .merge(&MvRegister::new())
            .values()
            .is_empty()
    );
}

// Merge_OneEmpty_ShouldReturnOther
#[test]
fn merge_one_empty_should_return_other() {
    let m = MvRegister::with_value("value", inc("node1")).merge(&MvRegister::new());
    assert_eq!(vals(&m), ["value"]);
}

// Merge_OneNewerThanOther_ShouldReturnNewer
#[test]
fn merge_one_newer_than_other_should_return_newer() {
    let c1 = inc("node1");
    let c2 = c1.increment("node1");
    let m = MvRegister::with_value("old", c1).merge(&MvRegister::with_value("new", c2));
    assert_eq!(vals(&m), ["new"]);
}

// Merge_ConcurrentValues_ShouldKeepBoth
#[test]
fn merge_concurrent_values_should_keep_both() {
    let m = MvRegister::with_value("value1", inc("node1"))
        .merge(&MvRegister::with_value("value2", inc("node2")));
    assert!(m.has_conflict());
    assert_eq!(vals(&m), ["value1", "value2"]);
}

// Merge_ShouldBeCommutative
#[test]
fn merge_should_be_commutative() {
    let a = MvRegister::with_value("value1", inc("node1"));
    let b = MvRegister::with_value("value2", inc("node2"));
    assert_eq!(vals(&a.merge(&b)), vals(&b.merge(&a)));
}

// Merge_ShouldRemoveDuplicateClocks
#[test]
fn merge_should_remove_duplicate_clocks() {
    let c = inc("node1");
    let m = MvRegister::with_value("value", c.clone()).merge(&MvRegister::with_value("value", c));
    assert_eq!(m.values().len(), 1);
}

// Merge_ComplexScenario_ShouldResolveCorrectly
#[test]
fn merge_complex_scenario_should_resolve_correctly() {
    let a1 = inc("nodeA");
    let b1 = inc("nodeB");
    let a2 = a1.merge(&b1).increment("nodeA");
    let mut r1 = MvRegister::new();
    r1.set("A1", a1);
    r1.set("B1", b1);
    let m = r1.merge(&MvRegister::with_value("A2", a2));
    assert_eq!(vals(&m), ["A2"]);
}

// Resolve_ShouldClearAllAndSetSingle
#[test]
fn resolve_should_clear_all_and_set_single() {
    let (c1, c2) = (inc("node1"), inc("node2"));
    let mut r = MvRegister::new();
    r.set("value1", c1.clone());
    r.set("value2", c2.clone());
    assert!(r.has_conflict());
    r.resolve("resolved", c1.merge(&c2).increment("resolver"));
    assert_eq!(vals(&r), ["resolved"]);
    assert!(!r.has_conflict());
}

// Resolve_WithUserSelection_ShouldSetSelectedValue
#[test]
fn resolve_with_user_selection_should_set_selected_value() {
    let (c1, c2) = (inc("node1"), inc("node2"));
    let mut r = MvRegister::new();
    r.set("option1", c1.clone());
    r.set("option2", c2.clone());
    r.resolve("option1", c1.merge(&c2).increment("user"));
    assert_eq!(*r.single_value().unwrap(), "option1");
}

// Scenario_ThreeWayConflict
#[test]
fn scenario_three_way_conflict() {
    let (a, b, c) = (inc("userA"), inc("userB"), inc("userC"));
    let mut r = MvRegister::new();
    r.set("User A's version", a.clone());
    r.set("User B's version", b.clone());
    r.set("User C's version", c.clone());
    assert_eq!(r.values().len(), 3);
    assert!(r.has_conflict());
    r.resolve(
        "Merged version from all users",
        a.merge(&b).merge(&c).increment("resolver"),
    );
    assert!(!r.has_conflict());
    assert_eq!(*r.single_value().unwrap(), "Merged version from all users");
}

// Scenario_OfflineSync
#[test]
fn scenario_offline_sync() {
    let m = MvRegister::with_value("Server version", inc("server")).merge(&MvRegister::with_value(
        "Offline changes",
        inc("offline_user"),
    ));
    assert!(m.has_conflict());
    assert_eq!(m.values().len(), 2);
}
