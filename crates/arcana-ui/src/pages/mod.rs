//! Every page: its view model, its async tasks and its view, behind one enum so the shell can
//! host any of them in a tab.

pub mod customer;
pub mod entity_form;
pub mod entity_list;
pub mod flowchart;
pub mod home;
pub mod order_detail;
pub mod order_list;
pub mod plugin_manager;
pub mod product;
pub mod settings;
mod view;

use arcana_domain::{Customer, Product};

use crate::commands::CommandId;
use crate::effect::{DataKind, Effect};
use crate::nav::Route;
use crate::services::UiServices;
use customer::{CustomerForm, CustomerList};
use product::{ProductForm, ProductList};

/// A page instance.
#[derive(Debug)]
pub enum Page {
    Home(home::ViewModel),
    OrderList(order_list::ViewModel),
    OrderDetail(order_detail::ViewModel),
    ProductList(entity_list::ViewModel<ProductList>),
    ProductDetail(entity_form::ViewModel<ProductForm>),
    CustomerList(entity_list::ViewModel<CustomerList>),
    CustomerDetail(entity_form::ViewModel<CustomerForm>),
    Settings(settings::ViewModel),
    PluginManager(plugin_manager::ViewModel),
    FlowChart(Box<flowchart::ViewModel>),
}

/// An input addressed to a page.
#[derive(Debug, Clone, PartialEq)]
pub enum PageMsg {
    Home(home::Input),
    OrderList(order_list::Input),
    OrderDetail(order_detail::Input),
    ProductList(entity_list::Input<Product>),
    ProductDetail(entity_form::Input<Product>),
    CustomerList(entity_list::Input<Customer>),
    CustomerDetail(entity_form::Input<Customer>),
    Settings(settings::Input),
    PluginManager(plugin_manager::Input),
    FlowChart(flowchart::Input),
}

/// An async task requested by a page.
#[derive(Debug, Clone, PartialEq)]
pub enum PageTask {
    Home(home::Task),
    OrderList(order_list::Task),
    OrderDetail(order_detail::Task),
    ProductList(entity_list::Task),
    ProductDetail(entity_form::Task<Product>),
    CustomerList(entity_list::Task),
    CustomerDetail(entity_form::Task<Customer>),
    Settings(settings::Task),
    PluginManager(plugin_manager::Task),
    FlowChart(flowchart::Task),
}

fn lift<T>(fx: Vec<Effect<T>>, f: impl Fn(T) -> PageTask) -> Vec<Effect<PageTask>> {
    fx.into_iter().map(|e| e.map(&f)).collect()
}

impl Page {
    /// Create the page for `route` with its initial effects.
    #[must_use]
    pub fn open(route: &Route, settings: &settings::Snapshot) -> (Self, Vec<Effect<PageTask>>) {
        match route {
            Route::Home => {
                let (vm, fx) = home::ViewModel::new();
                (Self::Home(vm), lift(fx, PageTask::Home))
            }
            Route::OrderList => {
                let (vm, fx) = order_list::ViewModel::new();
                (Self::OrderList(vm), lift(fx, PageTask::OrderList))
            }
            Route::OrderDetail { id, read_only } => {
                let (vm, fx) = order_detail::ViewModel::new(*id, *read_only);
                (Self::OrderDetail(vm), lift(fx, PageTask::OrderDetail))
            }
            Route::ProductList => {
                let (vm, fx) = entity_list::ViewModel::new();
                (Self::ProductList(vm), lift(fx, PageTask::ProductList))
            }
            Route::ProductDetail { id } => {
                let (vm, fx) = entity_form::ViewModel::new(*id);
                (Self::ProductDetail(vm), lift(fx, PageTask::ProductDetail))
            }
            Route::CustomerList => {
                let (vm, fx) = entity_list::ViewModel::new();
                (Self::CustomerList(vm), lift(fx, PageTask::CustomerList))
            }
            Route::CustomerDetail { id } => {
                let (vm, fx) = entity_form::ViewModel::new(*id);
                (Self::CustomerDetail(vm), lift(fx, PageTask::CustomerDetail))
            }
            Route::Settings => (
                Self::Settings(settings::ViewModel::new(settings.clone())),
                vec![],
            ),
            Route::PluginManager => {
                let (vm, fx) = plugin_manager::ViewModel::new();
                (Self::PluginManager(vm), lift(fx, PageTask::PluginManager))
            }
            Route::FlowChart { mode } => {
                let (vm, fx) = flowchart::ViewModel::new(mode);
                (Self::FlowChart(Box::new(vm)), lift(fx, PageTask::FlowChart))
            }
        }
    }

