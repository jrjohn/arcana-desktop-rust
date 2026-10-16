//! Repositories (C# `Arcana.Data.Repository`).

pub(crate) mod generic;
mod specialized;

pub use generic::SqliteRepository;
pub use specialized::{
    CustomerRepository, CustomerRepositoryImpl, OrderRepository, OrderRepositoryImpl,
    ProductRepository, ProductRepositoryImpl, customer_repository, order_repository,
    product_repository,
};
