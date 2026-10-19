use arcana_domain::{Customer, Order, OrderItem, PaymentMethod, Product};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use super::*;

fn customer(id: i32, name: &str) -> Customer {
    let mut c = Customer {
        code: format!("C{id}"),
        name: name.to_owned(),
        address: Some("100 Xinyi Road".into()),
        city: Some("Taipei".into()),
        postal_code: Some("110".into()),
        ..Customer::default()
    };
    c.base.id = id;
    c
}

fn product(id: i32, code: &str, price: i64) -> Product {
    let mut p = Product {
        code: code.to_owned(),
        name: format!("Product {code}"),
        price: Decimal::from(price),
        unit: "pc".into(),
        ..Product::default()
    };
    p.base.id = id;
    p
}

fn existing_order() -> Order {
    let mut o = Order {
        order_number: "ORD-202512-0010".into(),
        customer_id: 5,
        customer_name: "Taoyuan".into(),
        ..Order::default()
    };
    o.base.id = 10;
    o.items.push(OrderItem {
        line_number: 1,
        product_id: 7,
        product_code: "P007".into(),
        product_name: "A4 Copy Paper".into(),
        quantity: dec!(3),
        unit_price: dec!(120),
        discount_percent: dec!(5),
        ..OrderItem::default()
    });
    o.calculate_totals();
    o
}

fn opened_new() -> ViewModel {
    let (mut vm, fx) = ViewModel::new(None, false);
    assert_eq!(fx, vec![Effect::Run(Task::Load(None))]);
    let order = Order {
        order_number: "ORD-NEW-0001".into(),
        ..Order::default()
    };
    let _ = vm.on(Input::Loaded(Ok(Loaded {
        customers: vec![customer(1, "Taipei Tech"), customer(5, "Taoyuan")],
        order,
    })));
    vm
}

fn opened_existing(read_only: bool) -> ViewModel {
    let (mut vm, fx) = ViewModel::new(Some(10), read_only);
    assert_eq!(fx, vec![Effect::Run(Task::Load(Some(10)))]);
    let _ = vm.on(Input::Loaded(Ok(Loaded {
        customers: vec![customer(5, "Taoyuan")],
        order: existing_order(),
    })));
    vm
}

fn with_products(vm: &mut ViewModel) {
    let _ = vm.on(Input::ProductsFound(Ok(vec![
        product(1, "P001", 35_000),
        product(2, "P002", 800),
    ])));
}

#[test]
fn new_order_starts_in_edit_mode() {
    let vm = opened_new();
    let s = &vm.state;
    assert!(s.is_new && s.is_editing && s.can_save() && !s.can_edit());
    assert_eq!(s.order_date_text, fmt::iso_date(s.order.order_date));
    assert!(!s.has_items());
}

#[test]
fn existing_order_is_read_until_edit() {
    let mut vm = opened_existing(false);
    assert!(!vm.state.is_editing && vm.state.can_edit());
    assert_eq!(vm.state.item_discount_text, vec!["5".to_owned()]);
    assert_eq!(vm.state.selected_customer().map(|c| c.base.id), Some(5));
    // Edits are ignored until Edit.
    assert_eq!(vm.on(Input::IncreaseQuantity(0)).len(), 0);
    assert_eq!(vm.state.order.items[0].quantity, dec!(3));
    let _ = vm.on(Input::Edit);
    assert!(vm.state.is_editing);
    let _ = vm.on(Input::IncreaseQuantity(0));
    assert_eq!(vm.state.order.items[0].quantity, dec!(4));
    assert!(vm.state.is_dirty);
}

#[test]
fn read_only_order_cannot_be_edited() {
    let mut vm = opened_existing(true);
    let _ = vm.on(Input::Edit);
    assert!(!vm.state.is_editing && !vm.state.can_edit());
}

#[test]
fn selecting_customer_copies_shipping_details() {
    let mut vm = opened_new();
    let _ = vm.on(Input::SelectCustomer(1));
    let o = &vm.state.order;
    assert_eq!(
        (o.customer_id, o.customer_name.as_str()),
        (1, "Taipei Tech")
    );
    assert_eq!(o.shipping_city.as_deref(), Some("Taipei"));
    assert_eq!(o.shipping_postal_code.as_deref(), Some("110"));
    assert!(vm.state.is_dirty);
    let _ = vm.on(Input::SelectCustomer(999));
    assert_eq!(vm.state.order.customer_id, 1);
}

