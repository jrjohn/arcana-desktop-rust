//! C# `CollectionExtensionsTests` (28). C# `CollectionExtensions` maps onto std (see
//! `arcana_core::extensions`): `IsNullOrEmpty`/`HasItems` → `Option<&[T]>` +
//! `is_none_or(is_empty)`, `AddRange` → `Extend`, `OrEmpty` → `unwrap_or_default`,
//! `ForEach` → `for`, `ForEachAsync` → sequential `.await`, `Batch` → `chunks`. These tests
//! pin that mapping one-for-one.

use std::collections::BTreeSet;
use std::hint::black_box;

fn is_null_or_empty<T>(s: Option<&[T]>) -> bool {
    s.is_none_or(<[T]>::is_empty)
}

fn has_items<T>(s: Option<&[T]>) -> bool {
    !is_null_or_empty(s)
}

// IsNullOrEmpty_NullCollection_ShouldReturnTrue
#[test]
fn is_null_or_empty_null_collection_should_return_true() {
    assert!(is_null_or_empty::<i32>(None));
}

// IsNullOrEmpty_EmptyCollection_ShouldReturnTrue
#[test]
fn is_null_or_empty_empty_collection_should_return_true() {
    assert!(is_null_or_empty::<String>(Some(&[])));
}

// IsNullOrEmpty_NonEmptyCollection_ShouldReturnFalse
#[test]
fn is_null_or_empty_non_empty_collection_should_return_false() {
    assert!(!is_null_or_empty(Some(&[1, 2, 3][..])));
}

// IsNullOrEmpty_SingleElement_ShouldReturnFalse
#[test]
fn is_null_or_empty_single_element_should_return_false() {
    assert!(!is_null_or_empty(Some(&["hello"][..])));
}

// HasItems_NullCollection_ShouldReturnFalse
#[test]
fn has_items_null_collection_should_return_false() {
    assert!(!has_items::<i32>(None));
}

// HasItems_EmptyCollection_ShouldReturnFalse
#[test]
fn has_items_empty_collection_should_return_false() {
    assert!(!has_items::<String>(Some(&[])));
}

// HasItems_NonEmptyCollection_ShouldReturnTrue
#[test]
fn has_items_non_empty_collection_should_return_true() {
    assert!(has_items(Some(&[1, 2, 3][..])));
}

// HasItems_SingleElement_ShouldReturnTrue
#[test]
fn has_items_single_element_should_return_true() {
    assert!(has_items(Some(&["item"][..])));
}

// HasItems_IsInverseOfIsNullOrEmpty
#[test]
fn has_items_is_inverse_of_is_null_or_empty() {
    for src in [None, Some(&[][..]), Some(&[1][..])] {
        assert_eq!(has_items(src), !is_null_or_empty(src));
    }
}

// AddRange_AddsAllItemsToCollection
#[test]
fn add_range_adds_all_items_to_collection() {
    let mut list = vec![1, 2];
    list.extend([3, 4, 5]);
    assert_eq!(list, [1, 2, 3, 4, 5]);
}

// AddRange_EmptyItems_ShouldNotChangeCollection
#[test]
fn add_range_empty_items_should_not_change_collection() {
    let mut list = vec!["a", "b"];
    list.extend(std::iter::empty::<&str>());
    assert_eq!(list, ["a", "b"]);
}

// AddRange_ToEmptyCollection_ShouldAddAll
#[test]
fn add_range_to_empty_collection_should_add_all() {
    let mut list = Vec::new();
    list.extend([10, 20, 30]);
    assert_eq!(list, [10, 20, 30]);
}

// AddRange_WorksWithHashSet
#[test]
fn add_range_works_with_hash_set() {
    let mut set = BTreeSet::from([1, 2]);
    set.extend([3, 4, 5]);
    assert_eq!(set, BTreeSet::from([1, 2, 3, 4, 5]));
}

// OrEmpty_NullSource_ShouldReturnEmpty
#[test]
fn or_empty_null_source_should_return_empty() {
    let source: Option<Vec<i32>> = black_box(None);
    assert!(source.unwrap_or_default().is_empty());
}

