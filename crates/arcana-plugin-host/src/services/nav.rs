//! Queue-backed `NavGraph`: validates the route, enqueues the request, the shell drains it.

use std::any::Any;
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use arcana_plugin_api::NavGraph;

/// One queued navigation.
#[derive(Debug)]
pub enum NavRequest {
    To {
        route_id: String,
        parameter: Option<Box<dyn Any + Send>>,
    },
    NewTab {
        route_id: String,
        parameter: Option<Box<dyn Any + Send>>,
    },
    WithinTab {
        parent_route_id: String,
        route_id: String,
        parameter: Option<Box<dyn Any + Send>>,
    },
    Back,
    Forward,
    Close,
}

#[derive(Default)]
struct Inner {
    routes: BTreeSet<String>,
    queue: Vec<NavRequest>,
    back: Vec<String>,
    forward: Vec<String>,
    current: Option<String>,
}

/// The shell reports actual history with [`QueuedNavGraph::set_history`]; plugins only see
/// what the last frame reported.
#[derive(Clone, Default)]
pub struct QueuedNavGraph {
    inner: Arc<Mutex<Inner>>,
}

impl std::fmt::Debug for QueuedNavGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("QueuedNavGraph")
    }
}

impl QueuedNavGraph {
    #[must_use]
    pub fn new(routes: impl IntoIterator<Item = String>) -> Self {
        let g = Self::default();
        g.lock().routes.extend(routes);
        g
    }
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
    pub fn add_route(&self, route_id: &str) {
        self.lock().routes.insert(route_id.to_string());
    }
    pub fn remove_route(&self, route_id: &str) {
        self.lock().routes.remove(route_id);
    }
    #[must_use]
    pub fn drain(&self) -> Vec<NavRequest> {
        std::mem::take(&mut self.lock().queue)
    }
    pub fn set_history(&self, current: Option<&str>, back: Vec<String>, forward: Vec<String>) {
        let mut g = self.lock();
        g.current = current.map(str::to_string);
        g.back = back;
        g.forward = forward;
    }
    fn push_if_known(&self, route_id: &str, req: NavRequest) -> bool {
        let mut g = self.lock();
        if !g.routes.contains(route_id) {
            return false;
        }
        g.queue.push(req);
        true
    }
}

impl NavGraph for QueuedNavGraph {
    fn to(&self, route_id: &str, parameter: Option<Box<dyn Any + Send>>) -> bool {
        self.push_if_known(
            route_id,
            NavRequest::To {
                route_id: route_id.to_string(),
                parameter,
            },
        )
    }
    fn to_new_tab(&self, route_id: &str, parameter: Option<Box<dyn Any + Send>>) -> bool {
        self.push_if_known(
            route_id,
            NavRequest::NewTab {
                route_id: route_id.to_string(),
                parameter,
            },
        )
    }
    fn to_within_tab(
        &self,
        parent_route_id: &str,
        route_id: &str,
        parameter: Option<Box<dyn Any + Send>>,
    ) -> bool {
        self.push_if_known(
            route_id,
            NavRequest::WithinTab {
                parent_route_id: parent_route_id.to_string(),
                route_id: route_id.to_string(),
                parameter,
            },
        )
    }
    fn back(&self) -> bool {
        let mut g = self.lock();
        if g.back.is_empty() {
            return false;
        }
        g.queue.push(NavRequest::Back);
        true
    }
    fn forward(&self) -> bool {
        let mut g = self.lock();
        if g.forward.is_empty() {
            return false;
        }
        g.queue.push(NavRequest::Forward);
        true
    }
    fn close(&self) {
        self.lock().queue.push(NavRequest::Close);
    }
    fn can_go_back(&self) -> bool {
        !self.lock().back.is_empty()
    }
    fn can_go_forward(&self) -> bool {
        !self.lock().forward.is_empty()
    }
    fn current_route_id(&self) -> Option<String> {
        self.lock().current.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_route_is_rejected_known_is_queued() {
        let n = QueuedNavGraph::new(["OrderList".to_string()]);
        assert!(!n.to("Nope", None));
        assert!(n.to_new_tab("OrderList", Some(Box::new(5_i32))));
        assert!(!n.back());
        n.set_history(Some("OrderList"), vec!["Home".into()], vec![]);
        assert!(n.can_go_back() && !n.can_go_forward());
        assert!(n.back());
        n.close();
        let d = n.drain();
        assert_eq!(d.len(), 3);
        assert!(matches!(&d[0], NavRequest::NewTab { route_id, .. } if route_id == "OrderList"));
        assert_eq!(n.current_route_id().as_deref(), Some("OrderList"));
    }
}
