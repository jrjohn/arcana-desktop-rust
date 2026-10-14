//! Domain entities (C# `Arcana.Domain.Entities`).

mod base;
mod business;
mod identity;

pub use base::{BaseEntity, BaseGuidEntity, HasBase};
pub use business::{
    Customer, Order, OrderItem, OrderStatus, PaymentMethod, PaymentStatus, Product, ProductCategory,
};
pub use identity::{
    AppPermission, AuditEventType, AuditLog, Role, RolePermission, User, UserPermission, UserRole,
    system_permissions, system_roles,
};
