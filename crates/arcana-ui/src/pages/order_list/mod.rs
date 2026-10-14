//! Order list (C# `OrderListViewModel` + `OrderListPage`).
//!
//! Same filters, columns, paging and actions as C#. Two C# gaps are closed: C# collects the
//! search text, status and date range but never sends them with the query, and its columns
//! are not sortable. Here both work (see [`query`]).

pub mod query;
mod task;
#[cfg(test)]
mod tests;
mod view;

pub use query::{OrderQuery, SortColumn};
pub use task::run;
pub use view::show;

use arcana_core::PagedResult;
use arcana_domain::{Order, OrderStatus};

use crate::effect::{DataKind, Effect, Notice, NoticeLevel, Text};
use crate::fmt;
use crate::nav::NavGraph;

/// C# `PageSize = 20`.
pub const PAGE_SIZE: u32 = 20;

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub orders: Vec<Order>,
    pub selected: Option<i32>,
    pub current_page: u32,
    pub total_pages: u64,
    pub total_count: u64,
    pub search_text: String,
    pub status_filter: Option<OrderStatus>,
    /// Raw `yyyy-MM-dd` text; empty = no bound.
    pub from_date: String,
    pub to_date: String,
    pub sort: SortColumn,
    pub descending: bool,
    pub is_loading: bool,
    pub error: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            orders: Vec::new(),
            selected: None,
            current_page: 1,
            total_pages: 1,
            total_count: 0,
            search_text: String::new(),
            status_filter: None,
            from_date: String::new(),
            to_date: String::new(),
            sort: SortColumn::OrderDate,
            descending: true,
            is_loading: false,
            error: None,
        }
    }
}

