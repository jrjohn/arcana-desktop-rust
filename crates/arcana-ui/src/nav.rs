//! Type-safe navigation (C# `NavGraph`).
//!
//! Route ids are the C# `NavGraph.Routes` constants. C# navigation actions all call
//! `NavigateToNewTabAsync`; here each action returns a [`NavRequest`] that the shell
//! executes, so view models stay pure.

/// A navigation destination with its arguments.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Route {
    /// Home / dashboard (C# `HomePage`, `Routes.Dashboard`).
    Home,
    OrderList,
    /// `id = None` is a new order (C# `OrderDetailArgs(OrderId, ReadOnly)`).
    OrderDetail {
        id: Option<i32>,
        read_only: bool,
    },
    ProductList,
    ProductDetail {
        id: Option<i32>,
    },
    CustomerList,
    CustomerDetail {
        id: Option<i32>,
    },
    PluginManager,
    Settings,
    /// `FlowChart` editor (C# `FlowChartEditorPage`, contributed by the `FlowChart` plugin).
    FlowChart {
        mode: FlowMode,
    },
}

/// C# `EditorArgs` of the `FlowChart` editor route.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FlowMode {
    New,
    Sample,
    OpenDialog,
    Open(String),
}

/// C# `NavGraph.Routes` constants.
pub mod routes {
    pub const ORDER_LIST: &str = "OrderListPage";
    pub const ORDER_DETAIL: &str = "OrderDetailPage";
    pub const PRODUCT_LIST: &str = "ProductListPage";
    pub const PRODUCT_DETAIL: &str = "ProductDetailPage";
    pub const CUSTOMER_LIST: &str = "CustomerListPage";
    pub const CUSTOMER_DETAIL: &str = "CustomerDetailPage";
    pub const PLUGIN_MANAGER: &str = "PluginManagerPage";
    pub const SETTINGS: &str = "SettingsPage";
    pub const FLOWCHART: &str = "FlowChartEditorPage";
    pub const DASHBOARD: &str = "DashboardPage";
    /// The side-nav tag of the home item (C# `MainWindow.xaml`).
    pub const HOME: &str = "HomePage";
}

impl Route {
    /// The C# route id.
    #[must_use]
    pub fn id(&self) -> &'static str {
        match self {
            Self::Home => routes::DASHBOARD,
            Self::OrderList => routes::ORDER_LIST,
            Self::OrderDetail { .. } => routes::ORDER_DETAIL,
            Self::ProductList => routes::PRODUCT_LIST,
            Self::ProductDetail { .. } => routes::PRODUCT_DETAIL,
            Self::CustomerList => routes::CUSTOMER_LIST,
            Self::CustomerDetail { .. } => routes::CUSTOMER_DETAIL,
            Self::PluginManager => routes::PLUGIN_MANAGER,
            Self::Settings => routes::SETTINGS,
            Self::FlowChart { .. } => routes::FLOWCHART,
        }
    }

    /// Parse a route id without arguments (`NavGraph.To(routeId)`); detail routes open in
    /// "new" mode. Unknown ids (e.g. the C# `SalesReportPage` that has no page) give `None`.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            routes::DASHBOARD | routes::HOME => Self::Home,
            routes::ORDER_LIST => Self::OrderList,
            routes::ORDER_DETAIL => Self::OrderDetail {
                id: None,
                read_only: false,
            },
            routes::PRODUCT_LIST => Self::ProductList,
            routes::PRODUCT_DETAIL => Self::ProductDetail { id: None },
            routes::CUSTOMER_LIST => Self::CustomerList,
            routes::CUSTOMER_DETAIL => Self::CustomerDetail { id: None },
            routes::PLUGIN_MANAGER => Self::PluginManager,
            routes::SETTINGS => Self::Settings,
            routes::FLOWCHART => Self::FlowChart {
                mode: FlowMode::New,
            },
            _ => return None,
        })
    }

    /// Whether opening `other` should focus the tab already showing `self` instead of
    /// opening a second one: singleton pages match by route, details by entity id. A "new"
    /// detail never matches (each gets its own tab).
    #[must_use]
    pub fn same_document(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::OrderDetail { id: Some(a), .. }, Self::OrderDetail { id: Some(b), .. })
            | (Self::ProductDetail { id: Some(a) }, Self::ProductDetail { id: Some(b) })
            | (Self::CustomerDetail { id: Some(a) }, Self::CustomerDetail { id: Some(b) }) => {
                a == b
            }
            (
                Self::OrderDetail { .. }
                | Self::ProductDetail { .. }
                | Self::CustomerDetail { .. }
                | Self::FlowChart { .. },
                _,
            ) => false,
            (a, b) => a == b,
        }
    }

    /// Side-nav item this route highlights (lists and their details share one).
    #[must_use]
    pub fn nav_section(&self) -> &'static str {
        match self {
            Self::Home => routes::HOME,
            Self::OrderList | Self::OrderDetail { .. } => routes::ORDER_LIST,
            Self::ProductList | Self::ProductDetail { .. } => routes::PRODUCT_LIST,
            Self::CustomerList | Self::CustomerDetail { .. } => routes::CUSTOMER_LIST,
            Self::PluginManager => routes::PLUGIN_MANAGER,
            Self::Settings => routes::SETTINGS,
            Self::FlowChart { .. } => routes::FLOWCHART,
        }
    }
}

