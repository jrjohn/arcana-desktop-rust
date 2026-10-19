//! Product list + detail (C# `ProductListPage`; detail route `ProductDetailPage`).

use arcana_core::{AppResult, PageRequest, PagedResult};
use arcana_domain::{Product, ProductValidator, Validator};

use super::entity_form::{self, FieldDef, FieldKind, FormKind, Invalid};
use super::entity_list::{self, ListKind};
use crate::effect::DataKind;
use crate::fmt;
use crate::nav::{NavGraph, NavRequest};
use crate::services::UiServices;

/// List configuration.
#[derive(Debug)]
pub struct ProductList;

impl ListKind for ProductList {
    type Row = Product;
    const TITLE_KEY: &'static str = "product.list";
    const NEW_KEY: &'static str = "quick.newProduct";
    const SEARCH_HINT_KEY: &'static str = "ui.product.searchHint";
    const SALT: &'static str = "products";
    const COLUMNS: &'static [&'static str] = &[
        "ui.product.code",
        "ui.product.name",
        "ui.product.unit",
        "ui.product.price",
        "ui.product.stock",
        "ui.active",
    ];
    const DATA: DataKind = DataKind::Products;
    fn id(row: &Product) -> i32 {
        row.base.id
    }
    fn label(row: &Product) -> String {
        row.code.clone()
    }
    fn cells(p: &Product) -> Vec<String> {
        vec![
            p.code.clone(),
            p.name.clone(),
            p.unit.clone(),
            fmt::number(p.price, 0),
            fmt::number(p.stock_quantity, 0),
            if p.is_active { "✔" } else { "" }.to_owned(),
        ]
    }
    fn new_route() -> NavRequest {
        NavGraph::to_new_product()
    }
    fn detail_route(id: i32) -> NavRequest {
        NavGraph::to_product_detail(id)
    }
}

/// Form configuration.
#[derive(Debug)]
pub struct ProductForm;

fn opt(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_owned())
}

fn dec(s: &str) -> Result<rust_decimal::Decimal, Invalid> {
    fmt::parse_decimal(s).ok_or(Invalid)
}

impl FormKind for ProductForm {
    type Entity = Product;
    const TITLE_KEY: &'static str = "product.title";
    const NEW_KEY: &'static str = "quick.newProduct";
    const SALT: &'static str = "product_form";
    const DATA: DataKind = DataKind::Products;
    const FIELDS: &'static [FieldDef] = &[
        FieldDef {
            label_key: "ui.product.code",
            kind: FieldKind::Text,
        },
        FieldDef {
            label_key: "ui.product.name",
            kind: FieldKind::Text,
        },
        FieldDef {
            label_key: "ui.product.description",
            kind: FieldKind::Multiline,
        },
        FieldDef {
            label_key: "ui.product.category",
            kind: FieldKind::Choice,
        },
        FieldDef {
            label_key: "ui.product.unit",
            kind: FieldKind::Text,
        },
        FieldDef {
            label_key: "ui.product.price",
            kind: FieldKind::Decimal,
        },
        FieldDef {
            label_key: "ui.product.cost",
            kind: FieldKind::Decimal,
        },
        FieldDef {
            label_key: "ui.product.stock",
            kind: FieldKind::Decimal,
        },
        FieldDef {
            label_key: "ui.product.minStock",
            kind: FieldKind::Decimal,
        },
        FieldDef {
            label_key: "ui.product.maxStock",
            kind: FieldKind::Decimal,
        },
        FieldDef {
            label_key: "ui.product.barcode",
            kind: FieldKind::Text,
        },
        FieldDef {
            label_key: "ui.product.weight",
            kind: FieldKind::Text,
        },
        FieldDef {
            label_key: "ui.active",
            kind: FieldKind::Bool,
        },
    ];
    fn id(e: &Product) -> i32 {
        e.base.id
    }
    fn label(e: &Product) -> String {
        e.code.clone()
    }
    fn get(p: &Product, i: usize) -> String {
        let n = |d| fmt::number(d, 2).replace(',', "");
        match i {
            0 => p.code.clone(),
            1 => p.name.clone(),
            2 => p.description.clone().unwrap_or_default(),
            3 => p.category_id.map(|c| c.to_string()).unwrap_or_default(),
            4 => p.unit.clone(),
            5 => n(p.price),
            6 => n(p.cost),
            7 => n(p.stock_quantity),
            8 => n(p.min_stock_level),
            9 => n(p.max_stock_level),
            10 => p.barcode.clone().unwrap_or_default(),
            11 => p.weight.map(n).unwrap_or_default(),
            _ => p.is_active.to_string(),
        }
    }
    fn set(p: &mut Product, i: usize, t: &str) -> Result<(), Invalid> {
        match i {
            0 => t.trim().clone_into(&mut p.code),
            1 => t.trim().clone_into(&mut p.name),
            2 => p.description = opt(t),
            3 => {
                p.category_id = if t.trim().is_empty() {
                    None
                } else {
                    Some(t.trim().parse().map_err(|_| Invalid)?)
                }
            }
            4 => t.trim().clone_into(&mut p.unit),
            5 => p.price = dec(t)?,
            6 => p.cost = dec(t)?,
            7 => p.stock_quantity = dec(t)?,
            8 => p.min_stock_level = dec(t)?,
            9 => p.max_stock_level = dec(t)?,
            10 => p.barcode = opt(t),
            11 => {
                p.weight = if t.trim().is_empty() {
                    None
                } else {
                    Some(dec(t)?)
                }
            }
            _ => p.is_active = t == "true",
        }
        Ok(())
    }
    fn validate(p: &Product) -> Vec<String> {
        ProductValidator
            .validate(p)
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
) -> AppResult<PagedResult<Product>> {
    if search.is_empty() {
        let mut r = PageRequest::new(page, size);
        r.sort_by = Some("Code".to_owned());
        svc.products.get_products(&r).await
    } else {
        let items = svc.products.search_products(search, 100).await?;
        let n = items.len() as u64;
        Ok(PagedResult::new(
            items,
            1,
            u32::try_from(n).unwrap_or(u32::MAX).max(1),
            n,
        ))
    }
}