#[test]
fn product_search_and_add_item_merges_lines() {
    let mut vm = opened_new();
    assert_eq!(
        vm.on(Input::SearchProducts).len(),
        0,
        "blank search does nothing"
    );
    let _ = vm.on(Input::ProductSearchChanged(" P00 ".into()));
    assert_eq!(
        vm.on(Input::SearchProducts),
        vec![Effect::Run(Task::SearchProducts("P00".into()))]
    );
    with_products(&mut vm);
    let _ = vm.on(Input::AddItem(2));
    let _ = vm.on(Input::AddItem(1));
    let _ = vm.on(Input::AddItem(2));
    assert_eq!(vm.on(Input::AddItem(42)).len(), 0);
    let items = &vm.state.order.items;
    assert_eq!(items.len(), 2);
    assert_eq!(
        (items[0].product_code.as_str(), items[0].quantity),
        ("P002", dec!(2))
    );
    assert_eq!(items[1].line_number, 2);
    assert_eq!(vm.state.order.subtotal, dec!(36600));
    assert_eq!(vm.state.order.tax_amount, dec!(1830));
    let fx = vm.on(Input::ProductsFound(Err("x".into())));
    assert_eq!(fx, vec![Effect::Notify(Notice::error("x"))]);
}

#[test]
fn quantity_discount_remove_and_clear_recalculate() {
    let mut vm = opened_existing(false);
    let _ = vm.on(Input::Edit);
    let _ = vm.on(Input::DecreaseQuantity(0));
    let _ = vm.on(Input::DecreaseQuantity(0));
    assert_eq!(vm.state.order.items[0].quantity, dec!(1));
    assert_eq!(vm.on(Input::DecreaseQuantity(0)).len(), 0, "never below 1");
    let _ = vm.on(Input::ItemDiscountChanged(0, "10".into()));
    assert_eq!(vm.state.order.items[0].discount_percent, dec!(10));
    assert_eq!(vm.state.order.subtotal, dec!(108));
    let _ = vm.on(Input::ItemDiscountChanged(0, "abc".into()));
    assert_eq!(vm.state.item_discount_text[0], "abc");
    assert_eq!(
        vm.state.order.items[0].discount_percent,
        dec!(10),
        "bad text keeps value"
    );
    assert_eq!(vm.on(Input::ItemDiscountChanged(9, "1".into())).len(), 0);
    assert_eq!(vm.on(Input::IncreaseQuantity(9)).len(), 0);
    assert_eq!(vm.on(Input::RemoveItem(9)).len(), 0);
    let _ = vm.on(Input::RemoveItem(0));
    assert!(!vm.state.has_items());
    assert_eq!(vm.state.order.total_amount, Decimal::ZERO);
    with_products(&mut vm);
    let _ = vm.on(Input::AddItem(1));
    let _ = vm.on(Input::ClearItems);
    assert!(vm.state.order.items.is_empty() && vm.state.item_discount_text.is_empty());
}

#[test]
fn summary_fields_drive_total() {
    let mut vm = opened_existing(false);
    let _ = vm.on(Input::Edit);
    let _ = vm.on(Input::ShippingCostChanged("100".into()));
    let _ = vm.on(Input::DiscountChanged("1,000".into()));
    let o = &vm.state.order;
    assert_eq!(o.shipping_cost, dec!(100));
    assert_eq!(o.discount_amount, dec!(1000));
    assert_eq!(
        o.total_amount,
        o.subtotal + o.tax_amount + dec!(100) - dec!(1000)
    );
    let _ = vm.on(Input::CalculateTotals);
    let _ = vm.on(Input::PaymentMethodChanged(PaymentMethod::BankTransfer));
    assert_eq!(vm.state.order.payment_method, PaymentMethod::BankTransfer);
}

#[test]
fn text_fields_map_to_order() {
    let mut vm = opened_new();
    let _ = vm.on(Input::OrderDateChanged("2025-12-01".into()));
    let _ = vm.on(Input::ExpectedDeliveryChanged("2025-12-09".into()));
    let _ = vm.on(Input::ShippingAddressChanged(" 1 Road ".into()));
    let _ = vm.on(Input::ShippingCityChanged(String::new()));
    let _ = vm.on(Input::ShippingPostalCodeChanged("320".into()));
    let _ = vm.on(Input::NotesChanged("Urgent".into()));
    let o = &vm.state.order;
    assert_eq!(fmt::iso_date(o.order_date), "2025-12-01");
    assert_eq!(
        o.expected_delivery_date.map(fmt::iso_date).as_deref(),
        Some("2025-12-09")
    );
    assert_eq!(o.shipping_address.as_deref(), Some("1 Road"));
    assert_eq!(o.shipping_city, None);
    assert_eq!(o.shipping_postal_code.as_deref(), Some("320"));
    assert_eq!(o.notes.as_deref(), Some("Urgent"));
    let _ = vm.on(Input::OrderDateChanged("bad".into()));
    assert_eq!(fmt::iso_date(vm.state.order.order_date), "2025-12-01");
}

