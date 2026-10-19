//! C# `CustomerServiceTests` (22).

mod common;

use std::sync::Arc;

use arcana_core::{ErrorCode, PageRequest};
use arcana_domain::services::CustomerService;
use arcana_domain::{BaseEntity, Customer};
use arcana_infra::CustomerServiceImpl;
use common::{Fake, FakeValidator};

fn customer(id: i32, code: &str, name: &str) -> Customer {
    Customer {
        base: BaseEntity::with_id(id),
        code: code.into(),
        name: name.into(),
        ..Customer::default()
    }
}

/// One fixed instance: entities get a random `SyncId`, so equality needs the same one.
static FIXTURE: std::sync::LazyLock<Customer> =
    std::sync::LazyLock::new(|| customer(1, "CUST001", "Test Customer"));

fn c1() -> Customer {
    FIXTURE.clone()
}

fn svc(repo: &Arc<Fake<Customer>>) -> CustomerServiceImpl {
    CustomerServiceImpl::new(repo.clone(), Arc::new(FakeValidator::default()))
}

// GetCustomerByIdAsync_ExistingCustomer_ShouldReturnSuccess
#[tokio::test]
async fn get_customer_by_id_existing_customer_should_return_success() {
    let repo = Fake::with(vec![c1()]);
    assert_eq!(svc(&repo).get_customer_by_id(1).await.unwrap(), c1());
}

// GetCustomerByIdAsync_NotFound_ShouldReturnFailure
#[tokio::test]
async fn get_customer_by_id_not_found_should_return_failure() {
    let e = svc(&Fake::with(vec![]))
        .get_customer_by_id(99)
        .await
        .unwrap_err();
    assert_eq!(e.code(), ErrorCode::NotFound);
    assert_eq!(e.message(), "Customer with ID 99 not found");
}

// GetCustomerByIdAsync_RepositoryThrows_ShouldReturnFailure
#[tokio::test]
async fn get_customer_by_id_repository_throws_should_return_failure() {
    let repo = Fake::with(vec![c1()]);
    repo.fail("get_by_id");
    let e = svc(&repo).get_customer_by_id(1).await.unwrap_err();
    assert_eq!(e.code(), ErrorCode::QueryFailed);
    assert_eq!(e.message(), "Failed to get customer");
}

// GetCustomerByCodeAsync_ExistingCode_ShouldReturnSuccess
#[tokio::test]
async fn get_customer_by_code_existing_code_should_return_success() {
    let repo = Fake::with(vec![c1()]);
    assert_eq!(
        svc(&repo).get_customer_by_code("CUST001").await.unwrap(),
        c1()
    );
}

// GetCustomerByCodeAsync_NotFound_ShouldReturnFailure
#[tokio::test]
async fn get_customer_by_code_not_found_should_return_failure() {
    let e = svc(&Fake::with(vec![]))
        .get_customer_by_code("NOTEXIST")
        .await
        .unwrap_err();
    assert_eq!(e.message(), "Customer NOTEXIST not found");
}

// GetCustomerByCodeAsync_RepositoryThrows_ShouldReturnFailure
#[tokio::test]
async fn get_customer_by_code_repository_throws_should_return_failure() {
    let repo = Fake::with(vec![]);
    repo.fail("get_by_code");
    let e = svc(&repo)
        .get_customer_by_code("CUST001")
        .await
        .unwrap_err();
    assert_eq!(e.code(), ErrorCode::QueryFailed);
}

// GetCustomersAsync_ShouldReturnPagedResult
#[tokio::test]
async fn get_customers_should_return_paged_result() {
    let page = svc(&Fake::with(vec![c1()]))
        .get_customers(&PageRequest::default())
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
}

// GetCustomersAsync_RepositoryThrows_ShouldReturnFailure
#[tokio::test]
async fn get_customers_repository_throws_should_return_failure() {
    let repo = Fake::with(vec![]);
    repo.fail("get_paged");
    let e = svc(&repo)
        .get_customers(&PageRequest::default())
        .await
        .unwrap_err();
    assert_eq!(e.message(), "Failed to get customers");
    assert!(matches!(e, arcana_core::AppError::Data { inner: Some(ref i), .. } if i == "db error"));
}

// SearchCustomersAsync_ShouldReturnMatchingCustomers
#[tokio::test]
async fn search_customers_should_return_matching_customers() {
    let repo = Fake::with(vec![c1(), customer(2, "CUST002", "Test 2")]);
    assert_eq!(
        svc(&repo).search_customers("test", 20).await.unwrap().len(),
        2
    );
}

// SearchCustomersAsync_RepositoryThrows_ShouldReturnFailure
#[tokio::test]
async fn search_customers_repository_throws_should_return_failure() {
    let repo = Fake::with(vec![]);
    repo.fail("search");
    let e = svc(&repo).search_customers("test", 20).await.unwrap_err();
    assert_eq!(e.message(), "Failed to search customers");
}

// CreateCustomerAsync_Valid_ShouldReturnSuccess
#[tokio::test]
async fn create_customer_valid_should_return_success() {
    let repo = Fake::with(vec![]);
    assert_eq!(svc(&repo).create_customer(c1()).await.unwrap(), c1());
    assert_eq!(
        *repo.code_exists_args.lock().unwrap(),
        [("CUST001".to_owned(), None)]
    );
    assert!(repo.called("add"));
}

