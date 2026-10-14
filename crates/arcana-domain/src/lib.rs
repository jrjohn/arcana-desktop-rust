//! Entities, validators and domain service traits (C# `Arcana.Domain`).

pub mod entities;
pub mod services;
pub mod validation;

pub use entities::*;
pub use validation::{
    CustomerValidator, OrderItemValidator, OrderValidator, ProductValidator, ValidationFailure,
    ValidationResult, Validator,
};
