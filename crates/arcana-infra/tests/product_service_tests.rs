//! C# `ProductServiceTests` (26).

mod common;

use std::sync::Arc;

use arcana_core::{ErrorCode, PageRequest};
use arcana_domain::services::ProductService;
use arcana_domain::{BaseEntity, Product, ProductCategory};
use arcana_infra::ProductServiceImpl;
use common::{Fake, FakeValidator};

fn product(id: i32, code: &str, name: &str) -> Product {
    Product {
        base: BaseEntity::with_id(id),
        code: code.into(),
        name: name.into(),
        category_id: Some(1),
        ..Product::default()
    }
}

/// One fixed instance: entities get a random `SyncId`, so equality needs the same one.
static FIXTURE: std::sync::LazyLock<Product> =
    std::sync::LazyLock::new(|| product(1, "PROD001", "Test Product"));

fn p1() -> Product {
    FIXTURE.clone()
}

fn svc(repo: &Arc<Fake<Product>>) -> ProductServiceImpl {
    ProductServiceImpl::new(repo.clone(), Arc::new(FakeValidator::default()))
}

fn failing(method: &'static str) -> Arc<Fake<Product>> {
    let repo = Fake::with(vec![p1()]);
    repo.fail(method);
    repo
}

// GetProductsAsync_ShouldReturnPagedResult
#[tokio::test]
async fn get_products_should_return_paged_result() {
    let page = svc(&Fake::with(vec![p1()]))
        .get_products(&PageRequest::default())
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
}

// GetProductsAsync_RepositoryThrows_ShouldReturnFailure
#[tokio::test]
async fn get_products_repository_throws_should_return_failure() {
    let e = svc(&failing("get_paged"))
        .get_products(&PageRequest::default())
        .await
        .unwrap_err();
    assert_eq!(
        (e.code(), e.message()),
        (ErrorCode::QueryFailed, "Failed to get products")
    );
}

// GetProductByIdAsync_ExistingProduct_ShouldReturnSuccess
#[tokio::test]
async fn get_product_by_id_existing_product_should_return_success() {
    assert_eq!(
        svc(&Fake::with(vec![p1()]))
            .get_product_by_id(1)
            .await
            .unwrap(),
        p1()
    );
}

// GetProductByIdAsync_NotFound_ShouldReturnFailure
#[tokio::test]
async fn get_product_by_id_not_found_should_return_failure() {
    let e = svc(&Fake::with(vec![]))
        .get_product_by_id(99)
        .await
        .unwrap_err();
    assert_eq!(e.message(), "Product with ID 99 not found");
}

// GetProductByIdAsync_RepositoryThrows_ShouldReturnFailure
#[tokio::test]
async fn get_product_by_id_repository_throws_should_return_failure() {
    let e = svc(&failing("get_by_id"))
        .get_product_by_id(1)
        .await
        .unwrap_err();
    assert_eq!(
        (e.code(), e.message()),
        (ErrorCode::QueryFailed, "Failed to get product")
    );
}

// GetProductByCodeAsync_ExistingCode_ShouldReturnSuccess
#[tokio::test]
async fn get_product_by_code_existing_code_should_return_success() {
    assert_eq!(
        svc(&Fake::with(vec![p1()]))
            .get_product_by_code("PROD001")
            .await
            .unwrap(),
        p1()
    );
}

// GetProductByCodeAsync_NotFound_ShouldReturnFailure
#[tokio::test]
async fn get_product_by_code_not_found_should_return_failure() {
    let e = svc(&Fake::with(vec![]))
        .get_product_by_code("NOTEXIST")
        .await
        .unwrap_err();
    assert_eq!(
        (e.code(), e.message()),
        (ErrorCode::NotFound, "Product NOTEXIST not found")
    );
}

