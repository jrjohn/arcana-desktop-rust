use super::*;

#[derive(Debug, Clone, PartialEq, Default)]
struct Thing {
    id: i32,
    name: String,
    qty: i64,
    active: bool,
}

#[derive(Debug)]
struct Fake;

impl FormKind for Fake {
    type Entity = Thing;
    const TITLE_KEY: &'static str = "t";
    const NEW_KEY: &'static str = "n";
    const SALT: &'static str = "fake";
    const DATA: DataKind = DataKind::Customers;
    const FIELDS: &'static [FieldDef] = &[
        FieldDef {
            label_key: "name",
            kind: FieldKind::Text,
        },
        FieldDef {
            label_key: "qty",
            kind: FieldKind::Decimal,
        },
        FieldDef {
            label_key: "active",
            kind: FieldKind::Bool,
        },
        FieldDef {
            label_key: "id",
            kind: FieldKind::ReadOnly,
        },
    ];
    fn id(e: &Thing) -> i32 {
        e.id
    }
    fn label(e: &Thing) -> String {
        e.name.clone()
    }
    fn get(e: &Thing, i: usize) -> String {
        match i {
            0 => e.name.clone(),
            1 => e.qty.to_string(),
            2 => e.active.to_string(),
            _ => e.id.to_string(),
        }
    }
    fn set(e: &mut Thing, i: usize, text: &str) -> Result<(), Invalid> {
        match i {
            0 => e.name = text.to_owned(),
            1 => e.qty = text.parse().map_err(|_| Invalid)?,
            2 => e.active = text == "true",
            _ => {}
        }
        Ok(())
    }
    fn validate(e: &Thing) -> Vec<String> {
        if e.name.is_empty() {
            vec!["Name is required".into()]
        } else {
            vec![]
        }
    }
}

fn loaded(vm: &mut ViewModel<Fake>, t: Thing) {
    let _ = vm.on(Input::Loaded(Ok(Loaded {
        entity: t,
        options: vec![(1, "A".into())],
    })));
}

#[test]
fn new_form_validates_then_saves() {
    let (mut vm, fx) = ViewModel::<Fake>::new(None);
    assert_eq!(fx, vec![Effect::Run(Task::Load(None))]);
    assert_eq!(vm.on(Input::Load).len(), 0);
    loaded(&mut vm, Thing::default());
    assert_eq!(vm.state.options.len(), 1);
    let fx = vm.on(Input::Save);
    assert!(matches!(&fx[0], Effect::Notify(n) if n.level == NoticeLevel::Warning));
    assert_eq!(vm.state.validation, vec!["Name is required".to_owned()]);

    let _ = vm.on(Input::FieldChanged(0, "Widget".into()));
    let _ = vm.on(Input::FieldChanged(1, "x".into()));
    assert!(vm.state.is_dirty);
    let fx = vm.on(Input::Save);
    assert!(matches!(&fx[0], Effect::Notify(_)));
    assert_eq!(vm.state.validation, vec!["qty: invalid value".to_owned()]);

    let _ = vm.on(Input::FieldChanged(1, "3".into()));
    let _ = vm.on(Input::FieldChanged(2, "true".into()));
    assert_eq!(vm.on(Input::FieldChanged(3, "9".into())).len(), 0);
    assert_eq!(vm.on(Input::FieldChanged(99, "9".into())).len(), 0);
    let fx = vm.on(Input::Save);
    let Some(Effect::Run(Task::Save {
        entity,
        is_new: true,
    })) = fx.first()
    else {
        panic!("{fx:?}")
    };
    assert_eq!((entity.qty, entity.active), (3, true));
    assert_eq!(vm.on(Input::Save).len(), 0, "busy");
    assert_eq!(vm.on(Input::FieldChanged(0, "x".into())).len(), 0, "busy");
    let mut saved = (**entity).clone();
    saved.id = 5;
    let fx = vm.on(Input::Saved(Ok(saved)));
    assert!(fx.contains(&Effect::DataChanged(DataKind::Customers)));
    assert_eq!(
        (vm.state.id, vm.state.is_new, vm.state.is_dirty),
        (Some(5), false, false)
    );
    assert_eq!(vm.state.texts[3], "5");
}

#[test]
fn save_error_cancel_and_delete() {
    let (mut vm, _) = ViewModel::<Fake>::new(Some(5));
    loaded(
        &mut vm,
        Thing {
            id: 5,
            name: "W".into(),
            ..Thing::default()
        },
    );
    assert_eq!(vm.on(Input::Cancel), vec![Effect::CloseSelf]);
    let _ = vm.on(Input::FieldChanged(0, "W2".into()));
    assert_eq!(vm.on(Input::Cancel), vec![Effect::ConfirmDiscard]);
    assert_eq!(vm.on(Input::DiscardConfirmed), vec![Effect::CloseSelf]);
    let _ = vm.on(Input::Save);
    assert_eq!(
        vm.on(Input::Saved(Err("dup".into()))),
        vec![Effect::Notify(Notice::error("dup"))]
    );
    assert_eq!(
        vm.on(Input::Delete),
        vec![Effect::ConfirmDelete {
            id: 5,
            label: "W2".into()
        }]
    );
    assert_eq!(
        vm.on(Input::DeleteConfirmed(5)),
        vec![Effect::Run(Task::Delete(5))]
    );
    let fx = vm.on(Input::Deleted(Ok(())));
    assert!(fx.contains(&Effect::CloseSelf));
    let fx = vm.on(Input::Deleted(Err("in use".into())));
    assert_eq!(fx, vec![Effect::Notify(Notice::error("in use"))]);
    let fx = vm.on(Input::Loaded(Err("gone".into())));
    assert_eq!(vm.state.error.as_deref(), Some("gone"));
    assert_eq!(fx.len(), 1);

    let (mut fresh, _) = ViewModel::<Fake>::new(None);
    loaded(&mut fresh, Thing::default());
    assert_eq!(fresh.on(Input::Delete).len(), 0, "nothing to delete yet");
}