// OrEmpty_NonNullSource_ShouldReturnSame
#[test]
fn or_empty_non_null_source_should_return_same() {
    assert_eq!(
        black_box(Some(vec![1, 2, 3])).unwrap_or_default(),
        [1, 2, 3]
    );
}

// OrEmpty_EmptySource_ShouldReturnEmpty
#[test]
fn or_empty_empty_source_should_return_empty() {
    assert!(
        black_box(Some(Vec::<String>::new()))
            .unwrap_or_default()
            .is_empty()
    );
}

// ForEach_ExecutesActionForEachItem
#[test]
fn for_each_executes_action_for_each_item() {
    let mut results = Vec::new();
    for i in [1, 2, 3] {
        results.push(i * 2);
    }
    assert_eq!(results, [2, 4, 6]);
}

// ForEach_EmptyCollection_ShouldNotExecuteAction
#[test]
fn for_each_empty_collection_should_not_execute_action() {
    let mut count = 0;
    for _ in std::iter::empty::<&str>() {
        count += 1;
    }
    assert_eq!(count, 0);
}

// ForEach_SingleElement_ShouldExecuteOnce
#[test]
fn for_each_single_element_should_execute_once() {
    let mut count = 0;
    for _ in black_box(["x"]) {
        count += 1;
    }
    assert_eq!(count, 1);
}

// ForEach_PreservesOrder
#[test]
fn for_each_preserves_order() {
    let mut order = Vec::new();
    for i in [5, 3, 1, 4, 2] {
        order.push(i);
    }
    assert_eq!(order, [5, 3, 1, 4, 2]);
}

// ForEachAsync_ExecutesActionForEachItem
#[tokio::test]
async fn for_each_async_executes_action_for_each_item() {
    let mut results = Vec::new();
    for i in [1, 2, 3] {
        tokio::task::yield_now().await;
        results.push(i);
    }
    assert_eq!(results, [1, 2, 3]);
}

// ForEachAsync_EmptyCollection_ShouldNotExecute
#[tokio::test]
async fn for_each_async_empty_collection_should_not_execute() {
    let mut count = 0;
    for _ in std::iter::empty::<i32>() {
        tokio::task::yield_now().await;
        count += 1;
    }
    assert_eq!(count, 0);
}

// Batch_ExactMultiple_ShouldSplitEvenly
#[test]
fn batch_exact_multiple_should_split_evenly() {
    let items: Vec<_> = (1..=6).collect();
    let b: Vec<_> = items.chunks(2).collect();
    assert_eq!(b, [&[1, 2][..], &[3, 4], &[5, 6]]);
}

// Batch_RemainingItems_LastBatchShouldBeSmallerThanSize
#[test]
fn batch_remaining_items_last_batch_should_be_smaller_than_size() {
    let items: Vec<_> = (1..=7).collect();
    let b: Vec<_> = items.chunks(3).collect();
    assert_eq!(b, [&[1, 2, 3][..], &[4, 5, 6], &[7]]);
}

// Batch_EmptySource_ShouldReturnNoBatches
#[test]
fn batch_empty_source_should_return_no_batches() {
    assert_eq!(Vec::<i32>::new().chunks(10).count(), 0);
}

// Batch_BatchSizeLargerThanCollection_ShouldReturnSingleBatch
#[test]
fn batch_batch_size_larger_than_collection_should_return_single_batch() {
    let b: Vec<_> = [1, 2, 3].chunks(100).collect();
    assert_eq!(b, [&[1, 2, 3][..]]);
}

// Batch_BatchSizeOne_EachItemInOwnBatch
#[test]
fn batch_batch_size_one_each_item_in_own_batch() {
    let b: Vec<_> = ["a", "b", "c"].chunks(1).collect();
    assert_eq!(b, [&["a"][..], &["b"], &["c"]]);
}

// Batch_TotalItemCountPreserved
#[test]
fn batch_total_item_count_preserved() {
    let items: Vec<_> = (1..=100).collect();
    assert_eq!(items.chunks(7).map(<[i32]>::len).sum::<usize>(), 100);
}
