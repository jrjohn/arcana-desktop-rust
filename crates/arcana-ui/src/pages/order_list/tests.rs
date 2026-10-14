use arcana_core::PagedResult;
use arcana_domain::{Order, OrderStatus};
use chrono::NaiveDate;
use rust_decimal::Decimal;

use super::*;
use crate::nav::Route;

pub(crate) fn order(
    id: i32,
    number: &str,
    customer: &str,
    day: u32,
    amount: i64,
    status: OrderStatus,
) -> Order {
    let mut o = Order::default();
    o.base.id = id;
    o.order_number = number.to_owned();
    o.customer_name = customer.to_owned();
    o.order_date = NaiveDate::from_ymd_opt(2025, 11, day)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    o.total_amount = Decimal::from(amount);
    o.status = status;
    o
}

fn sample() -> Vec<Order> {
    vec![
        order(
            1,
            "ORD-0001",
            "Kaohsiung Trading Co.",
            27,
            292_856,
            OrderStatus::Draft,
        ),
        order(
            2,
            "ORD-0002",
            "Hsinchu Electronics Inc.",
            27,
            15_120,
            OrderStatus::Draft,
        ),
        order(
            3,
            "ORD-0003",
            "Kaohsiung Trading Co.",
            17,
            207_449,
            OrderStatus::Pending,
        ),
        order(
            5,
            "ORD-0005",
            "Taichung Precision",
            17,
            220_500,
            OrderStatus::Confirmed,
        ),
    ]
}

fn loaded(vm: &mut ViewModel, items: Vec<Order>, total: u64) {
    let _ = vm.on(Input::Loaded(Ok(PagedResult::new(
        items,
        vm.state.current_page,
        PAGE_SIZE,
        total,
    ))));
}

fn query_of(fx: &[Effect<Task>]) -> OrderQuery {
    fx.iter()
        .find_map(|e| match e {
            Effect::Run(Task::Query(q)) => Some(q.clone()),
            _ => None,
        })
        .expect("query effect")
}

#[test]
fn initial_load_queries_first_page_newest_first() {
    let (vm, fx) = ViewModel::new();
    let q = query_of(&fx);
    assert_eq!((q.page, q.page_size), (1, 20));
    assert_eq!((q.sort, q.descending), (SortColumn::OrderDate, true));
    assert!(q.is_unfiltered());
    assert!(vm.state.is_loading);
}

#[test]
fn loaded_page_updates_counts_and_paging_flags() {
    let (mut vm, _) = ViewModel::new();
    loaded(&mut vm, sample(), 45);
    let s = &vm.state;
    assert_eq!((s.total_count, s.total_pages), (45, 3));
    assert!(!s.can_go_previous() && s.can_go_next());
    assert_eq!(s.showing_range(), (1, 4));
    assert!(!s.is_loading);
}

#[test]
fn paging_moves_within_bounds_only() {
    let (mut vm, _) = ViewModel::new();
    loaded(&mut vm, sample(), 45);
    assert!(vm.on(Input::PreviousPage).is_empty());
    assert_eq!(query_of(&vm.on(Input::NextPage)).page, 2);
    loaded(&mut vm, sample(), 45);
    assert_eq!(query_of(&vm.on(Input::GoToPage(3))).page, 3);
    loaded(&mut vm, sample(), 45);
    assert!(vm.on(Input::NextPage).is_empty());
    assert!(vm.on(Input::GoToPage(9)).is_empty());
    assert!(vm.on(Input::GoToPage(0)).is_empty());
    assert_eq!(query_of(&vm.on(Input::PreviousPage)).page, 2);
    assert_eq!(vm.state.showing_range().0, 21, "page 2 starts at row 21");
}

