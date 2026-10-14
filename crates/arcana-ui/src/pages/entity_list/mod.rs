//! Searchable, paged entity list shared by the product and customer list pages.
//!
//! C# `ProductListPage` / `CustomerListPage` are placeholders ("Under development…"). The
//! port gives them a working list over the existing services with the same toolbar shape as
//! the order list; columns are the entity's main fields.

#[cfg(test)]
mod tests;
mod view;

pub use view::show;

use std::marker::PhantomData;

use arcana_core::PagedResult;

use crate::effect::{DataKind, Effect, Notice, NoticeLevel, Text};
use crate::nav::NavRequest;

/// Rows per page.
pub const PAGE_SIZE: u32 = 20;

/// What differs between the product and customer lists.
pub trait ListKind: 'static {
    type Row: Clone + std::fmt::Debug + PartialEq;
    const TITLE_KEY: &'static str;
    const NEW_KEY: &'static str;
    const SEARCH_HINT_KEY: &'static str;
    const SALT: &'static str;
    /// Column header keys, parallel to [`ListKind::cells`].
    const COLUMNS: &'static [&'static str];
    const DATA: DataKind;
    fn id(row: &Self::Row) -> i32;
    fn label(row: &Self::Row) -> String;
    fn cells(row: &Self::Row) -> Vec<String>;
    fn new_route() -> NavRequest;
    fn detail_route(id: i32) -> NavRequest;
}

#[derive(Debug, Clone, PartialEq)]
pub struct State<R> {
    pub rows: Vec<R>,
    pub current_page: u32,
    pub total_pages: u64,
    pub total_count: u64,
    pub search_text: String,
    pub is_loading: bool,
    pub error: Option<String>,
}

impl<R> Default for State<R> {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            current_page: 1,
            total_pages: 1,
            total_count: 0,
            search_text: String::new(),
            is_loading: false,
            error: None,
        }
    }
}

impl<R> State<R> {
    #[must_use]
    pub fn can_go_previous(&self) -> bool {
        self.current_page > 1
    }
    #[must_use]
    pub fn can_go_next(&self) -> bool {
        u64::from(self.current_page) < self.total_pages
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Input<R> {
    Load,
    Loaded(Result<PagedResult<R>, String>),
    Refresh,
    SearchTextChanged(String),
    Search,
    PreviousPage,
    NextPage,
    New,
    Open(i32),
    Delete(i32),
    DeleteConfirmed(i32),
    Deleted(Result<String, String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Task {
    /// Empty `search` pages through everything; otherwise one page of search results.
    Load {
        page: u32,
        page_size: u32,
        search: String,
    },
    Delete {
        id: i32,
        label: String,
    },
}

/// The list view model, parameterised by kind.
#[derive(Debug)]
pub struct ViewModel<K: ListKind> {
    pub state: State<K::Row>,
    kind: PhantomData<K>,
}

impl<K: ListKind> Default for ViewModel<K> {
    fn default() -> Self {
        Self {
            state: State::default(),
            kind: PhantomData,
        }
    }
}

impl<K: ListKind> ViewModel<K> {
    #[must_use]
    pub fn new() -> (Self, Vec<Effect<Task>>) {
        let mut vm = Self::default();
        let fx = vm.on(Input::Load);
        (vm, fx)
    }

    fn load(&mut self) -> Vec<Effect<Task>> {
        if self.state.is_loading {
            return vec![];
        }
        self.state.is_loading = true;
        self.state.error = None;
        vec![Effect::Run(Task::Load {
            page: self.state.current_page,
            page_size: PAGE_SIZE,
            search: self.state.search_text.trim().to_owned(),
        })]
    }

    pub fn on(&mut self, input: Input<K::Row>) -> Vec<Effect<Task>> {
        match input {
            Input::Load | Input::Refresh => self.load(),
            Input::Loaded(Ok(page)) => {
                let s = &mut self.state;
                s.is_loading = false;
                s.total_count = page.total_count;
                s.total_pages = page.total_pages().max(1);
                s.rows = page.items;
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
            Input::Search => {
                self.state.current_page = 1;
                self.load()
            }
            Input::PreviousPage if self.state.can_go_previous() => {
                self.state.current_page -= 1;
                self.load()
            }
            Input::NextPage if self.state.can_go_next() => {
                self.state.current_page += 1;
                self.load()
            }
            Input::PreviousPage | Input::NextPage => vec![],
            Input::New => vec![Effect::Navigate(K::new_route())],
            Input::Open(id) => vec![Effect::Navigate(K::detail_route(id))],
            Input::Delete(id) => self
                .find(id)
                .map(|r| {
                    vec![Effect::ConfirmDelete {
                        id,
                        label: K::label(r),
                    }]
                })
                .unwrap_or_default(),
            Input::DeleteConfirmed(id) => self
                .find(id)
                .map(|r| {
                    vec![Effect::Run(Task::Delete {
                        id,
                        label: K::label(r),
                    })]
                })
                .unwrap_or_default(),
            Input::Deleted(Ok(label)) => {
                self.state.rows.retain(|r| K::label(r) != label);
                self.state.total_count = self.state.total_count.saturating_sub(1);
                vec![
                    Effect::Notify(Notice::new(
                        NoticeLevel::Success,
                        Text::KeyWith("ui.deleted", vec![label]),
                    )),
                    Effect::DataChanged(K::DATA),
                ]
            }
            Input::Deleted(Err(e)) => vec![Effect::Notify(Notice::error(e))],
        }
    }

    fn find(&self, id: i32) -> Option<&K::Row> {
        self.state.rows.iter().find(|r| K::id(r) == id)
    }
}
