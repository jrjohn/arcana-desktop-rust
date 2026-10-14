//! New (no C# counterpart): CRDT laws checked with proptest — merge is commutative,
//! associative and idempotent, so replicas converge regardless of delivery order.
//!
//! Precondition shared with C#: a (timestamp, node) pair identifies one write, and a vector
//! clock identifies one value. Two *different* values written with the same stamp make LWW
//! keep whichever side `self` is, which is not commutative — proptest found exactly that, so
//! the generators derive the value from the stamp.

use arcana_core::Timestamp;
use arcana_sync::{CausalRelation, LwwMap, LwwRegister, MvRegister, VectorClock};
use chrono::DateTime;
use proptest::prelude::*;

const NODES: [&str; 4] = ["a", "b", "c", "d"];

fn clock() -> impl Strategy<Value = VectorClock> {
    prop::collection::btree_map(prop::sample::select(&NODES[..]), 0i64..5, 0..4)
        .prop_map(VectorClock::from_entries)
}

fn ts(secs: i64) -> Timestamp {
    DateTime::from_timestamp(1_700_000_000 + secs, 0)
        .unwrap()
        .naive_utc()
}

fn reg() -> impl Strategy<Value = LwwRegister<i32>> {
    (0i64..4, prop::sample::select(&NODES[..]))
        .prop_map(|(t, n)| LwwRegister::new(stamp_value(t, n), ts(t), n))
}

fn map() -> impl Strategy<Value = LwwMap> {
    prop::collection::vec(
        (
            prop::sample::select(&["x", "y", "z"][..]),
            0i64..4,
            prop::sample::select(&NODES[..]),
        ),
        0..6,
    )
    .prop_map(|writes| {
        let mut m = LwwMap::new();
        for (f, t, n) in writes {
            m.set(f, &(f, stamp_value(t, n)), ts(t), n);
        }
        m
    })
}

fn mv() -> impl Strategy<Value = MvRegister<u8>> {
    prop::collection::vec(clock(), 0..4).prop_map(|cs| {
        let mut r = MvRegister::new();
        for c in cs {
            r.set(clock_value(&c), c);
        }
        r
    })
}

fn stamp_value(t: i64, node: &str) -> i32 {
    i32::try_from(t).unwrap() * 100 + i32::from(node.as_bytes()[0])
}

fn clock_value(c: &VectorClock) -> u8 {
    u8::try_from(c.entries().values().sum::<i64>()).unwrap()
}

/// Set view of an MV register. `set` keeps a second copy of an equal clock (as in C#) while
/// `merge` de-duplicates, so laws are checked on the set of (value, clock) pairs.
fn mv_set(r: &MvRegister<u8>) -> std::collections::BTreeSet<(u8, String)> {
    // Causally equal clocks may differ by zero entries; compare the non-zero ones.
    r.values()
        .iter()
        .map(|(v, c)| {
            let nz: Vec<String> = c
                .entries()
                .iter()
                .filter(|(_, n)| **n != 0)
                .map(|(k, n)| format!("{k}:{n}"))
                .collect();
            (*v, nz.join(","))
        })
        .collect()
}

proptest! {
    #[test]
    fn vector_clock_merge_laws(a in clock(), b in clock(), c in clock()) {
        prop_assert_eq!(a.merge(&b), b.merge(&a));
        prop_assert_eq!(a.merge(&b).merge(&c), a.merge(&b.merge(&c)));
        prop_assert_eq!(a.merge(&a), a.clone());
        let m = a.merge(&b);
        prop_assert!(matches!(m.compare_to(&a), CausalRelation::HappenedAfter | CausalRelation::Equal));
    }

    #[test]
    fn vector_clock_relation_is_antisymmetric(a in clock(), b in clock()) {
        let flipped = match a.compare_to(&b) {
            CausalRelation::HappenedAfter => CausalRelation::HappenedBefore,
            CausalRelation::HappenedBefore => CausalRelation::HappenedAfter,
            other => other,
        };
        prop_assert_eq!(b.compare_to(&a), flipped);
    }

    #[test]
    fn lww_register_merge_laws(a in reg(), b in reg(), c in reg()) {
        prop_assert_eq!(a.merge(&b), b.merge(&a));
        prop_assert_eq!(a.merge(&b).merge(&c), a.merge(&b.merge(&c)));
        prop_assert_eq!(a.merge(&a), a.clone());
    }

    #[test]
    fn lww_map_merge_laws(a in map(), b in map(), c in map()) {
        prop_assert_eq!(a.merge(&b), b.merge(&a));
        prop_assert_eq!(a.merge(&b).merge(&c), a.merge(&b.merge(&c)));
        prop_assert_eq!(a.merge(&a), a.clone());
    }

    #[test]
    fn mv_register_merge_laws(a in mv(), b in mv(), c in mv()) {
        prop_assert_eq!(mv_set(&a.merge(&b)), mv_set(&b.merge(&a)));
        prop_assert_eq!(mv_set(&a.merge(&b).merge(&c)), mv_set(&a.merge(&b.merge(&c))));
        prop_assert_eq!(mv_set(&a.merge(&a)), mv_set(&a));
    }
}
