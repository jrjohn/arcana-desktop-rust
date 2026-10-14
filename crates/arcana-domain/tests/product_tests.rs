//! Port of C# `tests/Arcana.Domain.Tests/Entities/ProductTests.cs`
//! (`ProductTests` 19 + `ProductCategoryTests` 7 = 26 tests).

use arcana_domain::{Product, ProductCategory};
use rust_decimal_macros::dec;

fn create_product() -> Product {
    let mut p = Product {
        code: "P001".into(),
        name: "Widget".into(),
        description: Some("A widget".into()),
        unit: "PCS".into(),
        price: dec!(100),
        cost: dec!(60),
        stock_quantity: dec!(50),
        min_stock_level: dec!(5),
        max_stock_level: dec!(200),
        is_active: true,
        ..Product::default()
    };
    p.base.id = 1;
    p
}

fn category(code: &str, name: &str) -> ProductCategory {
    ProductCategory {
        code: code.into(),
        name: name.into(),
        ..ProductCategory::default()
    }
}

/// C#: `Product_Id_ShouldBeSet`
#[test]
fn product_id_should_be_set() {
    assert_eq!(create_product().base.id, 1);
}

/// C#: `Product_Code_ShouldBeSet`
#[test]
fn product_code_should_be_set() {
    assert_eq!(create_product().code, "P001");
}

/// C#: `Product_Name_ShouldBeSet`
#[test]
fn product_name_should_be_set() {
    assert_eq!(create_product().name, "Widget");
}

/// C#: `Product_Description_ShouldBeSet`
#[test]
fn product_description_should_be_set() {
    assert_eq!(create_product().description.as_deref(), Some("A widget"));
}

/// C#: `Product_Unit_DefaultValue_ShouldBePCS`
#[test]
fn product_unit_default_value_should_be_pcs() {
    assert_eq!(Product::default().unit, "PCS");
}

/// C#: `Product_IsActive_DefaultValue_ShouldBeTrue`
#[test]
fn product_is_active_default_value_should_be_true() {
    assert!(Product::default().is_active);
}

/// C#: `Product_Price_ShouldBeSet`
#[test]
fn product_price_should_be_set() {
    assert_eq!(create_product().price, dec!(100));
}

/// C#: `Product_Cost_ShouldBeSet`
#[test]
fn product_cost_should_be_set() {
    assert_eq!(create_product().cost, dec!(60));
}

/// C#: `Product_StockQuantity_ShouldBeSet`
#[test]
fn product_stock_quantity_should_be_set() {
    assert_eq!(create_product().stock_quantity, dec!(50));
}

/// C#: `Product_MinStockLevel_ShouldBeSet`
#[test]
fn product_min_stock_level_should_be_set() {
    assert_eq!(create_product().min_stock_level, dec!(5));
}

/// C#: `Product_MaxStockLevel_ShouldBeSet`
#[test]
fn product_max_stock_level_should_be_set() {
    assert_eq!(create_product().max_stock_level, dec!(200));
}

/// C#: `Product_Barcode_DefaultValue_ShouldBeNull`
#[test]
fn product_barcode_default_value_should_be_null() {
    assert_eq!(Product::default().barcode, None);
}

/// C#: `Product_Barcode_ShouldBeSettable`
#[test]
fn product_barcode_should_be_settable() {
    let mut p = create_product();
    p.barcode = Some("1234567890".into());
    assert_eq!(p.barcode.as_deref(), Some("1234567890"));
}

/// C#: `Product_Weight_DefaultValue_ShouldBeNull`
#[test]
fn product_weight_default_value_should_be_null() {
    assert_eq!(Product::default().weight, None);
}

/// C#: `Product_Weight_ShouldBeSettable`
#[test]
fn product_weight_should_be_settable() {
    let mut p = create_product();
    p.weight = Some(dec!(1.5));
    assert_eq!(p.weight, Some(dec!(1.5)));
}

/// C#: `Product_CategoryId_DefaultValue_ShouldBeNull`
#[test]
fn product_category_id_default_value_should_be_null() {
    assert_eq!(Product::default().category_id, None);
}

/// C#: `Product_CategoryId_ShouldBeSettable`
#[test]
fn product_category_id_should_be_settable() {
    let mut p = create_product();
    p.category_id = Some(5);
    assert_eq!(p.category_id, Some(5));
}

/// C#: `Product_Code_DefaultValue_ShouldBeEmptyString`
#[test]
fn product_code_default_value_should_be_empty_string() {
    assert_eq!(Product::default().code, "");
}

/// C#: `Product_Name_DefaultValue_ShouldBeEmptyString`
#[test]
fn product_name_default_value_should_be_empty_string() {
    assert_eq!(Product::default().name, "");
}

// ─── ProductCategoryTests ───────────────────────────────────────────────────

/// C#: `ProductCategory_Code_ShouldBeSet`
#[test]
fn product_category_code_should_be_set() {
    assert_eq!(category("CAT001", "Electronics").code, "CAT001");
}

/// C#: `ProductCategory_Name_ShouldBeSet`
#[test]
fn product_category_name_should_be_set() {
    assert_eq!(category("CAT001", "Electronics").name, "Electronics");
}

/// C#: `ProductCategory_ParentId_DefaultValue_ShouldBeNull`
#[test]
fn product_category_parent_id_default_value_should_be_null() {
    assert_eq!(category("C", "Cat").parent_id, None);
}

/// C#: `ProductCategory_SortOrder_DefaultValue_ShouldBeZero`
#[test]
fn product_category_sort_order_default_value_should_be_zero() {
    assert_eq!(category("C", "Cat").sort_order, 0);
}

/// C#: `ProductCategory_Children_DefaultValue_ShouldBeEmpty`
#[test]
fn product_category_children_default_value_should_be_empty() {
    assert!(category("C", "Cat").children.is_empty());
}

/// C#: `ProductCategory_Products_DefaultValue_ShouldBeEmpty`
#[test]
fn product_category_products_default_value_should_be_empty() {
    assert!(category("C", "Cat").products.is_empty());
}

/// C#: `ProductCategory_Parent_ShouldBeSettable`
#[test]
fn product_category_parent_should_be_settable() {
    let parent = category("P", "Parent");
    let mut child = category("C", "Child");
    child.parent_id = Some(parent.base.id);
    child.parent = Some(Box::new(parent.clone()));
    assert_eq!(child.parent.as_deref(), Some(&parent));
}
