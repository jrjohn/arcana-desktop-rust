//! C# `EffectSubjectTests.cs`: `EffectSubjectGenericTests` (18) + `EffectSubjectVoidTests` (9),
//! all 27 ported. A throwing C# subscriber is a panicking Rust one. "Dispose twice" on a
//! *subscription* cannot be expressed (`Subscription::dispose(self)` consumes it), so that test
//! disposes after the subject is gone instead.

use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use arcana_plugin_api::mvvm::{EffectSubject, Unit};

fn count() -> Arc<AtomicUsize> {
    Arc::new(AtomicUsize::new(0))
}

// ── EffectSubjectGenericTests ──

// Emit_NoSubscribers_ShouldNotThrow
#[test]
fn emit_no_subscribers() {
    EffectSubject::<String>::new().emit(&"hello".to_string());
}

// Emit_SingleSubscriber_ShouldReceiveValue
#[test]
fn emit_single() {
    let s = EffectSubject::<String>::new();
    let got = Arc::new(Mutex::new(None));
    let g = got.clone();
    let _sub = s.subscribe(move |v| *g.lock().unwrap() = Some(v.clone()));
    s.emit(&"hello".to_string());
    assert_eq!(got.lock().unwrap().as_deref(), Some("hello"));
}

// Emit_MultipleSubscribers_ShouldDeliverToAll
#[test]
fn emit_multiple() {
    let s = EffectSubject::<i32>::new();
    let r = Arc::new(Mutex::new(Vec::new()));
    let subs: Vec<_> = (1..=3)
        .map(|k| {
            let r = r.clone();
            s.subscribe(move |v| r.lock().unwrap().push(v * k))
        })
        .collect();
    s.emit(&5);
    assert_eq!(*r.lock().unwrap(), [5, 10, 15]);
    drop(subs);
}

// Emit_CalledMultipleTimes_SubscriberReceivesEach
#[test]
fn emit_many_times() {
    let s = EffectSubject::<String>::new();
    let r = Arc::new(Mutex::new(Vec::new()));
    let r2 = r.clone();
    let _sub = s.subscribe(move |v: &String| r2.lock().unwrap().push(v.clone()));
    for v in ["a", "b", "c"] {
        s.emit(&v.to_string());
    }
    assert_eq!(*r.lock().unwrap(), ["a", "b", "c"]);
}

// Emit_SubscriberThrows_ShouldContinueToNextSubscriber
#[test]
fn emit_panicking_subscriber_isolated() {
    let s = EffectSubject::<String>::new();
    let c = count();
    let c2 = c.clone();
    let _a = s.subscribe(|_| panic!("boom"));
    let _b = s.subscribe(move |_| {
        c2.fetch_add(1, Ordering::SeqCst);
    });
    s.emit(&"test".to_string());
    assert_eq!(c.load(Ordering::SeqCst), 1);
}

// Emit_AfterDispose_ShouldNotDeliverToSubscribers
#[test]
fn emit_after_dispose() {
    let s = EffectSubject::<String>::new();
    let c = count();
    let c2 = c.clone();
    let _a = s.subscribe(move |_| {
        c2.fetch_add(1, Ordering::SeqCst);
    });
    s.dispose();
    s.emit(&"ignored".to_string());
    assert_eq!(c.load(Ordering::SeqCst), 0);
}

// Subscribe_ReturnsDisposable
#[test]
fn subscribe_returns_subscription() {
    let s = EffectSubject::<String>::new();
    let sub = s.subscribe(|_| {});
    assert_eq!(s.subscriber_count(), 1);
    sub.dispose();
}

// Subscribe_ThenDispose_ShouldStopReceiving
#[test]
fn subscribe_then_dispose() {
    let s = EffectSubject::<String>::new();
    let got = Arc::new(Mutex::new(String::new()));
    let g = got.clone();
    let sub = s.subscribe(move |v: &String| g.lock().unwrap().clone_from(v));
    s.emit(&"first".to_string());
    sub.dispose();
    s.emit(&"second".to_string());
    assert_eq!(*got.lock().unwrap(), "first");
}