// GetProductByCodeAsync_RepositoryThrows_ShouldReturnFailure
#[tokio::test]
async fn get_product_by_code_repository_throws_should_return_failure() {
    let e = svc(&failing("get_by_code"))
        .get_product_by_code("PROD001")
        .await
        .unwrap_err();
    assert_eq!(e.code(), ErrorCode::QueryFailed);
}

// SearchProductsAsync_ShouldReturnMatchingProducts
#[tokio::test]
async fn search_products_should_return_matching_products() {
    let repo = Fake::with(vec![p1(), product(2, "PROD002", "Test 2")]);
    assert_eq!(
        svc(&repo).search_products("test", 20).await.unwrap().len(),
        2
    );
}

// SearchProductsAsync_RepositoryThrows_ShouldReturnFailure
#[tokio::test]
async fn search_products_repository_throws_should_return_failure() {
    let e = svc(&failing("search"))
        .search_products("test", 20)
        .await
        .unwrap_err();
    assert_eq!(e.message(), "Failed to search products");
}

// GetProductsByCategoryAsync_ShouldReturnProductsInCategory
#[tokio::test]
async fn get_products_by_category_should_return_products_in_category() {
    let mut other = product(2, "PROD002", "Other");
    other.category_id = Some(2);
    let repo = Fake::with(vec![p1(), other]);
    assert_eq!(
        svc(&repo).get_products_by_category(1).await.unwrap(),
        [p1()]
    );
}

// GetProductsByCategoryAsync_RepositoryThrows_ShouldReturnFailure
#[tokio::test]
async fn get_products_by_category_repository_throws_should_return_failure() {
    let e = svc(&failing("get_by_category"))
        .get_products_by_category(1)
        .await
        .unwrap_err();
    assert_eq!(
        (e.code(), e.message()),
        (ErrorCode::QueryFailed, "Failed to get products")
    );
}

// CreateProductAsync_Valid_ShouldReturnSuccess
#[tokio::test]
async fn create_product_valid_should_return_success() {
    let repo = Fake::with(vec![]);
    assert_eq!(svc(&repo).create_product(p1()).await.unwrap(), p1());
    assert_eq!(
        *repo.code_exists_args.lock().unwrap(),
        [("PROD001".to_owned(), None)]
    );
}

// CreateProductAsync_DuplicateCode_ShouldReturnFailure
#[tokio::test]
async fn create_product_duplicate_code_should_return_failure() {
    let repo = Fake::with(vec![]);
    *repo.code_exists.lock().unwrap() = true;
    let e = svc(&repo).create_product(p1()).await.unwrap_err();
    assert_eq!(e.code(), ErrorCode::DuplicateEntry);
    assert_eq!(e.message(), "Product code already exists");
    assert_eq!(e.to_user_message(), "Product code PROD001 already exists");
}

// CreateProductAsync_ValidationFails_ShouldReturnFailure
#[tokio::test]
async fn create_product_validation_fails_should_return_failure() {
    let s = ProductServiceImpl::new(
        Fake::with(vec![]),
        FakeValidator::failing("Name", "Name is required"),
    );
    let e = s.create_product(p1()).await.unwrap_err();
    assert_eq!(
        (e.code(), e.message()),
        (ErrorCode::ValidationFailed, "Product validation failed")
    );
}

// CreateProductAsync_RepositoryThrows_ShouldReturnFailure
#[tokio::test]
async fn create_product_repository_throws_should_return_failure() {
    let repo = Fake::with(vec![]);
    repo.fail("add");
    let e = svc(&repo).create_product(p1()).await.unwrap_err();
    assert_eq!(
        (e.code(), e.message()),
        (ErrorCode::DatabaseError, "Failed to create product")
    );
}

// UpdateProductAsync_Valid_ShouldReturnSuccess
#[tokio::test]
async fn update_product_valid_should_return_success() {
    let repo = Fake::with(vec![p1()]);
    let changed = Product {
        name: "New".into(),
        ..p1()
    };
    assert_eq!(
        svc(&repo).update_product(changed).await.unwrap().name,
        "New"
    );
    assert_eq!(
        *repo.code_exists_args.lock().unwrap(),
        [("PROD001".to_owned(), Some(1))]
    );
}

