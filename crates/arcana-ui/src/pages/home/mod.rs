//! Home / dashboard (C# `HomePage`).
//!
//! C# fills the four cards with hard-coded numbers (`12`, `$45,600`, `5`, `128`) and never
//! loads recent orders; here they are computed from the order and customer services.

mod task;
mod view;

pub use task::run;
pub use view::show;

use arcana_domain::Order;
use rust_decimal::Decimal;

use crate::effect::{Effect, Notice};
use crate::nav::NavGraph;

/// Dashboard figures.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Dashboard {
    pub today_orders: usize,
    pub today_revenue: Decimal,
    pub pending_orders: usize,
    pub total_customers: u64,
    /// Newest first, at most [`RECENT_COUNT`].
    pub recent: Vec<Order>,
}

/// How many recent orders the card lists.
pub const RECENT_COUNT: u32 = 5;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    pub dashboard: Dashboard,
    pub loading: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    Load,
    Loaded(Result<Dashboard, String>),
    NewOrder,
    NewCustomer,
    NewProduct,
    ViewReports,
    ViewAllOrders,
    OpenOrder(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Task {
    LoadDashboard,
}

#[derive(Debug, Default)]
pub struct ViewModel {
    pub state: State,
}

impl ViewModel {
    /// New view model plus its initial effects (load).
    #[must_use]
    pub fn new() -> (Self, Vec<Effect<Task>>) {
        let mut vm = Self::default();
        let fx = vm.on(Input::Load);
        (vm, fx)
    }

    pub fn on(&mut self, input: Input) -> Vec<Effect<Task>> {
        let s = &mut self.state;
        match input {
            Input::Load => {
                if s.loading {
                    return vec![];
                }
                s.loading = true;
                s.error = None;
                vec![Effect::Run(Task::LoadDashboard)]
            }
            Input::Loaded(Ok(d)) => {
                s.loading = false;
                s.dashboard = d;
                vec![]
            }
            Input::Loaded(Err(e)) => {
                s.loading = false;
                s.error = Some(e.clone());
                vec![Effect::Notify(Notice::error(e))]
            }
            Input::NewOrder => vec![Effect::Navigate(NavGraph::to_new_order())],
            Input::NewCustomer => vec![Effect::Navigate(NavGraph::to_new_customer())],
            Input::NewProduct => vec![Effect::Navigate(NavGraph::to_new_product())],
            // C# has no reports page; the button points at the order list it summarises.
            Input::ViewReports | Input::ViewAllOrders => {
                vec![Effect::Navigate(NavGraph::to_order_list())]
            }
            Input::OpenOrder(id) => vec![Effect::Navigate(NavGraph::to_order_detail(id, false))],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nav::Route;

    #[test]
    fn load_runs_once_and_result_lands_in_state() {
        let (mut vm, fx) = ViewModel::new();
        assert_eq!(fx, vec![Effect::Run(Task::LoadDashboard)]);
        assert!(vm.state.loading);
        assert!(vm.on(Input::Load).is_empty(), "no double load while busy");
        let d = Dashboard {
            today_orders: 2,
            pending_orders: 1,
            total_customers: 5,
            ..Dashboard::default()
        };
        assert!(vm.on(Input::Loaded(Ok(d.clone()))).is_empty());
        assert_eq!(vm.state.dashboard, d);
        assert!(!vm.state.loading);
    }

    #[test]
    fn load_error_notifies() {
        let (mut vm, _) = ViewModel::new();
        let fx = vm.on(Input::Loaded(Err("boom".into())));
        assert_eq!(fx, vec![Effect::Notify(Notice::error("boom"))]);
        assert_eq!(vm.state.error.as_deref(), Some("boom"));
    }

    #[test]
    fn quick_actions_navigate() {
        let mut vm = ViewModel::default();
        let nav = |fx: Vec<Effect<Task>>| match fx.as_slice() {
            [Effect::Navigate(r)] => r.route.clone(),
            other => panic!("{other:?}"),
        };
        assert_eq!(
            nav(vm.on(Input::NewOrder)),
            Route::OrderDetail {
                id: None,
                read_only: false
            }
        );
        assert_eq!(
            nav(vm.on(Input::NewCustomer)),
            Route::CustomerDetail { id: None }
        );
        assert_eq!(
            nav(vm.on(Input::NewProduct)),
            Route::ProductDetail { id: None }
        );
        assert_eq!(nav(vm.on(Input::ViewReports)), Route::OrderList);
        assert_eq!(nav(vm.on(Input::ViewAllOrders)), Route::OrderList);
        assert_eq!(
            nav(vm.on(Input::OpenOrder(7))),
            Route::OrderDetail {
                id: Some(7),
                read_only: false
            }
        );
    }
}
