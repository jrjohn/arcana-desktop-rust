//! C# `EventAggregatorTests` (14), all ported.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use arcana_plugin_api::{ApplicationEvent, ApplicationEventBase, EventAggregatorExt, Subscription};
use arcana_plugin_host::services::event_aggregator::EventAggregatorImpl;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq)]
struct TestEvent {
    base: ApplicationEventBase,
    message: String,
}
#[derive(Debug, Clone)]
struct AnotherEvent {
    base: ApplicationEventBase,
}

impl TestEvent {
    fn new(m: &str) -> Self {
        Self {
            base: ApplicationEventBase::default(),
            message: m.to_string(),
        }
    }
}

impl ApplicationEvent for TestEvent {
    fn timestamp(&self) -> DateTime<Utc> {
        self.base.timestamp
    }
    fn source_plugin_id(&self) -> Option<&str> {
        self.base.source_plugin_id.as_deref()
    }
}
impl ApplicationEvent for AnotherEvent {
    fn timestamp(&self) -> DateTime<Utc> {
        self.base.timestamp
    }
    fn source_plugin_id(&self) -> Option<&str> {
        self.base.source_plugin_id.as_deref()
    }
}

fn counting(c: &Arc<AtomicUsize>) -> impl Fn(&TestEvent) + Send + Sync + 'static {
    let c = c.clone();
    move |_| {
        c.fetch_add(1, Ordering::SeqCst);
    }
}

// Subscribe_ThenPublish_ShouldInvokeHandler
#[test]
fn subscribe_then_publish_invokes() {
    let a = EventAggregatorImpl::new();
    let got = Arc::new(Mutex::new(None));
    let g = got.clone();
    let _s = a.subscribe(move |e: &TestEvent| *g.lock().unwrap() = Some(e.clone()));
    let e = TestEvent::new("Hello");
    a.publish(&e);
    assert_eq!(*got.lock().unwrap(), Some(e));
}

// Publish_NoSubscribers_ShouldNotThrow
#[test]
fn publish_without_subscribers() {
    EventAggregatorImpl::new().publish(&TestEvent::new("ignored"));
}

// Publish_MultipleSubscribers_ShouldInvokeAll
#[test]
fn publish_invokes_all() {
    let a = EventAggregatorImpl::new();
    let log = Arc::new(Mutex::new(Vec::new()));
    let (l1, l2) = (log.clone(), log.clone());
    let _s1 =
        a.subscribe(move |e: &TestEvent| l1.lock().unwrap().push(format!("sub1:{}", e.message)));
    let _s2 =
        a.subscribe(move |e: &TestEvent| l2.lock().unwrap().push(format!("sub2:{}", e.message)));
    a.publish(&TestEvent::new("Hello"));
    let log = log.lock().unwrap();
    assert_eq!(*log, ["sub1:Hello", "sub2:Hello"]);
}

// Subscribe_ReturnsDisposable
#[test]
fn subscribe_returns_subscription() {
    let a = EventAggregatorImpl::new();
    let s: Subscription = a.subscribe(|_: &TestEvent| {});
    s.dispose();
}

// Dispose_Subscription_ShouldUnsubscribe
#[test]
fn dispose_unsubscribes() {
    let a = EventAggregatorImpl::new();
    let c = Arc::new(AtomicUsize::new(0));
    let s = a.subscribe(counting(&c));
    a.publish(&TestEvent::new("first"));
    s.dispose();
    a.publish(&TestEvent::new("second"));
    assert_eq!(c.load(Ordering::SeqCst), 1);
}

// Dispose_CalledTwice_ShouldNotThrow — `dispose(self)` makes a second call unrepresentable;
// disposing after the aggregator is gone must still be harmless.
#[test]
fn dispose_after_aggregator_dropped() {
    let a = EventAggregatorImpl::new();
    let s = a.subscribe(|_: &TestEvent| {});
    drop(a);
    s.dispose();
}

