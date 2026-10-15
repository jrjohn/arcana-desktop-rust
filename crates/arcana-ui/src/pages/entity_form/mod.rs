//! Create / edit form shared by the product and customer detail pages.
//!
//! C# has no product or customer detail page (the routes exist, the pages do not). The port
//! adds a form over the entity's fields, validated with the domain validator before saving.

#[cfg(test)]
mod tests;
mod view;

pub use view::show;

use std::marker::PhantomData;

use crate::effect::{DataKind, Effect, Notice, NoticeLevel, Text};

/// How a field is edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    Multiline,
    Decimal,
    Bool,
    /// Pick one of the loaded options (stored as the option id, empty = none).
    Choice,
    ReadOnly,
}

/// One form field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldDef {
    pub label_key: &'static str,
    pub kind: FieldKind,
}

/// A field text that does not parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Invalid;

/// What differs between the product and customer forms.
pub trait FormKind: 'static {
    type Entity: Clone + std::fmt::Debug + PartialEq + Default;
    const TITLE_KEY: &'static str;
    const NEW_KEY: &'static str;
    const SALT: &'static str;
    const DATA: DataKind;
    const FIELDS: &'static [FieldDef];
    fn id(e: &Self::Entity) -> i32;
    fn label(e: &Self::Entity) -> String;
    /// Field `i` as text (`"true"`/`"false"` for bools, option id for choices).
    fn get(e: &Self::Entity, i: usize) -> String;
    /// Store text into field `i`; `Err` when it does not parse.
    ///
    /// # Errors
    /// The text is not valid for the field (e.g. a bad number).
    fn set(e: &mut Self::Entity, i: usize, text: &str) -> Result<(), Invalid>;
    /// Domain validation messages.
    fn validate(e: &Self::Entity) -> Vec<String>;
}

/// A loaded entity plus choice options (`(id, name)`).
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded<E> {
    pub entity: E,
    pub options: Vec<(i32, String)>,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq)]
pub struct State<E> {
    pub id: Option<i32>,
    pub entity: E,
    pub texts: Vec<String>,
    pub options: Vec<(i32, String)>,
    pub is_new: bool,
    pub is_dirty: bool,
    pub is_busy: bool,
    pub validation: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Input<E> {
    Load,
    Loaded(Result<Loaded<E>, String>),
    FieldChanged(usize, String),
    Save,
    Saved(Result<E, String>),
    Cancel,
    DiscardConfirmed,
    Delete,
    DeleteConfirmed(i32),
    Deleted(Result<(), String>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Task<E> {
    Load(Option<i32>),
    Save { entity: Box<E>, is_new: bool },
    Delete(i32),
}

#[derive(Debug)]
pub struct ViewModel<K: FormKind> {
    pub state: State<K::Entity>,
    kind: PhantomData<K>,
}

impl<K: FormKind> ViewModel<K> {
    /// `id = None` creates a new entity.
    #[must_use]
    pub fn new(id: Option<i32>) -> (Self, Vec<Effect<Task<K::Entity>>>) {
        let entity = K::Entity::default();
        let texts = (0..K::FIELDS.len()).map(|i| K::get(&entity, i)).collect();
        let mut vm = Self {
            state: State {
                id,
                entity,
                texts,
                options: Vec::new(),
                is_new: id.is_none(),
                is_dirty: false,
                is_busy: false,
                validation: Vec::new(),
                error: None,
            },
            kind: PhantomData,
        };
        let fx = vm.on(Input::Load);
        (vm, fx)
    }

    fn sync(&mut self) {
        let e = &self.state.entity;
        self.state.texts = (0..K::FIELDS.len()).map(|i| K::get(e, i)).collect();
    }

    #[allow(clippy::too_many_lines)]
    pub fn on(&mut self, input: Input<K::Entity>) -> Vec<Effect<Task<K::Entity>>> {
        let s = &mut self.state;
        match input {
            Input::Load => {
                if s.is_busy {
                    return vec![];
                }
                s.is_busy = true;
                s.error = None;
                vec![Effect::Run(Task::Load(s.id))]
            }
            Input::Loaded(Ok(l)) => {
                s.is_busy = false;
                s.entity = l.entity;
                s.options = l.options;
                s.is_dirty = false;
                s.validation.clear();
                self.sync();
                vec![]
            }
            Input::Loaded(Err(e)) | Input::Deleted(Err(e)) => {
                s.is_busy = false;
                s.error = Some(e.clone());
                vec![Effect::Notify(Notice::error(e))]
            }
            Input::FieldChanged(i, text) => {
                let Some(def) = K::FIELDS.get(i) else {
                    return vec![];
                };
                if def.kind == FieldKind::ReadOnly || s.is_busy {
                    return vec![];
                }
                // A text that does not parse stays in the box; save reports it.
                let _ = K::set(&mut s.entity, i, &text);
                s.texts[i] = text;
                s.is_dirty = true;
                vec![]
            }
            Input::Save => {
                if s.is_busy {
                    return vec![];
                }
                let mut errors: Vec<String> = Vec::new();
                for (i, def) in K::FIELDS.iter().enumerate() {
                    if K::set(&mut s.entity, i, &s.texts[i]).is_err() {
                        errors.push(format!("{}: invalid value", def.label_key));
                    }
                }
                errors.extend(K::validate(&s.entity));
                s.validation = errors;
                if let Some(first) = s.validation.first() {
                    return vec![Effect::Notify(Notice::new(
                        NoticeLevel::Warning,
                        Text::Raw(first.clone()),
                    ))];
                }
                s.is_busy = true;
                vec![Effect::Run(Task::Save {
                    entity: Box::new(s.entity.clone()),
                    is_new: s.is_new,
                })]
            }
            Input::Saved(Ok(e)) => {
                s.is_busy = false;
                s.id = Some(K::id(&e));
                s.entity = e;
                s.is_new = false;
                s.is_dirty = false;
                self.sync();
                vec![
                    Effect::Notify(Notice::new(NoticeLevel::Success, Text::Key("ui.saved"))),
                    Effect::DataChanged(K::DATA),
                ]
            }
            Input::Saved(Err(e)) => {
                s.is_busy = false;
                s.validation = vec![e.clone()];
                vec![Effect::Notify(Notice::error(e))]
            }
            Input::Cancel => {
                if s.is_dirty {
                    vec![Effect::ConfirmDiscard]
                } else {
                    vec![Effect::CloseSelf]
                }
            }
            Input::DiscardConfirmed => vec![Effect::CloseSelf],
            Input::Delete => match s.id {
                Some(id) if !s.is_new => vec![Effect::ConfirmDelete {
                    id,
                    label: K::label(&s.entity),
                }],
                _ => vec![],
            },
            Input::DeleteConfirmed(id) => vec![Effect::Run(Task::Delete(id))],
            Input::Deleted(Ok(())) => vec![
                Effect::Notify(Notice::new(
                    NoticeLevel::Success,
                    Text::KeyWith("ui.deleted", vec![K::label(&s.entity)]),
                )),
                Effect::DataChanged(K::DATA),
                Effect::CloseSelf,
            ],
        }
    }
}