// Subscribe_DisposeOnlyRemovesOwnSubscription
#[test]
fn dispose_only_own() {
    let s = EffectSubject::<i32>::new();
    let (a, b) = (count(), count());
    let (a2, b2) = (a.clone(), b.clone());
    let s1 = s.subscribe(move |_| {
        a2.fetch_add(1, Ordering::SeqCst);
    });
    let _s2 = s.subscribe(move |_| {
        b2.fetch_add(1, Ordering::SeqCst);
    });
    s1.dispose();
    s.emit(&42);
    assert_eq!(a.load(Ordering::SeqCst), 0);
    assert_eq!(b.load(Ordering::SeqCst), 1);
}

// Subscribe_DisposeCalledTwice_ShouldNotThrow
#[test]
fn subscription_dispose_after_subject_dropped() {
    let s = EffectSubject::<String>::new();
    let sub = s.subscribe(|_| {});
    drop(s);
    sub.dispose();
}

// Subscribe_AfterDisposed_ShouldReturnEmptyDisposable
#[test]
fn subscribe_after_disposed_is_empty() {
    let s = EffectSubject::<String>::new();
    s.dispose();
    let sub = s.subscribe(|_| {});
    assert_eq!(s.subscriber_count(), 0);
    assert_eq!(format!("{sub:?}"), "Subscription(empty)");
}

// Subscribe_WithErrorHandler_ReceivesValueNormally
#[test]
fn error_handler_normal_value() {
    let s = EffectSubject::<i32>::new();
    let got = Arc::new(AtomicI32::new(0));
    let err = Arc::new(Mutex::new(None::<String>));
    let (g, e) = (got.clone(), err.clone());
    let _sub = s.subscribe_with_error(
        move |v| {
            g.store(*v, Ordering::SeqCst);
            Ok::<(), String>(())
        },
        move |x| *e.lock().unwrap() = Some(x),
    );
    s.emit(&99);
    assert_eq!(got.load(Ordering::SeqCst), 99);
    assert!(err.lock().unwrap().is_none());
}

// Subscribe_WithErrorHandler_OnNextThrows_ShouldCallOnError
#[test]
fn error_handler_receives_error() {
    let s = EffectSubject::<i32>::new();
    let err = Arc::new(Mutex::new(None::<String>));
    let e = err.clone();
    let _sub = s.subscribe_with_error(
        |_| Err("test error".to_string()),
        move |x| *e.lock().unwrap() = Some(x),
    );
    s.emit(&1);
    assert_eq!(err.lock().unwrap().as_deref(), Some("test error"));
}

// Subscribe_WithErrorHandler_AfterDisposed_ShouldReturnEmptyDisposable
#[test]
fn error_handler_after_disposed_is_empty() {
    let s = EffectSubject::<String>::new();
    s.dispose();
    let sub = s.subscribe_with_error(|_| Ok::<(), String>(()), |_| {});
    assert_eq!(format!("{sub:?}"), "Subscription(empty)");
}

// Dispose_ClearsAllSubscribers
#[test]
fn dispose_clears_all() {
    let s = EffectSubject::<String>::new();
    let c = count();
    let subs: Vec<_> = (0..2)
        .map(|_| {
            let c = c.clone();
            s.subscribe(move |_| {
                c.fetch_add(1, Ordering::SeqCst);
            })
        })
        .collect();
    s.dispose();
    s.emit(&"test".to_string());
    assert_eq!(c.load(Ordering::SeqCst), 0);
    assert_eq!(s.subscriber_count(), 0);
    drop(subs);
}

// Dispose_CalledTwice_ShouldNotThrow
#[test]
fn dispose_twice() {
    let s = EffectSubject::<String>::new();
    s.dispose();
    s.dispose();
    assert!(s.is_disposed());
}

