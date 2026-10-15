//! Order detail / new order (C# `OrderDetailViewModel` + `OrderDetailPage`).
//!
//! Before saving, the order is checked with the domain [`OrderValidator`]; its messages are
//! shown on the page (C# only pre-checks "customer selected" and "has items" and otherwise
//! surfaces the service error).

mod task;
#[cfg(test)]
mod tests;
mod view;

pub use task::run;
pub use view::show;

use arcana_domain::{
    Customer, Order, OrderItem, OrderStatus, OrderValidator, PaymentMethod, Product, Validator,
};
use rust_decimal::Decimal;

use crate::effect::{DataKind, Effect, Notice, NoticeLevel, Text};
use crate::fmt;

/// What a load returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loaded {
    pub customers: Vec<Customer>,
    pub order: Order,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
    pub order_id: Option<i32>,
    pub order: Order,
    pub customers: Vec<Customer>,
    pub products: Vec<Product>,
    pub product_search: String,
    pub is_new: bool,
    pub read_only: bool,
    pub is_editing: bool,
    pub is_dirty: bool,
    pub is_loading: bool,
    pub is_busy: bool,
    /// Editable text mirrors of numeric / date fields.
    pub order_date_text: String,
    pub expected_delivery_text: String,
    pub shipping_cost_text: String,
    pub discount_text: String,
    /// Per-item discount% text, parallel to `order.items`.
    pub item_discount_text: Vec<String>,
    /// Domain validation messages from the last save attempt.
    pub validation: Vec<String>,
    pub error: Option<String>,
    /// Order number this one was copied from (C# "Copied from Order").
    pub copied_from: Option<String>,
}

impl State {
    #[must_use]
    pub fn can_save(&self) -> bool {
        self.is_editing && !self.is_busy
    }
    #[must_use]
    pub fn can_edit(&self) -> bool {
        !self.is_editing && !self.is_new && !self.read_only
    }
    #[must_use]
    pub fn has_items(&self) -> bool {
        !self.order.items.is_empty()
    }
    #[must_use]
    pub fn selected_customer(&self) -> Option<&Customer> {
        self.customers
            .iter()
            .find(|c| c.base.id == self.order.customer_id)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    Load,
    Loaded(Result<Loaded, String>),
    Edit,
    Save,
    Saved(Result<Order, String>),
    Cancel,
    DiscardConfirmed,
    Copy,
    Print,
    SelectCustomer(i32),
    PaymentMethodChanged(PaymentMethod),
    OrderDateChanged(String),
    ExpectedDeliveryChanged(String),
    ShippingAddressChanged(String),
    ShippingCityChanged(String),
    ShippingPostalCodeChanged(String),
    NotesChanged(String),
    ShippingCostChanged(String),
    DiscountChanged(String),
    ProductSearchChanged(String),
    SearchProducts,
    ProductsFound(Result<Vec<Product>, String>),
    AddItem(i32),
    RemoveItem(usize),
    IncreaseQuantity(usize),
    DecreaseQuantity(usize),
    ItemDiscountChanged(usize, String),
    ClearItems,
    CalculateTotals,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Task {
    /// `None` = new order (generate a number).
    Load(Option<i32>),
    SearchProducts(String),
    Save {
        order: Box<Order>,
        is_new: bool,
    },
}

#[derive(Debug, Default)]
pub struct ViewModel {
    pub state: State,
}

fn opt(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_owned())
}

impl ViewModel {
    /// `id = None` opens a new order.
    #[must_use]
    pub fn new(id: Option<i32>, read_only: bool) -> (Self, Vec<Effect<Task>>) {
        let mut vm = Self::default();
        vm.state.order_id = id;
        vm.state.is_new = id.is_none();
        vm.state.read_only = read_only;
        let fx = vm.on(Input::Load);
        (vm, fx)
    }

    fn sync_texts(&mut self) {
        let s = &mut self.state;
        s.order_date_text = fmt::iso_date(s.order.order_date);
        s.expected_delivery_text = s
            .order
            .expected_delivery_date
            .map(fmt::iso_date)
            .unwrap_or_default();
        s.shipping_cost_text = fmt::number(s.order.shipping_cost, 0).replace(',', "");
        s.discount_text = fmt::number(s.order.discount_amount, 0).replace(',', "");
        s.item_discount_text = s
            .order
            .items
            .iter()
            .map(|i| i.discount_percent.normalize().to_string())
            .collect();
    }

    fn recalc(&mut self) {
        self.state.order.calculate_totals();
    }

    fn touch(&mut self) -> Vec<Effect<Task>> {
        self.state.is_dirty = true;
        self.recalc();
        vec![]
    }

    fn editable(&self) -> bool {
        self.state.is_editing && !self.state.is_busy
    }