/// Execute a product list task.
pub async fn run_list(task: entity_list::Task, svc: UiServices) -> entity_list::Input<Product> {
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
            svc.products
                .delete_product(id)
                .await
                .map(|()| label)
                .map_err(|e| e.to_user_message()),
        ),
    }
}

async fn load_form(svc: &UiServices, id: Option<i32>) -> AppResult<entity_form::Loaded<Product>> {
    let options = svc
        .products
        .get_categories()
        .await?
        .into_iter()
        .map(|c| (c.base.id, c.name))
        .collect();
    let entity = match id {
        Some(id) => svc.products.get_product_by_id(id).await?,
        None => Product::default(),
    };
    Ok(entity_form::Loaded { entity, options })
}

/// Execute a product form task.
pub async fn run_form(
    task: entity_form::Task<Product>,
    svc: UiServices,
) -> entity_form::Input<Product> {
    use entity_form::{Input, Task};
    let msg = |e: arcana_core::AppError| e.to_user_message();
    match task {
        Task::Load(id) => Input::Loaded(load_form(&svc, id).await.map_err(msg)),
        Task::Save { entity, is_new } => Input::Saved(
            if is_new {
                svc.products.create_product(*entity).await
            } else {
                svc.products.update_product(*entity).await
            }
            .map_err(msg),
        ),
        Task::Delete(id) => Input::Deleted(svc.products.delete_product(id).await.map_err(msg)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_fields_round_trip() {
        let mut p = Product::default();
        for (i, v) in [
            (0, "P009"),
            (1, "Pen"),
            (2, "Blue"),
            (3, "2"),
            (4, "box"),
            (5, "12.5"),
            (6, "8"),
            (7, "100"),
            (8, "10"),
            (9, "500"),
            (10, "4711"),
            (11, "0.25"),
            (12, "false"),
        ] {
            assert!(ProductForm::set(&mut p, i, v).is_ok(), "{i}");
        }
        assert_eq!(ProductForm::get(&p, 0), "P009");
        assert_eq!(ProductForm::get(&p, 3), "2");
        assert_eq!(ProductForm::get(&p, 5), "12.50");
        assert_eq!(ProductForm::get(&p, 11), "0.25");
        assert_eq!(ProductForm::get(&p, 12), "false");
        for i in [2, 10] {
            assert_ne!(ProductForm::get(&p, i).len(), 0);
        }
        assert!(ProductForm::set(&mut p, 5, "abc").is_err());
        assert!(ProductForm::set(&mut p, 3, "x").is_err());
        assert!(ProductForm::set(&mut p, 11, "x").is_err());
        assert!(ProductForm::set(&mut p, 3, "").is_ok());
        assert!(ProductForm::set(&mut p, 11, "").is_ok());
        assert_eq!(ProductForm::get(&p, 11), "");
        assert_eq!(ProductForm::validate(&p).len(), 0);
        assert_ne!(ProductForm::validate(&Product::default()).len(), 0);
        assert_eq!(ProductForm::FIELDS.len(), 13);
        assert_eq!(ProductList::cells(&p).len(), ProductList::COLUMNS.len());
        assert_eq!(ProductList::label(&p), "P009");
    }
}
