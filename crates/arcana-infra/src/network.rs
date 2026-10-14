//! Portable connectivity monitor (replaces C# `NetworkMonitorImpl`).
//!
//! C# subscribes to .NET `NetworkChange` events, which have no portable std equivalent. Here a
//! [`Probe`] is asked periodically whether the network is reachable; the default
//! [`TcpProbe`] opens a TCP connection to well-known endpoints. The probe is injectable so
//! tests (and offline builds) can replace it.

use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use arcana_core::network::{
    NetworkMonitor, NetworkStatusChanged, NetworkStatusHandler, NetworkType,
};
use async_trait::async_trait;

/// One reachability check.
pub trait Probe: Send + Sync {
    /// `true` when the network is reachable right now.
    fn probe(&self) -> bool;
}

/// Reachable when any target accepts a TCP connection within `timeout`.
#[derive(Debug, Clone)]
pub struct TcpProbe {
    pub targets: Vec<String>,
    pub timeout: Duration,
}

impl Default for TcpProbe {
    /// Public DNS resolvers on port 53 (no name lookup needed), 1.5 s timeout.
    fn default() -> Self {
        Self {
            targets: vec!["1.1.1.1:53".to_owned(), "8.8.8.8:53".to_owned()],
            timeout: Duration::from_millis(1500),
        }
    }
}

impl Probe for TcpProbe {
    fn probe(&self) -> bool {
        self.targets.iter().any(|t| {
            let addrs: Vec<SocketAddr> = t
                .to_socket_addrs()
                .map(Iterator::collect)
                .unwrap_or_default();
            addrs
                .iter()
                .any(|a| TcpStream::connect_timeout(a, self.timeout).is_ok())
        })
    }
}

/// Always reports the same state (tests, offline previews).
#[derive(Debug, Clone, Copy)]
pub struct StaticProbe(pub bool);

impl Probe for StaticProbe {
    fn probe(&self) -> bool {
        self.0
    }
}

struct Inner {
    probe: Box<dyn Probe>,
    online: AtomicBool,
    handlers: Mutex<Vec<NetworkStatusHandler>>,
    stop: AtomicBool,
}

impl Inner {
    fn check(&self) -> bool {
        let now = self.probe.probe();
        let before = self.online.swap(now, Ordering::SeqCst);
        if before != now {
            let kind = if now {
                NetworkType::Unknown
            } else {
                NetworkType::None
            };
            let e = NetworkStatusChanged::new(now, kind);
            for h in self
                .handlers
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .iter()
            {
                h(e);
            }
        }
        now
    }
}

/// [`NetworkMonitor`] backed by a periodic [`Probe`] on a background thread.
pub struct ProbeNetworkMonitor {
    inner: Arc<Inner>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl std::fmt::Debug for ProbeNetworkMonitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProbeNetworkMonitor")
            .field("online", &self.inner.online.load(Ordering::SeqCst))
            .finish_non_exhaustive()
    }
}

impl ProbeNetworkMonitor {
    /// Starts optimistic (online) until the first check says otherwise, like C# which reads
    /// `GetIsNetworkAvailable()` lazily.
    #[must_use]
    pub fn new(probe: impl Probe + 'static) -> Self {
        Self {
            inner: Arc::new(Inner {
                probe: Box::new(probe),
                online: AtomicBool::new(true),
                handlers: Mutex::new(Vec::new()),
                stop: AtomicBool::new(false),
            }),
            worker: Mutex::new(None),
        }
    }

    /// Probe now, firing handlers on a change. Returns the new state.
    pub fn check_now(&self) -> bool {
        self.inner.check()
    }

    /// Probe every `interval` on a background thread until [`Self::stop`] or drop.
    ///
    /// # Errors
    /// When the OS refuses to spawn the thread.
    pub fn start(&self, interval: Duration) -> std::io::Result<()> {
        let mut worker = self.worker.lock().unwrap_or_else(PoisonError::into_inner);
        if worker.is_some() {
            return Ok(());
        }
        self.inner.stop.store(false, Ordering::SeqCst);
        let inner = Arc::clone(&self.inner);
        let step = Duration::from_millis(50).min(interval);
        *worker = Some(
            std::thread::Builder::new()
                .name("network-probe".to_owned())
                .spawn(move || {
                    while !inner.stop.load(Ordering::SeqCst) {
                        inner.check();
                        let mut waited = Duration::ZERO;
                        while waited < interval && !inner.stop.load(Ordering::SeqCst) {
                            std::thread::sleep(step);
                            waited += step;
                        }
                    }
                })?,
        );
        Ok(())
    }

    /// Stop the background thread (waits for it to finish).
    pub fn stop(&self) {
        self.inner.stop.store(true, Ordering::SeqCst);
        let handle = self
            .worker
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(h) = handle {
            let _ = h.join();
        }
    }
}

impl Drop for ProbeNetworkMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}

#[async_trait]
impl NetworkMonitor for ProbeNetworkMonitor {
    fn is_online(&self) -> bool {
        self.inner.online.load(Ordering::SeqCst)
    }

    async fn is_online_async(&self) -> bool {
        self.is_online()
    }

    fn on_status_changed(&self, handler: NetworkStatusHandler) {
        self.inner
            .handlers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(handler);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[derive(Clone, Default)]
    struct Switch(Arc<AtomicBool>);
    impl Probe for Switch {
        fn probe(&self) -> bool {
            self.0.load(Ordering::SeqCst)
        }
    }

    #[test]
    fn check_now_fires_handlers_only_on_change() {
        let s = Switch::default();
        let m = ProbeNetworkMonitor::new(s.clone());
        let fired = Arc::new(AtomicUsize::new(0));
        let f = fired.clone();
        m.on_status_changed(Box::new(move |e| {
            assert_eq!(e.network_type == NetworkType::None, !e.is_online);
            f.fetch_add(1, Ordering::SeqCst);
        }));
        assert!(m.is_online());
        assert!(!m.check_now());
        assert!(!m.check_now());
        s.0.store(true, Ordering::SeqCst);
        assert!(m.check_now());
        assert_eq!(fired.load(Ordering::SeqCst), 2);
        assert!(format!("{m:?}").contains("online"));
    }

    #[tokio::test]
    async fn background_thread_picks_up_changes_and_stops() {
        let s = Switch::default();
        let m = ProbeNetworkMonitor::new(s.clone());
        m.start(Duration::from_millis(10)).unwrap();
        m.start(Duration::from_millis(10)).unwrap();
        for _ in 0..200 {
            if !m.is_online_async().await {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!m.is_online());
        m.stop();
        m.stop();
    }

    #[test]
    fn tcp_probe_reports_unreachable_target() {
        // Port 9 on localhost is almost never listening; an invalid target is skipped.
        let p = TcpProbe {
            targets: vec!["127.0.0.1:9".to_owned(), "not a host".to_owned()],
            timeout: Duration::from_millis(200),
        };
        assert!(!p.probe());
        assert_eq!(TcpProbe::default().targets.len(), 2);
    }

    #[test]
    fn tcp_probe_reports_reachable_listener() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let p = TcpProbe {
            targets: vec![l.local_addr().unwrap().to_string()],
            timeout: Duration::from_millis(500),
        };
        assert!(p.probe());
        assert!(StaticProbe(true).probe());
    }
}