    fn renumber(&mut self) {
        for (n, item) in (1..).zip(self.state.order.items.iter_mut()) {
            item.line_number = n;
        }
    }

    fn save(&mut self) -> Vec<Effect<Task>> {
        if !self.state.can_save() {
            return vec![];
        }
        let s = &mut self.state;
        if let Some(c) = s
            .customers
            .iter()
            .find(|c| c.base.id == s.order.customer_id)
        {
            s.order.customer_name.clone_from(&c.name);
        }
        s.order.calculate_totals();
        let result = OrderValidator.validate(&s.order);
        s.validation = result
            .errors
            .iter()
            .map(|e| e.error_message.clone())
            .collect();
        s.validation.dedup();
        if let Some(first) = s.validation.first() {
            return vec![Effect::Notify(Notice::new(
                NoticeLevel::Warning,
                Text::Raw(first.clone()),
            ))];
        }
        s.is_busy = true;
        vec![Effect::Run(Task::Save {
            order: Box::new(s.order.clone()),
            is_new: s.is_new,
        })]
    }

    #[allow(clippy::too_many_lines)]
    pub fn on(&mut self, input: Input) -> Vec<Effect<Task>> {
        match input {
            Input::Load => {
                if self.state.is_busy {
                    return vec![];
                }
                self.state.is_busy = true;
                self.state.is_loading = true;
                self.state.error = None;
                vec![Effect::Run(Task::Load(self.state.order_id))]
            }
            Input::Loaded(Ok(l)) => {
                let s = &mut self.state;
                s.is_busy = false;
                s.is_loading = false;
                s.customers = l.customers;
                s.order = l.order;
                s.is_editing = s.is_new;
                s.is_dirty = false;
                s.validation.clear();
                self.sync_texts();
                vec![]
            }
            Input::Loaded(Err(e)) => {
                self.state.is_busy = false;
                self.state.is_loading = false;
                self.state.error = Some(e.clone());
                vec![Effect::Notify(Notice::error(e))]
            }
            Input::Edit => {
                if self.state.can_edit() {
                    self.state.is_editing = true;
                }
                vec![]
            }
            Input::Save => self.save(),
            Input::Saved(Ok(order)) => {
                let s = &mut self.state;
                s.is_busy = false;
                s.order_id = Some(order.base.id);
                s.order = order;
                s.is_new = false;
                s.is_editing = false;
                s.is_dirty = false;
                s.copied_from = None;
                self.sync_texts();
                vec![
                    Effect::Notify(Notice::new(
                        NoticeLevel::Success,
                        Text::Key("ui.order.saved"),
                    )),
                    Effect::DataChanged(DataKind::Orders),
                ]
            }
            Input::Saved(Err(e)) => {
                self.state.is_busy = false;
                self.state.validation = vec![e.clone()];
                vec![Effect::Notify(Notice::error(e))]
            }
            Input::Cancel => {
                if self.state.is_dirty {
                    vec![Effect::ConfirmDiscard]
                } else {
                    self.on(Input::DiscardConfirmed)
                }
            }
            Input::DiscardConfirmed => {
                if self.state.is_new {
                    vec![Effect::CloseSelf]
                } else {
                    self.state.is_editing = false;
                    self.on(Input::Load)
                }
            }
            Input::Copy => {
                if self.state.is_new {
                    return vec![];
                }
                let s = &mut self.state;
                s.copied_from = Some(s.order.order_number.clone());
                s.order.base = arcana_domain::BaseEntity::default();
                s.order.order_number.clear();
                s.order.status = OrderStatus::Draft;
                s.order.order_date = arcana_core::today();
                for item in &mut s.order.items {
                    item.base = arcana_domain::BaseEntity::default();
                    item.order_id = 0;
                }
                s.order_id = None;
                s.is_new = true;
                s.read_only = false;
                s.is_editing = true;
                s.is_dirty = true;
                self.sync_texts();
                vec![]
            }
            Input::Print => vec![Effect::Notify(Notice::new(
                NoticeLevel::Info,
                Text::Key("common.developing"),
            ))],
            input if !self.editable() => {
                // Everything below edits the order; ignored unless editing (C# disables the
                // controls). Search results may still arrive.
                if let Input::ProductsFound(r) = input {
                    self.found(r)
                } else {
                    vec![]
                }
            }
            Input::SelectCustomer(id) => {
                let s = &mut self.state;
                if let Some(c) = s.customers.iter().find(|c| c.base.id == id) {
                    s.order.customer_id = c.base.id;
                    s.order.customer_name.clone_from(&c.name);
                    s.order.shipping_address.clone_from(&c.address);
                    s.order.shipping_city.clone_from(&c.city);
                    s.order.shipping_postal_code.clone_from(&c.postal_code);
                    s.is_dirty = true;
                }
                vec![]
            }
            Input::PaymentMethodChanged(m) => {
                self.state.order.payment_method = m;
                self.touch()
            }
            Input::OrderDateChanged(t) => {
                if let Some(d) = fmt::parse_date(&t) {
                    self.state.order.order_date = d.and_time(chrono::NaiveTime::MIN);
                }
                self.state.order_date_text = t;
                self.touch()
            }
            Input::ExpectedDeliveryChanged(t) => {
                self.state.order.expected_delivery_date =
                    fmt::parse_date(&t).map(|d| d.and_time(chrono::NaiveTime::MIN));
                self.state.expected_delivery_text = t;
                self.touch()
            }
            Input::ShippingAddressChanged(t) => {
                self.state.order.shipping_address = opt(&t);
                self.touch()
            }
            Input::ShippingCityChanged(t) => {
                self.state.order.shipping_city = opt(&t);
                self.touch()
            }
            Input::ShippingPostalCodeChanged(t) => {
                self.state.order.shipping_postal_code = opt(&t);
                self.touch()
            }
            Input::NotesChanged(t) => {
                self.state.order.notes = opt(&t);
                self.touch()
            }
            Input::ShippingCostChanged(t) => {
                if let Some(v) = fmt::parse_decimal(&t) {
                    self.state.order.shipping_cost = v;
                }
                self.state.shipping_cost_text = t;
                self.touch()
            }
            Input::DiscountChanged(t) => {
                if let Some(v) = fmt::parse_decimal(&t) {
                    self.state.order.discount_amount = v;
                }
                self.state.discount_text = t;
                self.touch()
            }
            Input::ProductSearchChanged(t) => {
                self.state.product_search = t;
                vec![]
            }
            Input::SearchProducts => {
                let q = self.state.product_search.trim().to_owned();
                if q.is_empty() {
                    vec![]
                } else {
                    vec![Effect::Run(Task::SearchProducts(q))]
                }
            }
            Input::ProductsFound(r) => self.found(r),
            Input::AddItem(product_id) => self.add_item(product_id),
            Input::RemoveItem(i) => {
                if i < self.state.order.items.len() {
                    self.state.order.items.remove(i);
                    self.state.item_discount_text.remove(i);
                    self.renumber();
                    return self.touch();
                }
                vec![]
            }
            Input::IncreaseQuantity(i) => match self.state.order.items.get_mut(i) {
                Some(item) => {
                    item.quantity += Decimal::ONE;
                    self.touch()
                }
                None => vec![],
            },
            Input::DecreaseQuantity(i) => match self.state.order.items.get_mut(i) {
                Some(item) if item.quantity > Decimal::ONE => {
                    item.quantity -= Decimal::ONE;
                    self.touch()
                }
                _ => vec![],
            },
            Input::ItemDiscountChanged(i, t) => {
                if i >= self.state.order.items.len() {
                    return vec![];
                }
                if let Some(v) = fmt::parse_decimal(&t) {
                    self.state.order.items[i].discount_percent = v;
                }
                self.state.item_discount_text[i] = t;
                self.touch()
            }
            Input::ClearItems => {
                self.state.order.items.clear();
                self.state.item_discount_text.clear();
                self.touch()
            }
            Input::CalculateTotals => {
                self.recalc();
                vec![]
            }
        }
    }

