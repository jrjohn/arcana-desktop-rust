//! The view side of [`Page`]: its tab title and dispatching `show` to the page's view.

use super::customer::{CustomerForm, CustomerList};
use super::product::{ProductForm, ProductList};
use super::{Page, PageMsg, entity_form, entity_list, flowchart, home, order_detail, order_list};
use super::{plugin_manager, settings};
use crate::widgets::ViewCx;

impl Page {
    /// Tab title.
    #[must_use]
    pub fn title(&self, cx: &ViewCx<'_>) -> String {
        match self {
            Self::Home(_) => cx.t("nav.home"),
            Self::OrderList(_) => cx.t("nav.orders"),
            Self::OrderDetail(vm) if vm.state.is_new => cx.t("order.new"),
            Self::OrderDetail(vm) => format!(
                "{} #{}",
                cx.t("order.title"),
                vm.state.order_id.unwrap_or_default()
            ),
            Self::ProductList(_) => cx.t("nav.products"),
            Self::ProductDetail(vm) if vm.state.is_new => cx.t("quick.newProduct"),
            Self::ProductDetail(vm) => {
                format!("{} {}", cx.t("product.title"), vm.state.entity.code)
            }
            Self::CustomerList(_) => cx.t("nav.customers"),
            Self::CustomerDetail(vm) if vm.state.is_new => cx.t("quick.newCustomer"),
            Self::CustomerDetail(vm) => {
                format!("{} {}", cx.t("customer.title"), vm.state.entity.code)
            }
            Self::Settings(_) => cx.t("nav.settings"),
            Self::PluginManager(_) => cx.t("nav.plugins"),
            Self::FlowChart(vm) => {
                let title = vm.editor.state.diagram.name.clone();
                format!("{} — {title}", cx.t("flowchart.editor"))
            }
        }
    }

    /// Draw the page; returns the inputs the user produced.
    pub fn show(&mut self, ui: &mut egui::Ui, cx: &ViewCx<'_>) -> Vec<PageMsg> {
        fn wrap<I>(v: Vec<I>, f: fn(I) -> PageMsg) -> Vec<PageMsg> {
            v.into_iter().map(f).collect()
        }
        match self {
            Self::Home(vm) => wrap(home::show(ui, &vm.state, cx), PageMsg::Home),
            Self::OrderList(vm) => wrap(order_list::show(ui, &vm.state, cx), PageMsg::OrderList),
            Self::OrderDetail(vm) => {
                wrap(order_detail::show(ui, &vm.state, cx), PageMsg::OrderDetail)
            }
            Self::ProductList(vm) => wrap(
                entity_list::show::<ProductList>(ui, &vm.state, cx),
                PageMsg::ProductList,
            ),
            Self::ProductDetail(vm) => wrap(
                entity_form::show::<ProductForm>(ui, &vm.state, cx),
                PageMsg::ProductDetail,
            ),
            Self::CustomerList(vm) => wrap(
                entity_list::show::<CustomerList>(ui, &vm.state, cx),
                PageMsg::CustomerList,
            ),
            Self::CustomerDetail(vm) => wrap(
                entity_form::show::<CustomerForm>(ui, &vm.state, cx),
                PageMsg::CustomerDetail,
            ),
            Self::Settings(vm) => wrap(settings::show(ui, &vm.state, cx), PageMsg::Settings),
            Self::PluginManager(vm) => wrap(
                plugin_manager::show(ui, &vm.state, cx),
                PageMsg::PluginManager,
            ),
            Self::FlowChart(vm) => wrap(flowchart::show(ui, vm, cx), PageMsg::FlowChart),
        }
    }
}
