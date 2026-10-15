//! Persistence on `SQLite` (C# `Arcana.Data`): migrations, row mapping, DAOs, repositories and
//! the unit of work.
//!
//! Queries are built at runtime (`sqlx::query`), never with the compile-time `query!` macros,
//! so building needs no `DATABASE_URL`.

pub mod dao;
pub mod db;
pub mod plugin_store;
pub mod repository;
pub mod row;
pub mod tables;
pub mod unit_of_work;

pub use dao::{
    CustomerDao, CustomerDaoImpl, OrderDao, OrderDaoImpl, OrderFilter, ProductDao, ProductDaoImpl,
};
pub use db::{Db, MIGRATOR};
pub use plugin_store::{
    MAX_HEALTH_SNAPSHOTS, PluginHealthSnapshotRow, PluginMetadataRow, PluginStoreDao,
    PluginVersionRow,
};
pub use repository::{
    CustomerRepository, CustomerRepositoryImpl, OrderRepository, OrderRepositoryImpl,
    ProductRepository, ProductRepositoryImpl, SqliteRepository,
};
pub use row::{FromSql, SqlValue, TableEntity, ToSql};
pub use tables::{SyncOperation, SyncQueueItem, SyncStatus};
pub use unit_of_work::{SqliteTransactionScope, SqliteUnitOfWork, SqliteUnitOfWorkFactory};
