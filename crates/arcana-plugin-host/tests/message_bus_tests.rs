//! C# `MessageBusTests` (22). Ported 21; not ported: `PublishAsync_CancellationRequested_*`
//! (the Rust contract has no `CancellationToken` — publishing is synchronous).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use arcana_plugin_api::{MessageBusExt, Subscription};
use arcana_plugin_host::services::message_bus::MessageBusImpl;

#[derive(Debug, Clone, PartialEq)]
struct Ping(String);
#[derive(Debug, Clone, PartialEq)]
struct Pong(String);
#[derive(Debug, Clone)]
struct Counter(usize);

fn ping(s: &str) -> Ping {
    Ping(s.to_string())
}

fn counter() -> Arc<AtomicUsize> {
    Arc::new(AtomicUsize::new(0))
}

fn inc(c: &Arc<AtomicUsize>) -> impl Fn(&Ping) + Send + Sync + 'static {
    let c = c.clone();
    move |_| {
        c.fetch_add(1, Ordering::SeqCst);
    }
}

// PublishAsync_NoSubscribers_ShouldNotThrow
#[test]
fn publish_no_subscribers_does_not_fail() {
    MessageBusImpl::new().publish(&ping("hello"));
}

// PublishAsync_SyncSubscriber_ShouldReceiveMessage
#[test]
fn publish_sync_subscriber_receives() {
    let bus = MessageBusImpl::new();
    let got = Arc::new(Mutex::new(None));
    let g = got.clone();
    let _s = bus.subscribe(move |m: &Ping| *g.lock().unwrap() = Some(m.clone()));
    bus.publish(&ping("hi"));
    assert_eq!(*got.lock().unwrap(), Some(ping("hi")));
}

// PublishAsync_AsyncSubscriber_ShouldReceiveMessage — Rust has one (synchronous) handler kind.
#[test]
fn publish_async_style_subscriber_receives() {
    let bus = MessageBusImpl::new();
    let got = Arc::new(Mutex::new(None));
    let g = got.clone();
    let _s = bus.subscribe(move |m: &Ping| *g.lock().unwrap() = Some(m.0.clone()));
    bus.publish(&ping("async"));
    assert_eq!(got.lock().unwrap().as_deref(), Some("async"));
}

// PublishAsync_MultipleSubscribers_ShouldInvokeAll
#[test]
fn publish_multiple_subscribers_all_invoked() {
    let bus = MessageBusImpl::new();
    let log = Arc::new(Mutex::new(Vec::new()));
    let (a, b) = (log.clone(), log.clone());
    let _s1 = bus.subscribe(move |m: &Ping| a.lock().unwrap().push(format!("sync:{}", m.0)));
    let _s2 = bus.subscribe(move |m: &Ping| b.lock().unwrap().push(format!("async:{}", m.0)));
    bus.publish(&ping("test"));
    let log = log.lock().unwrap();
    assert_eq!(log.len(), 2);
    assert!(log.contains(&"sync:test".to_string()));
    assert!(log.contains(&"async:test".to_string()));
}

// PublishAsync_WrongMessageType_ShouldNotDeliverToOtherSubscriber
#[test]
fn publish_other_type_not_delivered() {
    let bus = MessageBusImpl::new();
    let c = counter();
    let c2 = c.clone();
    let _s = bus.subscribe(move |_: &Pong| {
        c2.fetch_add(1, Ordering::SeqCst);
    });
    bus.publish(&ping("ping"));
    assert_eq!(c.load(Ordering::SeqCst), 0);
}

// PublishAsync_HandlerThrows_ShouldContinueToNextSubscriber
#[test]
fn publish_panicking_handler_does_not_stop_next() {
    let bus = MessageBusImpl::new();
    let c = counter();
    let _s1 = bus.subscribe(|_: &Ping| panic!("boom"));
    let _s2 = bus.subscribe(inc(&c));
    bus.publish(&ping("test"));
    assert_eq!(c.load(Ordering::SeqCst), 1);
}

// PublishAsync_PublishMultipleTimes_ShouldDeliverEachTime
#[test]
fn publish_three_times_delivers_three() {
    let bus = MessageBusImpl::new();
    let c = counter();
    let _s = bus.subscribe(inc(&c));
    for t in ["1", "2", "3"] {
        bus.publish(&ping(t));
    }
    assert_eq!(c.load(Ordering::SeqCst), 3);
}

// Subscribe_SyncHandler_ReturnsDisposable
#[test]
fn subscribe_returns_subscription() {
    let bus = MessageBusImpl::new();
    let s: Subscription = bus.subscribe(|_: &Ping| {});
    s.dispose();
}

// Subscribe_AsyncHandler_ReturnsDisposable
#[test]
fn subscribe_async_style_returns_subscription() {
    let bus = MessageBusImpl::new();
    let s: Subscription = bus.subscribe(|_: &Ping| ());
    s.dispose();
}

// Subscribe_ThenDispose_ShouldStopReceivingMessages
#[test]
fn dispose_stops_delivery() {
    let bus = MessageBusImpl::new();
    let c = counter();
    let s = bus.subscribe(inc(&c));
    bus.publish(&ping("before"));
    s.dispose();
    bus.publish(&ping("after"));
    assert_eq!(c.load(Ordering::SeqCst), 1);
}

