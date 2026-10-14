//! Security implementations (C# `Arcana.Infrastructure.Security`).

mod audit;
mod auth_service;
mod authorization;
mod current_user;
pub(crate) mod identity;
mod password_hasher;
mod token_service;

pub use audit::{AuditEntry, AuditLogger};
pub use auth_service::{AuthServiceImpl, LOCKOUT_DURATION, MAX_FAILED_ATTEMPTS};
pub use authorization::AuthorizationServiceImpl;
pub use current_user::CurrentUserServiceImpl;
pub use password_hasher::{CURRENT_VERSION, HASH_SIZE, ITERATIONS, PasswordHasherImpl, SALT_SIZE};
pub use token_service::{MAX_TICKS, TokenServiceImpl, TokenServiceOptions, from_ticks, to_ticks};
