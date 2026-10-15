//! C# `ReactiveViewModelBaseTests` (5), all ported.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use arcana_core::AppResult;
use arcana_plugin_api::Subscription;
use arcana_plugin_api::mvvm::{DisposeBag, ReactiveViewModel};

#[derive(Default)]
struct TestViewModel {
    bag: DisposeBag,
    initialize_called: bool,
    cleanup_called: bool,
}

impl ReactiveViewModel for TestViewModel {
    fn bag(&mut self) -> &mut DisposeBag {
        &mut self.bag
    }
    fn initialize(&mut self) -> AppResult<()> {
        self.initialize_called = true;
        Ok(())
    }
    fn cleanup(&mut self) -> AppResult<()> {
        self.cleanup_called = true;
        Ok(())
    }
}

fn counting(c: &Arc<AtomicUsize>) -> Subscription {
    let c = c.clone();
    Subscription::new(move || {
        c.fetch_add(1, Ordering::SeqCst);
    })
}

// InitializeAsync_ShouldCompleteSuccessfully
#[test]
fn initialize_completes() {
    let mut vm = TestViewModel::default();
    vm.initialize().unwrap();
    assert!(vm.initialize_called);
}

// CleanupAsync_ShouldCompleteSuccessfully
#[test]
fn cleanup_completes() {
    let mut vm = TestViewModel::default();
    vm.cleanup().unwrap();
    assert!(vm.cleanup_called);
}

// Dispose_ShouldDisposeRegisteredDisposables
#[test]
fn dispose_disposes_registered() {
    let mut vm = TestViewModel::default();
    let c = Arc::new(AtomicUsize::new(0));
    vm.add_disposable(counting(&c));
    vm.dispose();
    assert_eq!(c.load(Ordering::SeqCst), 1);
}

// Dispose_CalledTwice_ShouldNotThrow
#[test]
fn dispose_twice() {
    let mut vm = TestViewModel::default();
    let c = Arc::new(AtomicUsize::new(0));
    vm.add_disposable(counting(&c));
    vm.dispose();
    vm.dispose();
    assert_eq!(c.load(Ordering::SeqCst), 1);
    assert!(vm.bag.is_disposed());
}

// AddDisposable_MultipleDisposables_ShouldAllBeDisposed
#[test]
fn all_disposables_disposed() {
    let mut vm = TestViewModel::default();
    let c = Arc::new(AtomicUsize::new(0));
    for _ in 0..3 {
        vm.add_disposable(counting(&c));
    }
    vm.dispose();
    assert_eq!(c.load(Ordering::SeqCst), 3);
}
