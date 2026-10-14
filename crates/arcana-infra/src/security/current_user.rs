//! The signed-in user (C# `CurrentUserServiceImpl`).

use std::sync::{Mutex, MutexGuard, PoisonError, RwLock};

use arcana_core::security::{
    AuthenticatedUser, CurrentUserChanged, CurrentUserHandler, CurrentUserService,
};

/// C# `CurrentUserServiceImpl`. Thread-safe; handlers run after the lock is released, as in
/// C#.
#[derive(Default)]
pub struct CurrentUserServiceImpl {
    user: RwLock<Option<AuthenticatedUser>>,
    handlers: Mutex<Vec<CurrentUserHandler>>,
}

impl std::fmt::Debug for CurrentUserServiceImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CurrentUserServiceImpl")
            .field("user", &self.current_user())
            .finish_non_exhaustive()
    }
}

impl CurrentUserServiceImpl {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn handlers(&self) -> MutexGuard<'_, Vec<CurrentUserHandler>> {
        self.handlers.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn with_user<R>(&self, f: impl FnOnce(&AuthenticatedUser) -> R) -> Option<R> {
        self.user
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .map(f)
    }
}

fn has_role(user: &AuthenticatedUser, role: &str) -> bool {
    crate::security::identity::role_matches(&user.roles, role)
}

impl CurrentUserService for CurrentUserServiceImpl {
    fn is_authenticated(&self) -> bool {
        self.with_user(|_| ()).is_some()
    }

    fn current_user(&self) -> Option<AuthenticatedUser> {
        self.with_user(Clone::clone)
    }

    fn user_id(&self) -> Option<i32> {
        self.with_user(|u| u.id)
    }

    fn username(&self) -> Option<String> {
        self.with_user(|u| u.username.clone())
    }

    fn set_current_user(&self, user: Option<AuthenticatedUser>) {
        let previous_user = std::mem::replace(
            &mut *self.user.write().unwrap_or_else(PoisonError::into_inner),
            user.clone(),
        );
        let e = CurrentUserChanged {
            previous_user,
            new_user: user,
        };
        for h in self.handlers().iter() {
            h(&e);
        }
    }

    fn clear_current_user(&self) {
        self.set_current_user(None);
    }

    fn has_permission(&self, permission: &str) -> bool {
        self.with_user(|u| u.permissions.contains(permission))
            .unwrap_or(false)
    }

    fn has_any_permission(&self, permissions: &[&str]) -> bool {
        self.with_user(|u| permissions.iter().any(|p| u.permissions.contains(*p)))
            .unwrap_or(false)
    }

    fn has_all_permissions(&self, permissions: &[&str]) -> bool {
        self.with_user(|u| permissions.iter().all(|p| u.permissions.contains(*p)))
            .unwrap_or(false)
    }

    fn is_in_role(&self, role: &str) -> bool {
        self.with_user(|u| has_role(u, role)).unwrap_or(false)
    }

    fn is_in_any_role(&self, roles: &[&str]) -> bool {
        self.with_user(|u| roles.iter().any(|r| has_role(u, r)))
            .unwrap_or(false)
    }

    fn on_current_user_changed(&self, handler: CurrentUserHandler) {
        self.handlers().push(handler);
    }
}