impl State {
    #[must_use]
    pub fn can_go_previous(&self) -> bool {
        self.current_page > 1
    }
    #[must_use]
    pub fn can_go_next(&self) -> bool {
        u64::from(self.current_page) < self.total_pages
    }
    /// First and last row number on this page, 1-based (`Showing 1-10 of 10`).
    #[must_use]
    pub fn showing_range(&self) -> (u64, u64) {
        if self.orders.is_empty() {
            return (0, 0);
        }
        let first = u64::from(self.current_page - 1) * u64::from(PAGE_SIZE) + 1;
        (first, first + self.orders.len() as u64 - 1)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    Load,
    Loaded(Result<PagedResult<Order>, String>),
    Refresh,
    SearchTextChanged(String),
    Search,
    StatusFilterChanged(Option<OrderStatus>),
    FromDateChanged(String),
    ToDateChanged(String),
    ClearFilter,
    PreviousPage,
    NextPage,
    GoToPage(u32),
    SortBy(SortColumn),
    Select(Option<i32>),
    CreateOrder,
    EditOrder(i32),
    ViewOrder(i32),
    DeleteOrder(i32),
    DeleteConfirmed(i32),
    Deleted(Result<String, String>),
    ChangeStatus(i32),
    StatusChanged(Result<(), String>),
    Export,
    Print,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Task {
    Query(OrderQuery),
    Delete { id: i32, order_number: String },
    ChangeStatus { id: i32, status: OrderStatus },
}

#[derive(Debug, Default)]
pub struct ViewModel {
    pub state: State,
}

impl ViewModel {
    /// New view model plus its initial load (C# `InitializeAsync`).
    #[must_use]
    pub fn new() -> (Self, Vec<Effect<Task>>) {
        let mut vm = Self::default();
        let fx = vm.on(Input::Load);
        (vm, fx)
    }

    fn query(&self) -> OrderQuery {
        let s = &self.state;
        OrderQuery {
            page: s.current_page,
            page_size: PAGE_SIZE,
            search: s.search_text.trim().to_owned(),
            status: s.status_filter,
            from: fmt::parse_date(&s.from_date),
            to: fmt::parse_date(&s.to_date),
            sort: s.sort,
            descending: s.descending,
        }
    }

    fn load(&mut self) -> Vec<Effect<Task>> {
        if self.state.is_loading {
            return vec![];
        }
        self.state.is_loading = true;
        self.state.error = None;
        let mut fx = Vec::new();
        for (raw, _) in [(&self.state.from_date, 0), (&self.state.to_date, 1)] {
            if !raw.trim().is_empty() && fmt::parse_date(raw).is_none() {
                fx.push(Effect::Notify(Notice::new(
                    NoticeLevel::Warning,
                    Text::KeyWith("ui.invalidDate", vec![raw.clone()]),
                )));
            }
        }
        fx.push(Effect::Run(Task::Query(self.query())));
        fx
    }

    fn first_page_and_load(&mut self) -> Vec<Effect<Task>> {
        self.state.current_page = 1;
        self.load()
    }

    #[allow(clippy::too_many_lines)]
    pub fn on(&mut self, input: Input) -> Vec<Effect<Task>> {
        match input {
            Input::Load | Input::Refresh => self.load(),
            Input::Loaded(Ok(page)) => {
                let s = &mut self.state;
                s.is_loading = false;
                s.total_count = page.total_count;
                s.total_pages = page.total_pages().max(1);
                s.orders = page.items;
                if s.selected
                    .is_some_and(|id| !s.orders.iter().any(|o| o.base.id == id))
                {
                    s.selected = None;
                }
                vec![]
            }
            Input::Loaded(Err(e)) => {
                self.state.is_loading = false;
                self.state.error = Some(e.clone());
                vec![Effect::Notify(Notice::error(e))]
            }
            Input::SearchTextChanged(t) => {
                self.state.search_text = t;
                vec![]
            }
            Input::Search => self.first_page_and_load(),
            Input::StatusFilterChanged(st) => {
                self.state.status_filter = st;
                self.first_page_and_load()
            }
            Input::FromDateChanged(d) => {
                self.state.from_date = d;
                vec![]
            }
            Input::ToDateChanged(d) => {
                self.state.to_date = d;
                vec![]
            }
            Input::ClearFilter => {
                let s = &mut self.state;
                s.search_text.clear();
                s.status_filter = None;
                s.from_date.clear();
                s.to_date.clear();
                self.first_page_and_load()
            }
            Input::PreviousPage => {
                if self.state.can_go_previous() {
                    self.state.current_page -= 1;
                    self.load()
                } else {
                    vec![]
                }
            }
            Input::NextPage => {
                if self.state.can_go_next() {
                    self.state.current_page += 1;
                    self.load()
                } else {
                    vec![]
                }
            }
            Input::GoToPage(p) => {
                if p >= 1 && u64::from(p) <= self.state.total_pages {
                    self.state.current_page = p;
                    self.load()
                } else {
                    vec![]
                }
            }
            Input::SortBy(col) => {
                let s = &mut self.state;
                if s.sort == col {
                    s.descending = !s.descending;
                } else {
                    s.sort = col;
                    s.descending = false;
                }
                self.first_page_and_load()
            }
            Input::Select(id) => {
                self.state.selected = id;
                vec![]
            }
            Input::CreateOrder => vec![Effect::Navigate(NavGraph::to_new_order())],
            Input::EditOrder(id) => vec![Effect::Navigate(NavGraph::to_order_detail(id, false))],
            Input::ViewOrder(id) => vec![Effect::Navigate(NavGraph::to_order_detail(id, true))],
            Input::DeleteOrder(id) => match self.find(id) {
                Some(o) => vec![Effect::ConfirmDelete {
                    id,
                    label: o.order_number.clone(),
                }],
                None => vec![],
            },
            Input::DeleteConfirmed(id) => match self.find(id) {
                Some(o) => vec![Effect::Run(Task::Delete {
                    id,
                    order_number: o.order_number.clone(),
                })],
                None => vec![],
            },
            Input::Deleted(Ok(number)) => {
                let s = &mut self.state;
                s.orders.retain(|o| o.order_number != number);
                s.total_count = s.total_count.saturating_sub(1);
                vec![
                    Effect::Notify(Notice::new(
                        NoticeLevel::Success,
                        Text::KeyWith("ui.order.deleted", vec![number]),
                    )),
                    Effect::DataChanged(DataKind::Orders),
                ]
            }
            Input::Deleted(Err(e)) | Input::StatusChanged(Err(e)) => {
                vec![Effect::Notify(Notice::error(e))]
            }
            Input::ChangeStatus(id) => match self.find(id) {
                Some(o) => vec![Effect::Run(Task::ChangeStatus {
                    id,
                    status: fmt::next_status(o.status),
                })],
                None => vec![],
            },
            Input::StatusChanged(Ok(())) => {
                let mut fx = self.load();
                fx.push(Effect::DataChanged(DataKind::Orders));
                fx
            }
            Input::Export => vec![Effect::Notify(Notice::new(
                NoticeLevel::Info,
                Text::Key("common.developing"),
            ))],
            Input::Print => {
                if self.state.selected.is_none() {
                    vec![Effect::Notify(Notice::new(
                        NoticeLevel::Warning,
                        Text::Key("ui.order.selectToPrint"),
                    ))]
                } else {
                    vec![Effect::Notify(Notice::new(
                        NoticeLevel::Info,
                        Text::Key("common.developing"),
                    ))]
                }
            }
        }
    }

    fn find(&self, id: i32) -> Option<&Order> {
        self.state.orders.iter().find(|o| o.base.id == id)
    }
}