#[test]
fn search_and_filters_reset_to_first_page() {
    let (mut vm, _) = ViewModel::new();
    loaded(&mut vm, sample(), 45);
    let _ = vm.on(Input::NextPage);
    loaded(&mut vm, sample(), 45);
    assert!(
        vm.on(Input::SearchTextChanged(" kaohsiung ".into()))
            .is_empty()
    );
    let q = query_of(&vm.on(Input::Search));
    assert_eq!((q.page, q.search.as_str()), (1, "kaohsiung"));
    loaded(&mut vm, sample(), 4);
    let q = query_of(&vm.on(Input::StatusFilterChanged(Some(OrderStatus::Pending))));
    assert_eq!(q.status, Some(OrderStatus::Pending));
    loaded(&mut vm, sample(), 4);
    let _ = vm.on(Input::FromDateChanged("2025-11-01".into()));
    let _ = vm.on(Input::ToDateChanged("2025-11-30".into()));
    let q = query_of(&vm.on(Input::Search));
    assert_eq!(q.from, NaiveDate::from_ymd_opt(2025, 11, 1));
    assert_eq!(q.to, NaiveDate::from_ymd_opt(2025, 11, 30));
    loaded(&mut vm, sample(), 4);
    let q = query_of(&vm.on(Input::ClearFilter));
    assert!(q.is_unfiltered());
    assert!(vm.state.search_text.is_empty() && vm.state.from_date.is_empty());
}

#[test]
fn invalid_date_warns_but_still_queries_without_it() {
    let (mut vm, _) = ViewModel::new();
    loaded(&mut vm, vec![], 0);
    let _ = vm.on(Input::FromDateChanged("31/31/2025".into()));
    let fx = vm.on(Input::Search);
    assert!(matches!(&fx[0], Effect::Notify(n) if n.level == NoticeLevel::Warning));
    assert_eq!(query_of(&fx).from, None);
}

#[test]
fn sort_toggles_direction_on_same_column() {
    let (mut vm, _) = ViewModel::new();
    loaded(&mut vm, vec![], 0);
    let q = query_of(&vm.on(Input::SortBy(SortColumn::OrderDate)));
    assert!(!q.descending);
    loaded(&mut vm, vec![], 0);
    let q = query_of(&vm.on(Input::SortBy(SortColumn::Amount)));
    assert_eq!((q.sort, q.descending), (SortColumn::Amount, false));
    loaded(&mut vm, vec![], 0);
    assert!(query_of(&vm.on(Input::SortBy(SortColumn::Amount))).descending);
}

#[test]
fn no_double_load_while_busy_and_errors_notify() {
    let (mut vm, _) = ViewModel::new();
    assert!(vm.on(Input::Refresh).is_empty());
    let fx = vm.on(Input::Loaded(Err("db down".into())));
    assert_eq!(fx, vec![Effect::Notify(Notice::error("db down"))]);
    assert_eq!(vm.state.error.as_deref(), Some("db down"));
    assert!(!vm.on(Input::Refresh).is_empty());
}

#[test]
fn navigation_actions_match_csharp() {
    let mut vm = ViewModel::default();
    let route = |fx: Vec<Effect<Task>>| match fx.as_slice() {
        [Effect::Navigate(r)] => r.route.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        route(vm.on(Input::CreateOrder)),
        Route::OrderDetail {
            id: None,
            read_only: false
        }
    );
    assert_eq!(
        route(vm.on(Input::EditOrder(3))),
        Route::OrderDetail {
            id: Some(3),
            read_only: false
        }
    );
    assert_eq!(
        route(vm.on(Input::ViewOrder(3))),
        Route::OrderDetail {
            id: Some(3),
            read_only: true
        }
    );
}

#[test]
fn delete_confirms_then_runs_then_removes_row() {
    let (mut vm, _) = ViewModel::new();
    loaded(&mut vm, sample(), 4);
    assert!(vm.on(Input::DeleteOrder(99)).is_empty());
    assert_eq!(
        vm.on(Input::DeleteOrder(2)),
        vec![Effect::ConfirmDelete {
            id: 2,
            label: "ORD-0002".into()
        }]
    );
    assert_eq!(
        vm.on(Input::DeleteConfirmed(2)),
        vec![Effect::Run(Task::Delete {
            id: 2,
            order_number: "ORD-0002".into()
        })]
    );
    assert!(vm.on(Input::DeleteConfirmed(99)).is_empty());
    let fx = vm.on(Input::Deleted(Ok("ORD-0002".into())));
    assert!(fx.contains(&Effect::DataChanged(DataKind::Orders)));
    assert_eq!(vm.state.orders.len(), 3);
    assert_eq!(vm.state.total_count, 3);
    assert_eq!(
        vm.on(Input::Deleted(Err("no".into()))),
        vec![Effect::Notify(Notice::error("no"))]
    );
}