#[test]
fn save_runs_domain_validator_first() {
    let mut vm = opened_new();
    let fx = vm.on(Input::Save);
    assert!(matches!(&fx[0], Effect::Notify(n) if n.level == NoticeLevel::Warning));
    assert!(
        vm.state
            .validation
            .contains(&"Please select a customer".to_owned())
    );
    assert!(vm.state.validation.iter().any(|m| m.contains("item")));
    assert!(!vm.state.is_busy);
}

#[test]
fn save_new_then_saved_switches_to_view_mode() {
    let mut vm = opened_new();
    let _ = vm.on(Input::SelectCustomer(5));
    with_products(&mut vm);
    let _ = vm.on(Input::AddItem(1));
    let fx = vm.on(Input::Save);
    let Some(Effect::Run(Task::Save {
        order,
        is_new: true,
    })) = fx.first()
    else {
        panic!("{fx:?}");
    };
    assert_eq!(order.customer_name, "Taoyuan");
    assert!(vm.state.validation.is_empty() && vm.state.is_busy);
    assert_eq!(vm.on(Input::Save).len(), 0, "busy");
    let mut saved = (**order).clone();
    saved.base.id = 77;
    let fx = vm.on(Input::Saved(Ok(saved)));
    assert!(fx.contains(&Effect::DataChanged(DataKind::Orders)));
    let s = &vm.state;
    assert!(!s.is_new && !s.is_editing && !s.is_dirty);
    assert_eq!(s.order_id, Some(77));
}

#[test]
fn save_error_is_shown() {
    let mut vm = opened_existing(false);
    let _ = vm.on(Input::Edit);
    assert!(matches!(
        vm.on(Input::Save)[0],
        Effect::Run(Task::Save { is_new: false, .. })
    ));
    let fx = vm.on(Input::Saved(Err("Validation failed".into())));
    assert_eq!(fx, vec![Effect::Notify(Notice::error("Validation failed"))]);
    assert_eq!(vm.state.validation, vec!["Validation failed".to_owned()]);
    assert!(!vm.state.is_busy);
}

#[test]
fn cancel_confirms_when_dirty_and_closes_or_reloads() {
    let mut vm = opened_new();
    assert_eq!(vm.on(Input::Cancel), vec![Effect::CloseSelf]);
    let _ = vm.on(Input::NotesChanged("x".into()));
    assert_eq!(vm.on(Input::Cancel), vec![Effect::ConfirmDiscard]);
    assert_eq!(vm.on(Input::DiscardConfirmed), vec![Effect::CloseSelf]);

    let mut vm = opened_existing(false);
    let _ = vm.on(Input::Edit);
    let _ = vm.on(Input::NotesChanged("x".into()));
    assert_eq!(
        vm.on(Input::DiscardConfirmed),
        vec![Effect::Run(Task::Load(Some(10)))]
    );
    assert!(!vm.state.is_editing);
}

#[test]
fn copy_turns_order_into_new_draft() {
    let mut vm = opened_new();
    assert_eq!(vm.on(Input::Copy).len(), 0, "nothing to copy yet");
    let mut vm = opened_existing(true);
    let _ = vm.on(Input::Copy);
    let s = &vm.state;
    assert_eq!(s.copied_from.as_deref(), Some("ORD-202512-0010"));
    assert!(s.is_new && s.is_editing && !s.read_only);
    assert_eq!(s.order.base.id, 0);
    assert_eq!(s.order.order_number.len(), 0);
    assert_eq!(s.order.items[0].base.id, 0);
    assert!(matches!(&vm.on(Input::Print)[0], Effect::Notify(_)));
}

#[test]
fn load_error_and_busy_guard() {
    let (mut vm, _) = ViewModel::new(Some(1), false);
    assert_eq!(vm.on(Input::Load).len(), 0);
    let fx = vm.on(Input::Loaded(Err("Order with ID 1 not found".into())));
    assert!(matches!(&fx[0], Effect::Notify(_)));
    assert!(vm.state.error.is_some() && !vm.state.is_loading);
}
