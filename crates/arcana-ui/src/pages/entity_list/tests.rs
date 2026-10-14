use arcana_core::PagedResult;

use super::*;
use crate::nav::{NavGraph, Route};

#[derive(Debug)]
struct Fake;

impl ListKind for Fake {
    type Row = (i32, String);
    const TITLE_KEY: &'static str = "t";
    const NEW_KEY: &'static str = "n";
    const SEARCH_HINT_KEY: &'static str = "s";
    const SALT: &'static str = "fake";
    const COLUMNS: &'static [&'static str] = &["a"];
    const DATA: DataKind = DataKind::Products;
    fn id(row: &Self::Row) -> i32 {
        row.0
    }
    fn label(row: &Self::Row) -> String {
        row.1.clone()
    }
    fn cells(row: &Self::Row) -> Vec<String> {
        vec![row.1.clone()]
    }
    fn new_route() -> NavRequest {
        NavGraph::to_new_product()
    }
    fn detail_route(id: i32) -> NavRequest {
        NavGraph::to_product_detail(id)
    }
}

fn rows() -> Vec<(i32, String)> {
    vec![(1, "P001".into()), (2, "P002".into())]
}

fn loaded(vm: &mut ViewModel<Fake>, total: u64) {
    let _ = vm.on(Input::Loaded(Ok(PagedResult::new(
        rows(),
        vm.state.current_page,
        PAGE_SIZE,
        total,
    ))));
}

#[test]
fn loads_pages_and_searches() {
    let (mut vm, fx) = ViewModel::<Fake>::new();
    assert_eq!(
        fx,
        vec![Effect::Run(Task::Load {
            page: 1,
            page_size: 20,
            search: String::new()
        })]
    );
    assert!(vm.on(Input::Refresh).is_empty(), "busy");
    loaded(&mut vm, 41);
    assert_eq!(vm.state.total_pages, 3);
    assert!(vm.on(Input::PreviousPage).is_empty());
    assert_eq!(
        vm.on(Input::NextPage),
        vec![Effect::Run(Task::Load {
            page: 2,
            page_size: 20,
            search: String::new()
        })]
    );
    loaded(&mut vm, 41);
    let _ = vm.on(Input::SearchTextChanged(" mouse ".into()));
    assert_eq!(
        vm.on(Input::Search),
        vec![Effect::Run(Task::Load {
            page: 1,
            page_size: 20,
            search: "mouse".into()
        })]
    );
    loaded(&mut vm, 1);
    assert!(vm.on(Input::NextPage).is_empty());
    let _ = vm.on(Input::Refresh);
    let fx = vm.on(Input::Loaded(Err("e".into())));
    assert_eq!(fx, vec![Effect::Notify(Notice::error("e"))]);
    assert!(!vm.state.is_loading);
    loaded(&mut vm, 41);
    let _ = vm.on(Input::NextPage);
    loaded(&mut vm, 41);
    assert!(matches!(
        vm.on(Input::PreviousPage)[0],
        Effect::Run(Task::Load { page: 1, .. })
    ));
}

#[test]
fn navigation_and_delete_flow() {
    let (mut vm, _) = ViewModel::<Fake>::new();
    loaded(&mut vm, 2);
    assert!(
        matches!(&vm.on(Input::New)[0], Effect::Navigate(r) if r.route == Route::ProductDetail { id: None })
    );
    assert!(
        matches!(&vm.on(Input::Open(2))[0], Effect::Navigate(r) if r.route == Route::ProductDetail { id: Some(2) })
    );
    assert_eq!(
        vm.on(Input::Delete(2)),
        vec![Effect::ConfirmDelete {
            id: 2,
            label: "P002".into()
        }]
    );
    assert!(vm.on(Input::Delete(9)).is_empty());
    assert_eq!(
        vm.on(Input::DeleteConfirmed(2)),
        vec![Effect::Run(Task::Delete {
            id: 2,
            label: "P002".into()
        })]
    );
    assert!(vm.on(Input::DeleteConfirmed(9)).is_empty());
    let fx = vm.on(Input::Deleted(Ok("P002".into())));
    assert!(fx.contains(&Effect::DataChanged(DataKind::Products)));
    assert_eq!(vm.state.rows.len(), 1);
    assert_eq!(vm.state.total_count, 1);
    assert_eq!(
        vm.on(Input::Deleted(Err("x".into()))),
        vec![Effect::Notify(Notice::error("x"))]
    );
}