// UpdateProductAsync_NotFound_ShouldReturnFailure
#[tokio::test]
async fn update_product_not_found_should_return_failure() {
    let e = svc(&Fake::with(vec![]))
        .update_product(p1())
        .await
        .unwrap_err();
    assert_eq!(e.code(), ErrorCode::NotFound);
}

// UpdateProductAsync_DuplicateCode_ShouldReturnFailure
#[tokio::test]
async fn update_product_duplicate_code_should_return_failure() {
    let repo = Fake::with(vec![p1()]);
    *repo.code_exists.lock().unwrap() = true;
    assert_eq!(
        svc(&repo).update_product(p1()).await.unwrap_err().code(),
        ErrorCode::DuplicateEntry
    );
}

// UpdateProductAsync_ValidationFails_ShouldReturnFailure
#[tokio::test]
async fn update_product_validation_fails_should_return_failure() {
    let s = ProductServiceImpl::new(
        Fake::with(vec![p1()]),
        FakeValidator::failing("Price", "bad"),
    );
    assert_eq!(
        s.update_product(p1()).await.unwrap_err().code(),
        ErrorCode::ValidationFailed
    );
}

// UpdateProductAsync_RepositoryThrows_ShouldReturnFailure
#[tokio::test]
async fn update_product_repository_throws_should_return_failure() {
    let e = svc(&failing("update"))
        .update_product(p1())
        .await
        .unwrap_err();
    assert_eq!(
        (e.code(), e.message()),
        (ErrorCode::DatabaseError, "Failed to update product")
    );
}

// DeleteProductAsync_ExistingProduct_ShouldReturnSuccess
#[tokio::test]
async fn delete_product_existing_product_should_return_success() {
    let repo = Fake::with(vec![p1()]);
    svc(&repo).delete_product(1).await.unwrap();
    assert_eq!(repo.items.lock().unwrap().len(), 0);
}

// DeleteProductAsync_NotFound_ShouldReturnFailure
#[tokio::test]
async fn delete_product_not_found_should_return_failure() {
    assert_eq!(
        svc(&Fake::with(vec![]))
            .delete_product(99)
            .await
            .unwrap_err()
            .code(),
        ErrorCode::NotFound
    );
}

// DeleteProductAsync_RepositoryThrows_ShouldReturnFailure
#[tokio::test]
async fn delete_product_repository_throws_should_return_failure() {
    let e = svc(&failing("delete")).delete_product(1).await.unwrap_err();
    assert_eq!(
        (e.code(), e.message()),
        (ErrorCode::DatabaseError, "Failed to delete product")
    );
}

// GetCategoriesAsync_ShouldReturnCategories
#[tokio::test]
async fn get_categories_should_return_categories() {
    let repo = Fake::with(vec![]);
    *repo.categories.lock().unwrap() = vec![
        ProductCategory {
            base: BaseEntity::with_id(1),
            code: "CAT001".into(),
            name: "Electronics".into(),
            ..ProductCategory::default()
        },
        ProductCategory {
            base: BaseEntity::with_id(2),
            code: "CAT002".into(),
            name: "Furniture".into(),
            ..ProductCategory::default()
        },
    ];
    assert_eq!(svc(&repo).get_categories().await.unwrap().len(), 2);
}

// GetCategoriesAsync_RepositoryThrows_ShouldReturnFailure
#[tokio::test]
async fn get_categories_repository_throws_should_return_failure() {
    let e = svc(&failing("get_categories"))
        .get_categories()
        .await
        .unwrap_err();
    assert_eq!(
        (e.code(), e.message()),
        (ErrorCode::QueryFailed, "Failed to get categories")
    );
}