#[test]
fn change_status_advances_and_reloads() {
    let (mut vm, _) = ViewModel::new();
    loaded(&mut vm, sample(), 4);
    assert_eq!(
        vm.on(Input::ChangeStatus(3)),
        vec![Effect::Run(Task::ChangeStatus {
            id: 3,
            status: OrderStatus::Confirmed
        })]
    );
    assert!(vm.on(Input::ChangeStatus(42)).is_empty());
    let fx = vm.on(Input::StatusChanged(Ok(())));
    assert!(matches!(fx[0], Effect::Run(Task::Query(_))));
    assert!(fx.contains(&Effect::DataChanged(DataKind::Orders)));
    assert_eq!(vm.on(Input::StatusChanged(Err("x".into()))).len(), 1);
}

#[test]
fn selection_export_and_print() {
    let (mut vm, _) = ViewModel::new();
    loaded(&mut vm, sample(), 4);
    let warn = vm.on(Input::Print);
    assert!(matches!(&warn[0], Effect::Notify(n) if n.level == NoticeLevel::Warning));
    let _ = vm.on(Input::Select(Some(1)));
    let info = vm.on(Input::Print);
    assert!(matches!(&info[0], Effect::Notify(n) if n.level == NoticeLevel::Info));
    assert!(matches!(&vm.on(Input::Export)[0], Effect::Notify(_)));
    // A reload that no longer contains the selected row clears the selection.
    let _ = vm.on(Input::Refresh);
    loaded(
        &mut vm,
        vec![order(9, "X", "Y", 1, 1, OrderStatus::Draft)],
        1,
    );
    assert_eq!(vm.state.selected, None);
}

#[test]
fn query_apply_filters_sorts_and_pages() {
    let base = OrderQuery {
        page: 1,
        page_size: 2,
        search: String::new(),
        status: None,
        from: None,
        to: None,
        sort: SortColumn::Amount,
        descending: true,
    };
    let r = base.apply(sample());
    assert_eq!(r.total_count, 4);
    assert_eq!(
        r.items.iter().map(|o| o.base.id).collect::<Vec<_>>(),
        [1, 5]
    );
    let page2 = OrderQuery {
        page: 2,
        ..base.clone()
    }
    .apply(sample());
    assert_eq!(
        page2.items.iter().map(|o| o.base.id).collect::<Vec<_>>(),
        [3, 2]
    );

    let q = OrderQuery {
        search: "kaohsiung".into(),
        page_size: 20,
        ..base.clone()
    };
    assert_eq!(q.apply(sample()).total_count, 2);
    let q = OrderQuery {
        search: "ord-0005".into(),
        page_size: 20,
        ..base.clone()
    };
    assert_eq!(q.apply(sample()).items[0].base.id, 5);
    let q = OrderQuery {
        status: Some(OrderStatus::Draft),
        page_size: 20,
        ..base.clone()
    };
    assert_eq!(q.apply(sample()).total_count, 2);
    let q = OrderQuery {
        from: NaiveDate::from_ymd_opt(2025, 11, 20),
        to: NaiveDate::from_ymd_opt(2025, 11, 30),
        page_size: 20,
        ..base.clone()
    };
    assert_eq!(q.apply(sample()).total_count, 2);
    for (col, first) in [
        (SortColumn::OrderNumber, 1),
        (SortColumn::OrderDate, 3),
        (SortColumn::Customer, 2),
        (SortColumn::PaymentStatus, 1),
        (SortColumn::Status, 1),
    ] {
        let q = OrderQuery {
            sort: col,
            descending: false,
            page_size: 20,
            ..base.clone()
        };
        assert_eq!(q.apply(sample()).items[0].base.id, first, "{col:?}");
        assert!(!col.column().is_empty());
    }
    assert!(!q_unfiltered_page(&base).sort_by.unwrap().is_empty());
}

fn q_unfiltered_page(q: &OrderQuery) -> arcana_core::PageRequest {
    assert!(q.is_unfiltered());
    q.page_request()
}
