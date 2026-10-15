//! `INavGraph` — route-based navigation exposed to plugins.

use std::any::Any;

/// C# `NavigationType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationType {
    Forward,
    Back,
    NewTab,
    Replace,
}

/// C# `INavGraph`. Calls only enqueue the navigation; the shell performs it on its next frame,
/// so the `bool` means "route known and request queued".
pub trait NavGraph: Send + Sync {
    fn to(&self, route_id: &str, parameter: Option<Box<dyn Any + Send>>) -> bool;
    fn to_new_tab(&self, route_id: &str, parameter: Option<Box<dyn Any + Send>>) -> bool;
    fn to_within_tab(
        &self,
        parent_route_id: &str,
        route_id: &str,
        parameter: Option<Box<dyn Any + Send>>,
    ) -> bool;
    fn back(&self) -> bool;
    fn forward(&self) -> bool;
    fn close(&self);
    fn can_go_back(&self) -> bool;
    fn can_go_forward(&self) -> bool;
    fn current_route_id(&self) -> Option<String>;
}