// Emit_FromMultipleThreads_ShouldNotThrow
#[test]
fn emit_from_threads() {
    let s = EffectSubject::<usize>::new();
    let c = count();
    let c2 = c.clone();
    let _sub = s.subscribe(move |v| {
        c2.fetch_add(*v, Ordering::SeqCst);
    });
    std::thread::scope(|sc| {
        for _ in 0..50 {
            sc.spawn(|| s.emit(&1));
        }
    });
    assert_eq!(c.load(Ordering::SeqCst), 50);
}

// Subscribe_ConcurrentlyFromMultipleThreads_ShouldNotThrow
#[test]
fn subscribe_from_threads() {
    let s = EffectSubject::<String>::new();
    std::thread::scope(|sc| {
        for _ in 0..20 {
            sc.spawn(|| s.subscribe(|_| {}));
        }
    });
    assert_eq!(s.subscriber_count(), 20);
}

// ── EffectSubjectVoidTests (EffectSubject<Unit> + fire) ──

fn counting(s: &EffectSubject<Unit>, c: &Arc<AtomicUsize>) -> arcana_plugin_api::Subscription {
    let c = c.clone();
    s.subscribe(move |_| {
        c.fetch_add(1, Ordering::SeqCst);
    })
}

// Emit_NoSubscribers_ShouldNotThrow (void)
#[test]
fn void_emit_no_subscribers() {
    EffectSubject::<Unit>::new().fire();
}

// Emit_SingleSubscriber_ShouldBeInvoked
#[test]
fn void_emit_single() {
    let s = EffectSubject::<Unit>::new();
    let c = count();
    let _a = counting(&s, &c);
    s.fire();
    assert_eq!(c.load(Ordering::SeqCst), 1);
}

// Emit_MultipleSubscribers_ShouldInvokeAll
#[test]
fn void_emit_multiple() {
    let s = EffectSubject::<Unit>::new();
    let c = count();
    let _subs: Vec<_> = (0..3).map(|_| counting(&s, &c)).collect();
    s.fire();
    assert_eq!(c.load(Ordering::SeqCst), 3);
}

// Emit_SubscriberThrows_ShouldContinueToNext
#[test]
fn void_emit_panicking_isolated() {
    let s = EffectSubject::<Unit>::new();
    let c = count();
    let _a = s.subscribe(|_| panic!("void-boom"));
    let _b = counting(&s, &c);
    s.fire();
    assert_eq!(c.load(Ordering::SeqCst), 1);
}

// Emit_AfterDispose_ShouldNotInvokeSubscribers
#[test]
fn void_emit_after_dispose() {
    let s = EffectSubject::<Unit>::new();
    let c = count();
    let _a = counting(&s, &c);
    s.dispose();
    s.fire();
    assert_eq!(c.load(Ordering::SeqCst), 0);
}

// Subscribe_ThenDispose_ShouldStopReceiving (void)
#[test]
fn void_subscribe_then_dispose() {
    let s = EffectSubject::<Unit>::new();
    let c = count();
    let sub = counting(&s, &c);
    s.fire();
    sub.dispose();
    s.fire();
    assert_eq!(c.load(Ordering::SeqCst), 1);
}

// Subscribe_AfterDisposed_ShouldReturnEmptyDisposable (void)
#[test]
fn void_subscribe_after_disposed() {
    let s = EffectSubject::<Unit>::new();
    s.dispose();
    let sub = s.subscribe(|_| {});
    assert_eq!(format!("{sub:?}"), "Subscription(empty)");
}

// Dispose_CalledTwice_ShouldNotThrow (void)
#[test]
fn void_dispose_twice() {
    let s = EffectSubject::<Unit>::new();
    s.dispose();
    s.dispose();
    assert!(s.is_disposed());
}

// Unit_Default_ShouldEqualDefault
#[test]
fn unit_default_equal() {
    let (u1, u2) = (Unit::DEFAULT, Unit::DEFAULT);
    assert_eq!(u1, u2);
    assert_eq!(u1, <Unit as Default>::default());
}