    /// The route this page currently represents (a saved new order becomes its id).
    #[must_use]
    pub fn route(&self) -> Route {
        match self {
            Self::Home(_) => Route::Home,
            Self::OrderList(_) => Route::OrderList,
            Self::OrderDetail(vm) => Route::OrderDetail {
                id: vm.state.order_id,
                read_only: vm.state.read_only,
            },
            Self::ProductList(_) => Route::ProductList,
            Self::ProductDetail(vm) => Route::ProductDetail { id: vm.state.id },
            Self::CustomerList(_) => Route::CustomerList,
            Self::CustomerDetail(vm) => Route::CustomerDetail { id: vm.state.id },
            Self::Settings(_) => Route::Settings,
            Self::PluginManager(_) => Route::PluginManager,
            Self::FlowChart(_) => Route::FlowChart {
                mode: crate::nav::FlowMode::New,
            },
        }
    }

    /// Feed an input; a message for another page type is ignored.
    pub fn on(&mut self, msg: PageMsg) -> Vec<Effect<PageTask>> {
        match (self, msg) {
            (Self::Home(vm), PageMsg::Home(i)) => lift(vm.on(i), PageTask::Home),
            (Self::OrderList(vm), PageMsg::OrderList(i)) => lift(vm.on(i), PageTask::OrderList),
            (Self::OrderDetail(vm), PageMsg::OrderDetail(i)) => {
                lift(vm.on(i), PageTask::OrderDetail)
            }
            (Self::ProductList(vm), PageMsg::ProductList(i)) => {
                lift(vm.on(i), PageTask::ProductList)
            }
            (Self::ProductDetail(vm), PageMsg::ProductDetail(i)) => {
                lift(vm.on(i), PageTask::ProductDetail)
            }
            (Self::CustomerList(vm), PageMsg::CustomerList(i)) => {
                lift(vm.on(i), PageTask::CustomerList)
            }
            (Self::CustomerDetail(vm), PageMsg::CustomerDetail(i)) => {
                lift(vm.on(i), PageTask::CustomerDetail)
            }
            (Self::Settings(vm), PageMsg::Settings(i)) => lift(vm.on(i), PageTask::Settings),
            (Self::PluginManager(vm), PageMsg::PluginManager(i)) => {
                lift(vm.on(i), PageTask::PluginManager)
            }
            (Self::FlowChart(vm), PageMsg::FlowChart(i)) => lift(vm.on(i), PageTask::FlowChart),
            _ => vec![],
        }
    }

