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
        self.detail_title(cx)
            .unwrap_or_else(|| cx.t(self.fixed_title_key()))
    }

    /// Title key of pages whose title does not depend on their state.
    fn fixed_title_key(&self) -> &'static str {
        match self {
            Self::Home(_) => "nav.home",
            Self::OrderList(_) => "nav.orders",
            Self::ProductList(_) => "nav.products",
            Self::CustomerList(_) => "nav.customers",
            Self::Settings(_) => "nav.settings",
            Self::PluginManager(_) => "nav.plugins",
            Self::OrderDetail(_) => "order.new",
            Self::ProductDetail(_) => "quick.newProduct",
            Self::CustomerDetail(_) => "quick.newCustomer",
            Self::FlowChart(_) => "flowchart.editor",
        }
    }

    /// Title of an existing record / open diagram; `None` falls back to the fixed key.
    fn detail_title(&self, cx: &ViewCx<'_>) -> Option<String> {
        match self {
            Self::OrderDetail(vm) if !vm.state.is_new => Some(format!(
                "{} #{}",
                cx.t("order.title"),
                vm.state.order_id.unwrap_or_default()
            )),
            Self::FlowChart(vm) => Some(format!(
                "{} — {}",
                cx.t("flowchart.editor"),
                vm.editor.state.diagram.name
            )),
            _ => self.entity_title(cx),
        }
    }

    fn entity_title(&self, cx: &ViewCx<'_>) -> Option<String> {
        let (key, code) = match self {
            Self::ProductDetail(vm) if !vm.state.is_new => ("product.title", &vm.state.entity.code),
            Self::CustomerDetail(vm) if !vm.state.is_new => {
                ("customer.title", &vm.state.entity.code)
            }
            _ => return None,
        };
        Some(format!("{} {code}", cx.t(key)))
    }

    /// Draw the page; returns the inputs the user produced.
    pub fn show(&mut self, ui: &mut egui::Ui, cx: &ViewCx<'_>) -> Vec<PageMsg> {
        match self {
            Self::Home(vm) => wrap(home::show(ui, &vm.state, cx), PageMsg::Home),
            Self::OrderList(vm) => wrap(order_list::show(ui, &vm.state, cx), PageMsg::OrderList),
            Self::OrderDetail(vm) => {
                wrap(order_detail::show(ui, &vm.state, cx), PageMsg::OrderDetail)
            }
            Self::Settings(vm) => wrap(settings::show(ui, &vm.state, cx), PageMsg::Settings),
            Self::PluginManager(vm) => wrap(
                plugin_manager::show(ui, &vm.state, cx),
                PageMsg::PluginManager,
            ),
            Self::FlowChart(vm) => wrap(flowchart::show(ui, vm, cx), PageMsg::FlowChart),
            entity => entity.show_entity(ui, cx),
        }
    }

    /// The generic product / customer list and form pages.
    fn show_entity(&mut self, ui: &mut egui::Ui, cx: &ViewCx<'_>) -> Vec<PageMsg> {
        match self {
            Self::ProductList(vm) => wrap(
                entity_list::show::<ProductList>(ui, &vm.state, cx),
                PageMsg::ProductList,
            ),
            Self::CustomerList(vm) => wrap(
                entity_list::show::<CustomerList>(ui, &vm.state, cx),
                PageMsg::CustomerList,
            ),
            form => form.show_form(ui, cx),
        }
    }

    fn show_form(&mut self, ui: &mut egui::Ui, cx: &ViewCx<'_>) -> Vec<PageMsg> {
        match self {
            Self::ProductDetail(vm) => wrap(
                entity_form::show::<ProductForm>(ui, &vm.state, cx),
                PageMsg::ProductDetail,
            ),
            Self::CustomerDetail(vm) => wrap(
                entity_form::show::<CustomerForm>(ui, &vm.state, cx),
                PageMsg::CustomerDetail,
            ),
            _ => Vec::new(),
        }
    }
}

fn wrap<I>(v: Vec<I>, f: fn(I) -> PageMsg) -> Vec<PageMsg> {
    v.into_iter().map(f).collect()
}
