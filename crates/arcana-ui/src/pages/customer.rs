//! Customer list + detail (C# `CustomerListPage`; detail route `CustomerDetailPage`).

use arcana_core::{AppResult, PageRequest, PagedResult};
use arcana_domain::{Customer, CustomerValidator, Validator};

use super::entity_form::{self, FieldDef, FieldKind, FormKind, Invalid};
use super::entity_list::{self, ListKind};
use crate::effect::DataKind;
use crate::fmt;
use crate::nav::{NavGraph, NavRequest};
use crate::services::UiServices;

/// List configuration.
#[derive(Debug)]
pub struct CustomerList;

impl ListKind for CustomerList {
    type Row = Customer;
    const TITLE_KEY: &'static str = "customer.list";
    const NEW_KEY: &'static str = "quick.newCustomer";
    const SEARCH_HINT_KEY: &'static str = "ui.customer.searchHint";
    const SALT: &'static str = "customers";
    const COLUMNS: &'static [&'static str] = &[
        "ui.customer.code",
        "ui.customer.name",
        "ui.customer.contact",
        "ui.customer.phone",
        "ui.customer.email",
        "ui.customer.city",
        "ui.customer.creditLimit",
        "ui.active",
    ];
    const DATA: DataKind = DataKind::Customers;
    fn id(row: &Customer) -> i32 {
        row.base.id
    }
    fn label(row: &Customer) -> String {
        row.code.clone()
    }
    fn cells(c: &Customer) -> Vec<String> {
        let o = |v: &Option<String>| v.clone().unwrap_or_default();
        vec![
            c.code.clone(),
            c.name.clone(),
            o(&c.contact_name),
            o(&c.phone),
            o(&c.email),
            o(&c.city),
            fmt::number(c.credit_limit, 0),
            if c.is_active { "✔" } else { "" }.to_owned(),
        ]
    }
    fn new_route() -> NavRequest {
        NavGraph::to_new_customer()
    }
    fn detail_route(id: i32) -> NavRequest {
        NavGraph::to_customer_detail(id)
    }
}

/// Form configuration.
#[derive(Debug)]
pub struct CustomerForm;

fn opt(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_owned())
}

const fn text(label_key: &'static str) -> FieldDef {
    FieldDef {
        label_key,
        kind: FieldKind::Text,
    }
}

impl FormKind for CustomerForm {
    type Entity = Customer;
    const TITLE_KEY: &'static str = "customer.title";
    const NEW_KEY: &'static str = "quick.newCustomer";
    const SALT: &'static str = "customer_form";
    const DATA: DataKind = DataKind::Customers;
    const FIELDS: &'static [FieldDef] = &[
        text("ui.customer.code"),
        text("ui.customer.name"),
        text("ui.customer.contact"),
        text("ui.customer.phone"),
        text("ui.customer.email"),
        text("ui.customer.address"),
        text("ui.customer.city"),
        text("ui.customer.state"),
        text("ui.customer.postalCode"),
        text("ui.customer.country"),
        text("ui.customer.taxId"),
        FieldDef {
            label_key: "ui.customer.creditLimit",
            kind: FieldKind::Decimal,
        },
        FieldDef {
            label_key: "ui.customer.balance",
            kind: FieldKind::ReadOnly,
        },
        FieldDef {
            label_key: "ui.active",
            kind: FieldKind::Bool,
        },
        FieldDef {
            label_key: "ui.customer.notes",
            kind: FieldKind::Multiline,
        },
    ];
    fn id(e: &Customer) -> i32 {
        e.base.id
    }
    fn label(e: &Customer) -> String {
        e.code.clone()
    }
    fn get(c: &Customer, i: usize) -> String {
        let o = |v: &Option<String>| v.clone().unwrap_or_default();
        match i {
            0 => c.code.clone(),
            1 => c.name.clone(),
            2 => o(&c.contact_name),
            3 => o(&c.phone),
            4 => o(&c.email),
            5 => o(&c.address),
            6 => o(&c.city),
            7 => o(&c.state),
            8 => o(&c.postal_code),
            9 => o(&c.country),
            10 => o(&c.tax_id),
            11 => fmt::number(c.credit_limit, 2).replace(',', ""),
            12 => fmt::number(c.balance, 2),
            13 => c.is_active.to_string(),
            _ => o(&c.notes),
        }
    }
    fn set(c: &mut Customer, i: usize, t: &str) -> Result<(), Invalid> {
        match i {
            0 => t.trim().clone_into(&mut c.code),
            1 => t.trim().clone_into(&mut c.name),
            2 => c.contact_name = opt(t),
            3 => c.phone = opt(t),
            4 => c.email = opt(t),
            5 => c.address = opt(t),
            6 => c.city = opt(t),
            7 => c.state = opt(t),
            8 => c.postal_code = opt(t),
            9 => c.country = opt(t),
            10 => c.tax_id = opt(t),
            11 => c.credit_limit = fmt::parse_decimal(t).ok_or(Invalid)?,
            12 => {}
            13 => c.is_active = t == "true",
            _ => c.notes = opt(t),
        }
        Ok(())
    }
    fn validate(c: &Customer) -> Vec<String> {
        CustomerValidator
            .validate(c)
            .errors
            .into_iter()
            .map(|e| e.error_message)
            .collect()
    }
}