// CreateCustomerAsync_DuplicateCode_ShouldReturnFailure
#[tokio::test]
async fn create_customer_duplicate_code_should_return_failure() {
    let repo = Fake::with(vec![]);
    *repo.code_exists.lock().unwrap() = true;
    let e = svc(&repo).create_customer(c1()).await.unwrap_err();
    assert_eq!(e.code(), ErrorCode::DuplicateEntry);
    assert_eq!(e.to_user_message(), "Customer code CUST001 already exists");
    assert!(!repo.called("add"));
}

// CreateCustomerAsync_ValidationFails_ShouldReturnFailure
#[tokio::test]
async fn create_customer_validation_fails_should_return_failure() {
    let repo = Fake::with(vec![]);
    let s = CustomerServiceImpl::new(
        repo.clone(),
        FakeValidator::failing("Name", "Name is required"),
    );
    let e = s.create_customer(c1()).await.unwrap_err();
    assert_eq!(e.code(), ErrorCode::ValidationFailed);
    assert_eq!(e.message(), "Customer validation failed");
    assert_eq!(e.to_user_message(), "Name is required");
}

// CreateCustomerAsync_RepositoryThrows_ShouldReturnFailure
#[tokio::test]
async fn create_customer_repository_throws_should_return_failure() {
    let repo = Fake::with(vec![]);
    repo.fail("add");
    let e = svc(&repo).create_customer(c1()).await.unwrap_err();
    assert_eq!(e.code(), ErrorCode::DatabaseError);
    assert_eq!(e.message(), "Failed to create customer");
}

// UpdateCustomerAsync_Valid_ShouldReturnSuccess
#[tokio::test]
async fn update_customer_valid_should_return_success() {
    let repo = Fake::with(vec![c1()]);
    let renamed = Customer {
        name: "Renamed".into(),
        ..c1()
    };
    assert_eq!(
        svc(&repo).update_customer(renamed).await.unwrap().name,
        "Renamed"
    );
    assert_eq!(
        *repo.code_exists_args.lock().unwrap(),
        [("CUST001".to_owned(), Some(1))]
    );
    assert_eq!(repo.items.lock().unwrap()[0].name, "Renamed");
}

// UpdateCustomerAsync_NotFound_ShouldReturnFailure
#[tokio::test]
async fn update_customer_not_found_should_return_failure() {
    let e = svc(&Fake::with(vec![]))
        .update_customer(c1())
        .await
        .unwrap_err();
    assert_eq!(e.code(), ErrorCode::NotFound);
}

// UpdateCustomerAsync_DuplicateCode_ShouldReturnFailure
#[tokio::test]
async fn update_customer_duplicate_code_should_return_failure() {
    let repo = Fake::with(vec![c1()]);
    *repo.code_exists.lock().unwrap() = true;
    let e = svc(&repo).update_customer(c1()).await.unwrap_err();
    assert_eq!(e.code(), ErrorCode::DuplicateEntry);
}

// DeleteCustomerAsync_ExistingCustomer_ShouldReturnSuccess
#[tokio::test]
async fn delete_customer_existing_customer_should_return_success() {
    let repo = Fake::with(vec![c1()]);
    svc(&repo).delete_customer(1).await.unwrap();
    assert_eq!(repo.items.lock().unwrap().len(), 0);
}

// DeleteCustomerAsync_NotFound_ShouldReturnFailure
#[tokio::test]
async fn delete_customer_not_found_should_return_failure() {
    let e = svc(&Fake::with(vec![]))
        .delete_customer(99)
        .await
        .unwrap_err();
    assert_eq!(e.code(), ErrorCode::NotFound);
}

// DeleteCustomerAsync_RepositoryThrows_ShouldReturnFailure
#[tokio::test]
async fn delete_customer_repository_throws_should_return_failure() {
    let repo = Fake::with(vec![c1()]);
    repo.fail("delete");
    let e = svc(&repo).delete_customer(1).await.unwrap_err();
    assert_eq!(e.code(), ErrorCode::DatabaseError);
    assert_eq!(e.message(), "Failed to delete customer");
}

// CustomerCodeExistsAsync_ExistingCode_ShouldReturnTrue
#[tokio::test]
async fn customer_code_exists_existing_code_should_return_true() {
    let repo = Fake::with(vec![]);
    *repo.code_exists.lock().unwrap() = true;
    assert!(
        svc(&repo)
            .customer_code_exists("CUST001", None)
            .await
            .unwrap()
    );
}

// CustomerCodeExistsAsync_WithExclude_ShouldPassThroughToRepo
#[tokio::test]
async fn customer_code_exists_with_exclude_should_pass_through_to_repo() {
    let repo = Fake::with(vec![]);
    assert!(
        !svc(&repo)
            .customer_code_exists("CUST001", Some(5))
            .await
            .unwrap()
    );
    assert_eq!(
        *repo.code_exists_args.lock().unwrap(),
        [("CUST001".to_owned(), Some(5))]
    );
}
