//! Connectivity (C# `NetworkMonitor`).

use async_trait::async_trait;

/// Connection kind (C# `NetworkType`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NetworkType {
    #[default]
    Unknown,
    None,
    Ethernet,
    WiFi,
    Cellular,
}

/// Payload of a status change (C# `NetworkStatusChangedEventArgs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetworkStatusChanged {
    pub is_online: bool,
    pub network_type: NetworkType,
}

impl NetworkStatusChanged {
    /// C# constructor; `network_type` defaults to `Unknown` there.
    #[must_use]
    pub fn new(is_online: bool, network_type: NetworkType) -> Self {
        Self {
            is_online,
            network_type,
        }
    }
}

/// Handler for status changes (the C# `StatusChanged` event).
pub type NetworkStatusHandler = Box<dyn Fn(NetworkStatusChanged) + Send + Sync>;

/// C# `NetworkMonitor`. The C# event becomes a subscription method.
#[async_trait]
pub trait NetworkMonitor: Send + Sync {
    fn is_online(&self) -> bool;
    async fn is_online_async(&self) -> bool;
    fn on_status_changed(&self, handler: NetworkStatusHandler);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[derive(Default)]
    struct Fake {
        online: AtomicBool,
        handlers: Mutex<Vec<NetworkStatusHandler>>,
    }
    impl Fake {
        fn set(&self, online: bool) {
            self.online.store(online, Ordering::SeqCst);
            for h in self.handlers.lock().unwrap().iter() {
                h(NetworkStatusChanged::new(online, NetworkType::WiFi));
            }
        }
    }
    #[async_trait]
    impl NetworkMonitor for Fake {
        fn is_online(&self) -> bool {
            self.online.load(Ordering::SeqCst)
        }
        async fn is_online_async(&self) -> bool {
            self.is_online()
        }
        fn on_status_changed(&self, handler: NetworkStatusHandler) {
            self.handlers.lock().unwrap().push(handler);
        }
    }

    #[tokio::test]
    async fn monitor_is_dyn_compatible_and_notifies() {
        let fake = std::sync::Arc::new(Fake::default());
        let seen = std::sync::Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        let m: &dyn NetworkMonitor = &*fake;
        m.on_status_changed(Box::new(move |e| sink.lock().unwrap().push(e)));
        assert!(!m.is_online_async().await);
        fake.set(true);
        assert!(m.is_online());
        assert_eq!(
            *seen.lock().unwrap(),
            [NetworkStatusChanged::new(true, NetworkType::WiFi)]
        );
        assert_eq!(NetworkType::default(), NetworkType::Unknown);
        let _ = [
            NetworkType::None,
            NetworkType::Ethernet,
            NetworkType::Cellular,
        ];
    }
}