    /// Unsaved changes (closing asks first).
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        match self {
            Self::OrderDetail(vm) => vm.state.is_dirty,
            Self::ProductDetail(vm) => vm.state.is_dirty,
            Self::CustomerDetail(vm) => vm.state.is_dirty,
            Self::FlowChart(vm) => vm.editor.state.modified,
            _ => false,
        }
    }

    /// The message a page-level command maps to (Save, Refresh), if this page supports it.
    #[must_use]
    pub fn command(&self, cmd: CommandId) -> Option<PageMsg> {
        Some(match (self, cmd) {
            (Self::OrderDetail(_), CommandId::Save) => {
                PageMsg::OrderDetail(order_detail::Input::Save)
            }
            (Self::ProductDetail(_), CommandId::Save) => {
                PageMsg::ProductDetail(entity_form::Input::Save)
            }
            (Self::CustomerDetail(_), CommandId::Save) => {
                PageMsg::CustomerDetail(entity_form::Input::Save)
            }
            (Self::Home(_), CommandId::Refresh) => PageMsg::Home(home::Input::Load),
            (Self::OrderList(_), CommandId::Refresh) => {
                PageMsg::OrderList(order_list::Input::Refresh)
            }
            (Self::ProductList(_), CommandId::Refresh) => {
                PageMsg::ProductList(entity_list::Input::Refresh)
            }
            (Self::CustomerList(_), CommandId::Refresh) => {
                PageMsg::CustomerList(entity_list::Input::Refresh)
            }
            (Self::PluginManager(_), CommandId::Refresh) => {
                PageMsg::PluginManager(plugin_manager::Input::Refresh)
            }
            _ => return None,
        })
    }

    /// The reply to a confirmed dialog.
    #[must_use]
    pub fn confirm_delete(&self, id: i32) -> Option<PageMsg> {
        Some(match self {
            Self::OrderList(_) => PageMsg::OrderList(order_list::Input::DeleteConfirmed(id)),
            Self::ProductList(_) => PageMsg::ProductList(entity_list::Input::DeleteConfirmed(id)),
            Self::CustomerList(_) => PageMsg::CustomerList(entity_list::Input::DeleteConfirmed(id)),
            Self::ProductDetail(_) => {
                PageMsg::ProductDetail(entity_form::Input::DeleteConfirmed(id))
            }
            Self::CustomerDetail(_) => {
                PageMsg::CustomerDetail(entity_form::Input::DeleteConfirmed(id))
            }
            _ => return None,
        })
    }

    /// The reply to a confirmed "discard changes".
    #[must_use]
    pub fn confirm_discard(&self) -> Option<PageMsg> {
        Some(match self {
            Self::OrderDetail(_) => PageMsg::OrderDetail(order_detail::Input::DiscardConfirmed),
            Self::ProductDetail(_) => PageMsg::ProductDetail(entity_form::Input::DiscardConfirmed),
            Self::CustomerDetail(_) => {
                PageMsg::CustomerDetail(entity_form::Input::DiscardConfirmed)
            }
            _ => return None,
        })
    }

    /// Refresh when data shown by this page changed elsewhere.
    #[must_use]
    pub fn on_data_changed(&self, kind: DataKind) -> Option<PageMsg> {
        Some(match (self, kind) {
            (Self::Home(_), DataKind::Orders | DataKind::Customers) => {
                PageMsg::Home(home::Input::Load)
            }
            (Self::OrderList(_), DataKind::Orders) => {
                PageMsg::OrderList(order_list::Input::Refresh)
            }
            (Self::ProductList(_), DataKind::Products) => {
                PageMsg::ProductList(entity_list::Input::Refresh)
            }
            (Self::CustomerList(_), DataKind::Customers) => {
                PageMsg::CustomerList(entity_list::Input::Refresh)
            }
            _ => return None,
        })
    }
}

/// Execute a page task on the runtime.
pub async fn run_task(task: PageTask, svc: UiServices) -> PageMsg {
    match task {
        PageTask::Home(t) => PageMsg::Home(home::run(t, svc).await),
        PageTask::OrderList(t) => PageMsg::OrderList(order_list::run(t, svc).await),
        PageTask::OrderDetail(t) => PageMsg::OrderDetail(order_detail::run(t, svc).await),
        PageTask::ProductList(t) => PageMsg::ProductList(product::run_list(t, svc).await),
        PageTask::ProductDetail(t) => PageMsg::ProductDetail(product::run_form(t, svc).await),
        PageTask::CustomerList(t) => PageMsg::CustomerList(customer::run_list(t, svc).await),
        PageTask::CustomerDetail(t) => PageMsg::CustomerDetail(customer::run_form(t, svc).await),
        PageTask::Settings(t) => PageMsg::Settings(settings::run(t, svc).await),
        PageTask::PluginManager(t) => PageMsg::PluginManager(plugin_manager::run(t, svc).await),
        PageTask::FlowChart(t) => PageMsg::FlowChart(flowchart::run(t, svc).await),
    }
}