/// Where a navigation lands (C# `NavigateToNewTabAsync` / `NavigateWithinTabAsync`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavMode {
    /// Open (or focus) a document tab.
    NewTab,
    /// Replace the current tab's page, keeping a back stack.
    WithinTab,
}

/// One navigation request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavRequest {
    pub route: Route,
    pub mode: NavMode,
}

impl NavRequest {
    #[must_use]
    pub fn new_tab(route: Route) -> Self {
        Self {
            route,
            mode: NavMode::NewTab,
        }
    }

    #[must_use]
    pub fn within_tab(route: Route) -> Self {
        Self {
            route,
            mode: NavMode::WithinTab,
        }
    }
}

/// C# `NavGraph` navigation actions. Every C# action opens a new tab.
#[derive(Debug, Clone, Copy, Default)]
pub struct NavGraph;

impl NavGraph {
    #[must_use]
    pub fn to_order_list() -> NavRequest {
        NavRequest::new_tab(Route::OrderList)
    }
    #[must_use]
    pub fn to_order_detail(order_id: i32, read_only: bool) -> NavRequest {
        NavRequest::new_tab(Route::OrderDetail {
            id: Some(order_id),
            read_only,
        })
    }
    #[must_use]
    pub fn to_new_order() -> NavRequest {
        NavRequest::new_tab(Route::OrderDetail {
            id: None,
            read_only: false,
        })
    }
    #[must_use]
    pub fn to_product_list() -> NavRequest {
        NavRequest::new_tab(Route::ProductList)
    }
    #[must_use]
    pub fn to_product_detail(product_id: i32) -> NavRequest {
        NavRequest::new_tab(Route::ProductDetail {
            id: Some(product_id),
        })
    }
    #[must_use]
    pub fn to_new_product() -> NavRequest {
        NavRequest::new_tab(Route::ProductDetail { id: None })
    }
    #[must_use]
    pub fn to_customer_list() -> NavRequest {
        NavRequest::new_tab(Route::CustomerList)
    }
    #[must_use]
    pub fn to_customer_detail(customer_id: i32) -> NavRequest {
        NavRequest::new_tab(Route::CustomerDetail {
            id: Some(customer_id),
        })
    }
    #[must_use]
    pub fn to_new_customer() -> NavRequest {
        NavRequest::new_tab(Route::CustomerDetail { id: None })
    }
    #[must_use]
    pub fn to_plugin_manager() -> NavRequest {
        NavRequest::new_tab(Route::PluginManager)
    }
    #[must_use]
    pub fn to_settings() -> NavRequest {
        NavRequest::new_tab(Route::Settings)
    }
    #[must_use]
    pub fn to_dashboard() -> NavRequest {
        NavRequest::new_tab(Route::Home)
    }
    /// C# `NavGraph.To(routeId)`: `None` for an unknown route id.
    #[must_use]
    pub fn to(route_id: &str) -> Option<NavRequest> {
        Route::from_id(route_id).map(NavRequest::new_tab)
    }
    /// C# `NavGraph.ToWithinTab`.
    #[must_use]
    pub fn to_within_tab(route_id: &str) -> Option<NavRequest> {
        Route::from_id(route_id).map(NavRequest::within_tab)
    }
}

/// A navigation request a plugin queued, in the UI's own terms. The shell only ever sees this
/// type; the translation from the plugin host's queue type happens here, in the view model.
#[derive(Debug)]
pub enum PluginNavRequest {
    /// Open `route_id` (plugin `NavigateTo` / `NavigateToNewTab` — both open a tab here).
    Open {
        route_id: String,
        parameter: Option<Box<dyn std::any::Any + Send>>,
    },
    /// Navigate inside the focused tab.
    WithinTab {
        route_id: String,
        parameter: Option<Box<dyn std::any::Any + Send>>,
    },
    Back,
    Forward,
    Close,
}