async fn load_list(
    svc: &UiServices,
    page: u32,
    size: u32,
    search: &str,
) -> AppResult<PagedResult<Customer>> {
    if search.is_empty() {
        let mut r = PageRequest::new(page, size);
        r.sort_by = Some("Code".to_owned());
        svc.customers.get_customers(&r).await
    } else {
        let items = svc.customers.search_customers(search, 100).await?;
        let n = items.len() as u64;
        Ok(PagedResult::new(
            items,
            1,
            u32::try_from(n).unwrap_or(u32::MAX).max(1),
            n,
        ))
    }
}

/// Execute a customer list task.
pub async fn run_list(task: entity_list::Task, svc: UiServices) -> entity_list::Input<Customer> {
    use entity_list::{Input, Task};
    match task {
        Task::Load {
            page,
            page_size,
            search,
        } => Input::Loaded(
            load_list(&svc, page, page_size, &search)
                .await
                .map_err(|e| e.to_user_message()),
        ),
        Task::Delete { id, label } => Input::Deleted(
            svc.customers
                .delete_customer(id)
                .await
                .map(|()| label)
                .map_err(|e| e.to_user_message()),
        ),
    }
}

async fn load_form(svc: &UiServices, id: Option<i32>) -> AppResult<entity_form::Loaded<Customer>> {
    let entity = match id {
        Some(id) => svc.customers.get_customer_by_id(id).await?,
        None => Customer::default(),
    };
    Ok(entity_form::Loaded {
        entity,
        options: Vec::new(),
    })
}

/// Execute a customer form task.
pub async fn run_form(
    task: entity_form::Task<Customer>,
    svc: UiServices,
) -> entity_form::Input<Customer> {
    use entity_form::{Input, Task};
    let msg = |e: arcana_core::AppError| e.to_user_message();
    match task {
        Task::Load(id) => Input::Loaded(load_form(&svc, id).await.map_err(msg)),
        Task::Save { entity, is_new } => Input::Saved(
            if is_new {
                svc.customers.create_customer(*entity).await
            } else {
                svc.customers.update_customer(*entity).await
            }
            .map_err(msg),
        ),
        Task::Delete(id) => Input::Deleted(svc.customers.delete_customer(id).await.map_err(msg)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_fields_round_trip() {
        let mut c = Customer::default();
        let values = [
            "C009", "Acme", "Ann", "02-1", "a@b.c", "1 Rd", "Taipei", "TP", "110", "TW", "1234",
            "5,000", "ignored", "true", "VIP",
        ];
        for (i, v) in values.iter().enumerate() {
            assert!(CustomerForm::set(&mut c, i, v).is_ok(), "{i}");
        }
        for i in 0..CustomerForm::FIELDS.len() {
            assert_ne!(CustomerForm::get(&c, i).len(), 0, "{i}");
        }
        assert_eq!(CustomerForm::get(&c, 11), "5000.00");
        assert!(CustomerForm::set(&mut c, 11, "lots").is_err());
        assert_eq!(CustomerForm::validate(&c).len(), 0);
        assert_ne!(CustomerForm::validate(&Customer::default()).len(), 0);
        assert_eq!(CustomerList::cells(&c).len(), CustomerList::COLUMNS.len());
        assert_eq!(CustomerList::label(&c), "C009");
        assert_eq!(CustomerList::id(&c), 0);
    }
}