// Dispose_OnlyRemovesSpecificSubscription
#[test]
fn dispose_only_own() {
    let a = EventAggregatorImpl::new();
    let (c1, c2) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
    let s1 = a.subscribe(counting(&c1));
    let _s2 = a.subscribe(counting(&c2));
    s1.dispose();
    a.publish(&TestEvent::new("test"));
    assert_eq!(c1.load(Ordering::SeqCst), 0);
    assert_eq!(c2.load(Ordering::SeqCst), 1);
}

// Publish_DifferentEventTypes_ShouldOnlyTriggerCorrectHandlers
#[test]
fn publish_only_matching_type() {
    let a = EventAggregatorImpl::new();
    let (t, o) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
    let _s1 = a.subscribe(counting(&t));
    let o2 = o.clone();
    let _s2 = a.subscribe(move |_: &AnotherEvent| {
        o2.fetch_add(1, Ordering::SeqCst);
    });
    a.publish(&TestEvent::new("test"));
    assert_eq!(t.load(Ordering::SeqCst), 1);
    assert_eq!(o.load(Ordering::SeqCst), 0);
}

// Publish_HandlerThrows_ShouldContinueWithOtherSubscribers
#[test]
fn panicking_handler_isolated() {
    let a = EventAggregatorImpl::new();
    let c = Arc::new(AtomicUsize::new(0));
    let _s1 = a.subscribe(|_: &TestEvent| panic!("handler failed"));
    let _s2 = a.subscribe(counting(&c));
    a.publish(&TestEvent::new("test"));
    assert_eq!(c.load(Ordering::SeqCst), 1);
}

// Subscribe_AfterFirstPublish_ShouldOnlyReceiveFutureEvents
#[test]
fn only_future_events() {
    let a = EventAggregatorImpl::new();
    let log = Arc::new(Mutex::new(Vec::new()));
    a.publish(&TestEvent::new("before-subscribe"));
    let l = log.clone();
    let _s = a.subscribe(move |e: &TestEvent| l.lock().unwrap().push(e.message.clone()));
    a.publish(&TestEvent::new("after-subscribe"));
    assert_eq!(*log.lock().unwrap(), ["after-subscribe"]);
}

// Publish_ConcurrentSubscribersModification_ShouldNotThrow
#[test]
fn subscribe_from_inside_handler() {
    let a = EventAggregatorImpl::new();
    let inner = a.clone();
    let _s = a.subscribe(move |_: &TestEvent| {
        let _ = inner.subscribe(|_: &TestEvent| {});
    });
    a.publish(&TestEvent::new("concurrent"));
}

// Publish_FromMultipleThreads_ShouldNotThrow
#[test]
fn publish_from_threads() {
    let a = EventAggregatorImpl::new();
    let c = Arc::new(AtomicUsize::new(0));
    let _s = a.subscribe(counting(&c));
    std::thread::scope(|sc| {
        for i in 0..20 {
            let a = &a;
            sc.spawn(move || a.publish(&TestEvent::new(&format!("event-{i}"))));
        }
    });
    assert_eq!(c.load(Ordering::SeqCst), 20);
}

// MultipleSubscribeDispose_ForSameType_ShouldWork
#[test]
fn many_subscribe_dispose() {
    let a = EventAggregatorImpl::new();
    let c = Arc::new(AtomicUsize::new(0));
    let subs: Vec<_> = (0..5).map(|_| a.subscribe(counting(&c))).collect();
    a.publish(&TestEvent::new("test"));
    assert_eq!(c.swap(0, Ordering::SeqCst), 5);
    subs.into_iter().for_each(Subscription::dispose);
    a.publish(&TestEvent::new("test"));
    assert_eq!(c.load(Ordering::SeqCst), 0);
}

// Event_Timestamp_ShouldBeSetApproximatelyToNow
#[test]
fn timestamp_is_now() {
    let before = Utc::now();
    let e = TestEvent::new("ts-test");
    let after = Utc::now();
    assert!(e.timestamp() >= before && e.timestamp() <= after + chrono::Duration::seconds(1));
    assert!(e.source_plugin_id().is_none());
}