    fn found(&mut self, r: Result<Vec<Product>, String>) -> Vec<Effect<Task>> {
        match r {
            Ok(p) => {
                self.state.products = p;
                vec![]
            }
            Err(e) => vec![Effect::Notify(Notice::error(e))],
        }
    }

    /// C# `AddItem`: an existing line for the product gets +1, otherwise a new line.
    fn add_item(&mut self, product_id: i32) -> Vec<Effect<Task>> {
        let Some(p) = self
            .state
            .products
            .iter()
            .find(|p| p.base.id == product_id)
            .cloned()
        else {
            return vec![];
        };
        let items = &mut self.state.order.items;
        if let Some(existing) = items.iter_mut().find(|i| i.product_id == product_id) {
            existing.quantity += Decimal::ONE;
        } else {
            items.push(OrderItem {
                line_number: i32::try_from(items.len() + 1).unwrap_or(i32::MAX),
                product_id: p.base.id,
                product_code: p.code.clone(),
                product_name: p.name.clone(),
                unit: p.unit.clone(),
                unit_price: p.price,
                quantity: Decimal::ONE,
                discount_percent: Decimal::ZERO,
                ..OrderItem::default()
            });
            self.state.item_discount_text.push("0".to_owned());
        }
        self.touch()
    }
}