impl From<arcana_plugin_host::services::nav::NavRequest> for PluginNavRequest {
    fn from(req: arcana_plugin_host::services::nav::NavRequest) -> Self {
        use arcana_plugin_host::services::nav::NavRequest as Host;
        match req {
            Host::To {
                route_id,
                parameter,
            }
            | Host::NewTab {
                route_id,
                parameter,
            } => Self::Open {
                route_id,
                parameter,
            },
            Host::WithinTab {
                route_id,
                parameter,
                ..
            } => Self::WithinTab {
                route_id,
                parameter,
            },
            Host::Back => Self::Back,
            Host::Forward => Self::Forward,
            Host::Close => Self::Close,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_ids_match_csharp_nav_graph_routes() {
        assert_eq!(routes::ORDER_LIST, "OrderListPage");
        assert_eq!(routes::ORDER_DETAIL, "OrderDetailPage");
        assert_eq!(routes::PRODUCT_LIST, "ProductListPage");
        assert_eq!(routes::PRODUCT_DETAIL, "ProductDetailPage");
        assert_eq!(routes::CUSTOMER_LIST, "CustomerListPage");
        assert_eq!(routes::CUSTOMER_DETAIL, "CustomerDetailPage");
        assert_eq!(routes::PLUGIN_MANAGER, "PluginManagerPage");
        assert_eq!(routes::SETTINGS, "SettingsPage");
        assert_eq!(routes::DASHBOARD, "DashboardPage");
    }

    #[test]
    fn every_route_round_trips_through_its_id() {
        let all = [
            Route::Home,
            Route::OrderList,
            Route::OrderDetail {
                id: None,
                read_only: false,
            },
            Route::ProductList,
            Route::ProductDetail { id: None },
            Route::CustomerList,
            Route::CustomerDetail { id: None },
            Route::PluginManager,
            Route::Settings,
            Route::FlowChart {
                mode: FlowMode::New,
            },
        ];
        for r in all {
            assert_eq!(Route::from_id(r.id()), Some(r.clone()), "{r:?}");
            assert_ne!(r.nav_section().len(), 0);
        }
        assert_eq!(Route::from_id(routes::HOME), Some(Route::Home));
        assert_eq!(Route::from_id("SalesReportPage"), None);
    }

    #[test]
    fn actions_open_new_tabs_with_csharp_arguments() {
        assert_eq!(
            NavGraph::to_order_detail(10, true),
            NavRequest::new_tab(Route::OrderDetail {
                id: Some(10),
                read_only: true
            })
        );
        assert_eq!(
            NavGraph::to_new_order().route,
            Route::OrderDetail {
                id: None,
                read_only: false
            }
        );
        let all = [
            NavGraph::to_order_list(),
            NavGraph::to_product_list(),
            NavGraph::to_product_detail(1),
            NavGraph::to_new_product(),
            NavGraph::to_customer_list(),
            NavGraph::to_customer_detail(2),
            NavGraph::to_new_customer(),
            NavGraph::to_plugin_manager(),
            NavGraph::to_settings(),
            NavGraph::to_dashboard(),
        ];
        assert!(all.iter().all(|r| r.mode == NavMode::NewTab));
        assert_eq!(NavGraph::to("SettingsPage"), Some(NavGraph::to_settings()));
        assert_eq!(NavGraph::to("nope"), None);
        assert_eq!(
            NavGraph::to_within_tab("OrderListPage").map(|r| r.mode),
            Some(NavMode::WithinTab)
        );
    }

    #[test]
    fn same_document_rules() {
        let d = |id| Route::OrderDetail {
            id,
            read_only: false,
        };
        assert!(Route::OrderList.same_document(&Route::OrderList));
        assert!(!Route::OrderList.same_document(&Route::ProductList));
        assert!(d(Some(1)).same_document(&Route::OrderDetail {
            id: Some(1),
            read_only: true
        }));
        assert!(!d(Some(1)).same_document(&d(Some(2))));
        assert!(!d(None).same_document(&d(None)));
        assert!(!d(Some(1)).same_document(&Route::OrderList));
        assert!(
            Route::ProductDetail { id: Some(3) }
                .same_document(&Route::ProductDetail { id: Some(3) })
        );
        assert!(
            !Route::CustomerDetail { id: None }.same_document(&Route::CustomerDetail { id: None })
        );
        assert_eq!(d(Some(1)).nav_section(), routes::ORDER_LIST);
    }

    #[test]
    fn plugin_nav_requests_translate_to_ui_terms() {
        use arcana_plugin_host::services::nav::NavRequest as Host;
        let open = |r| matches!(PluginNavRequest::from(r), PluginNavRequest::Open { route_id, .. } if route_id == "X");
        assert!(open(Host::To {
            route_id: "X".into(),
            parameter: None
        }));
        assert!(open(Host::NewTab {
            route_id: "X".into(),
            parameter: None
        }));
        assert!(matches!(
            PluginNavRequest::from(Host::WithinTab {
                parent_route_id: "P".into(),
                route_id: "X".into(),
                parameter: None
            }),
            PluginNavRequest::WithinTab { route_id, .. } if route_id == "X"
        ));
        assert!(matches!(
            PluginNavRequest::from(Host::Back),
            PluginNavRequest::Back
        ));
        assert!(matches!(
            PluginNavRequest::from(Host::Forward),
            PluginNavRequest::Forward
        ));
        assert!(matches!(
            PluginNavRequest::from(Host::Close),
            PluginNavRequest::Close
        ));
    }
}