// Subscribe_DisposeOnlyRemovesOwnSubscription
#[test]
fn dispose_removes_only_own() {
    let bus = MessageBusImpl::new();
    let (a, b) = (counter(), counter());
    let s1 = bus.subscribe(inc(&a));
    let _s2 = bus.subscribe(inc(&b));
    s1.dispose();
    bus.publish(&ping("test"));
    assert_eq!(a.load(Ordering::SeqCst), 0);
    assert_eq!(b.load(Ordering::SeqCst), 1);
}

// Subscribe_DisposeCalledTwice_ShouldNotThrow — `dispose(self)` consumes the subscription, so a
// second call cannot compile; the test checks a single dispose of an already-removed type.
#[test]
fn dispose_after_bus_dropped_is_harmless() {
    let bus = MessageBusImpl::new();
    let s = bus.subscribe(|_: &Ping| {});
    drop(bus);
    s.dispose();
}

// Subscribe_Multiple_ThenDisposeAll_ShouldReceiveNothing
#[test]
fn dispose_all_receives_nothing() {
    let bus = MessageBusImpl::new();
    let c = counter();
    let subs: Vec<_> = (0..5).map(|_| bus.subscribe(inc(&c))).collect();
    subs.into_iter().for_each(Subscription::dispose);
    bus.publish(&ping("test"));
    assert_eq!(c.load(Ordering::SeqCst), 0);
}

// Subscribe_TwoDifferentMessageTypes_EachOnlyReceivesOwn
#[test]
fn two_types_each_receive_own() {
    let bus = MessageBusImpl::new();
    let (p, q) = (counter(), counter());
    let _s1 = bus.subscribe(inc(&p));
    let q2 = q.clone();
    let _s2 = bus.subscribe(move |_: &Pong| {
        q2.fetch_add(1, Ordering::SeqCst);
    });
    bus.publish(&ping("ping-only"));
    assert_eq!(p.load(Ordering::SeqCst), 1);
    assert_eq!(q.load(Ordering::SeqCst), 0);
}

// RequestAsync_NoHandler_ShouldReturnNull
#[test]
fn request_without_handler_is_none() {
    let bus = MessageBusImpl::new();
    let r: Option<Pong> = bus.request(&ping("test"), None).unwrap();
    assert!(r.is_none());
}

// RegisterHandler_ThenRequest_ShouldReturnResponse
#[test]
fn register_handler_then_request() {
    let bus = MessageBusImpl::new();
    let _h = bus
        .register_handler(|r: &Ping| Ok(Pong(format!("pong:{}", r.0))))
        .unwrap();
    let r: Option<Pong> = bus.request(&ping("hello"), None).unwrap();
    assert_eq!(r, Some(Pong("pong:hello".into())));
}

// RegisterHandler_ThenDispose_ShouldReturnNull
#[test]
fn register_handler_dispose_then_none() {
    let bus = MessageBusImpl::new();
    let h = bus
        .register_handler(|_: &Ping| Ok(Pong("pong".into())))
        .unwrap();
    h.dispose();
    let r: Option<Pong> = bus.request(&ping("test"), None).unwrap();
    assert!(r.is_none());
}

// RegisterHandler_Timeout_ShouldReturnDefault — handlers are synchronous; a reply slower than
// the timeout is discarded (C# returns `default` too). 5000 ms shortened to 120 ms.
#[test]
fn register_handler_timeout_is_none() {
    let bus = MessageBusImpl::new();
    let _h = bus
        .register_handler(|_: &Ping| {
            std::thread::sleep(Duration::from_millis(120));
            Ok(Pong("too-late".into()))
        })
        .unwrap();
    let r: Option<Pong> = bus
        .request(&ping("test"), Some(Duration::from_millis(50)))
        .unwrap();
    assert!(r.is_none());
}

// RegisterHandler_WithTimeout_ShouldReturnResponse_WhenFast
#[test]
fn register_handler_fast_within_timeout() {
    let bus = MessageBusImpl::new();
    let _h = bus
        .register_handler(|_: &Ping| Ok(Pong("fast".into())))
        .unwrap();
    let r: Option<Pong> = bus
        .request(&ping("test"), Some(Duration::from_secs(5)))
        .unwrap();
    assert_eq!(r, Some(Pong("fast".into())));
}

// PublishAsync_FromMultipleThreadsConcurrently_ShouldNotThrow
#[test]
fn publish_from_many_threads() {
    let bus = MessageBusImpl::new();
    let c = counter();
    let c2 = c.clone();
    let _s = bus.subscribe(move |m: &Counter| {
        c2.fetch_add(m.0, Ordering::SeqCst);
    });
    std::thread::scope(|sc| {
        for _ in 0..50 {
            sc.spawn(|| bus.publish(&Counter(1)));
        }
    });
    assert_eq!(c.load(Ordering::SeqCst), 50);
}

// Subscribe_ConcurrentlyFromMultipleThreads_ShouldNotThrow
#[test]
fn subscribe_from_many_threads() {
    let bus = MessageBusImpl::new();
    std::thread::scope(|sc| {
        for _ in 0..20 {
            sc.spawn(|| bus.subscribe(|_: &Counter| {}));
        }
    });
    assert_eq!(bus.subscriber_count(std::any::TypeId::of::<Counter>()), 20);
}
