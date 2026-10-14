//! Identity entities (C# `Arcana.Domain.Entities.Identity`).

use arcana_core::{Timestamp, int_enum, utc_now};

use super::base::{BaseEntity, impl_base_entity};

/// C# `User`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub base: BaseEntity,
    pub username: String,
    pub email: Option<String>,
    pub display_name: String,
    pub password_hash: String,
    pub password_salt: Option<String>,
    pub is_active: bool,
    pub is_locked: bool,
    pub locked_until: Option<Timestamp>,
    pub failed_login_attempts: i32,
    pub last_login_at: Option<Timestamp>,
    pub password_changed_at: Option<Timestamp>,
    pub must_change_password: bool,
    pub refresh_token: Option<String>,
    pub refresh_token_expires_at: Option<Timestamp>,
    pub user_roles: Vec<UserRole>,
    pub user_permissions: Vec<UserPermission>,
}

impl Default for User {
    fn default() -> Self {
        Self {
            base: BaseEntity::default(),
            username: String::new(),
            email: None,
            display_name: String::new(),
            password_hash: String::new(),
            password_salt: None,
            is_active: true,
            is_locked: false,
            locked_until: None,
            failed_login_attempts: 0,
            last_login_at: None,
            password_changed_at: None,
            must_change_password: false,
            refresh_token: None,
            refresh_token_expires_at: None,
            user_roles: Vec::new(),
            user_permissions: Vec::new(),
        }
    }
}

/// C# `Role`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Role {
    pub base: BaseEntity,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub is_system: bool,
    pub priority: i32,
    pub user_roles: Vec<UserRole>,
    pub role_permissions: Vec<RolePermission>,
}

/// Built-in role names (C# `SystemRoles`).
pub mod system_roles {
    pub const ADMINISTRATOR: &str = "Administrator";
    pub const MANAGER: &str = "Manager";
    pub const USER: &str = "User";
    pub const GUEST: &str = "Guest";
    pub const ALL: [&str; 4] = [ADMINISTRATOR, MANAGER, USER, GUEST];
}

/// C# `UserRole` (user ↔ role link).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserRole {
    pub base: BaseEntity,
    pub user_id: i32,
    pub role_id: i32,
    pub assigned_at: Timestamp,
    pub assigned_by: Option<String>,
    pub expires_at: Option<Timestamp>,
    pub user: Option<Box<User>>,
    pub role: Option<Box<Role>>,
}

impl Default for UserRole {
    fn default() -> Self {
        Self {
            base: BaseEntity::default(),
            user_id: 0,
            role_id: 0,
            assigned_at: utc_now(),
            assigned_by: None,
            expires_at: None,
            user: None,
            role: None,
        }
    }
}

/// C# `AppPermission`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppPermission {
    pub base: BaseEntity,
    pub code: String,
    pub display_name: String,
    pub description: Option<String>,
    pub category: String,
    pub is_system: bool,
    pub role_permissions: Vec<RolePermission>,
    pub user_permissions: Vec<UserPermission>,
}

/// Built-in permission codes (C# `SystemPermissions`).
pub mod system_permissions {
    pub const ORDERS_VIEW: &str = "orders.view";
    pub const ORDERS_CREATE: &str = "orders.create";
    pub const ORDERS_EDIT: &str = "orders.edit";
    pub const ORDERS_DELETE: &str = "orders.delete";
    pub const ORDERS_APPROVE: &str = "orders.approve";
    pub const ORDERS_CANCEL: &str = "orders.cancel";
    pub const CUSTOMERS_VIEW: &str = "customers.view";
    pub const CUSTOMERS_CREATE: &str = "customers.create";
    pub const CUSTOMERS_EDIT: &str = "customers.edit";
    pub const CUSTOMERS_DELETE: &str = "customers.delete";
    pub const PRODUCTS_VIEW: &str = "products.view";
    pub const PRODUCTS_CREATE: &str = "products.create";
    pub const PRODUCTS_EDIT: &str = "products.edit";
    pub const PRODUCTS_DELETE: &str = "products.delete";
    pub const PRODUCTS_MANAGE_STOCK: &str = "products.manage_stock";
    pub const USERS_VIEW: &str = "users.view";
    pub const USERS_CREATE: &str = "users.create";
    pub const USERS_EDIT: &str = "users.edit";
    pub const USERS_DELETE: &str = "users.delete";
    pub const USERS_MANAGE_ROLES: &str = "users.manage_roles";
    pub const ROLES_VIEW: &str = "roles.view";
    pub const ROLES_CREATE: &str = "roles.create";
    pub const ROLES_EDIT: &str = "roles.edit";
    pub const ROLES_DELETE: &str = "roles.delete";
    pub const PLUGINS_VIEW: &str = "plugins.view";
    pub const PLUGINS_INSTALL: &str = "plugins.install";
    pub const PLUGINS_UNINSTALL: &str = "plugins.uninstall";
    pub const PLUGINS_MANAGE: &str = "plugins.manage";
    pub const SETTINGS_VIEW: &str = "settings.view";
    pub const SETTINGS_EDIT: &str = "settings.edit";
    pub const SYSTEM_BACKUP: &str = "system.backup";
    pub const SYSTEM_RESTORE: &str = "system.restore";
    pub const AUDIT_LOGS_VIEW: &str = "audit.view";

    /// Every code above, in declaration order (used for seeding in milestone 3).
    pub const ALL: [&str; 33] = [
        ORDERS_VIEW,
        ORDERS_CREATE,
        ORDERS_EDIT,
        ORDERS_DELETE,
        ORDERS_APPROVE,
        ORDERS_CANCEL,
        CUSTOMERS_VIEW,
        CUSTOMERS_CREATE,
        CUSTOMERS_EDIT,
        CUSTOMERS_DELETE,
        PRODUCTS_VIEW,
        PRODUCTS_CREATE,
        PRODUCTS_EDIT,
        PRODUCTS_DELETE,
        PRODUCTS_MANAGE_STOCK,
        USERS_VIEW,
        USERS_CREATE,
        USERS_EDIT,
        USERS_DELETE,
        USERS_MANAGE_ROLES,
        ROLES_VIEW,
        ROLES_CREATE,
        ROLES_EDIT,
        ROLES_DELETE,
        PLUGINS_VIEW,
        PLUGINS_INSTALL,
        PLUGINS_UNINSTALL,
        PLUGINS_MANAGE,
        SETTINGS_VIEW,
        SETTINGS_EDIT,
        SYSTEM_BACKUP,
        SYSTEM_RESTORE,
        AUDIT_LOGS_VIEW,
    ];
}

/// C# `RolePermission` (role ↔ permission link).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolePermission {
    pub base: BaseEntity,
    pub role_id: i32,
    pub permission_id: i32,
    pub granted_at: Timestamp,
    pub granted_by: Option<String>,
    pub role: Option<Box<Role>>,
    pub permission: Option<Box<AppPermission>>,
}

impl Default for RolePermission {
    fn default() -> Self {
        Self {
            base: BaseEntity::default(),
            role_id: 0,
            permission_id: 0,
            granted_at: utc_now(),
            granted_by: None,
            role: None,
            permission: None,
        }
    }
}

/// C# `UserPermission`: a direct grant (or explicit deny when `is_granted == false`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserPermission {
    pub base: BaseEntity,
    pub user_id: i32,
    pub permission_id: i32,
    pub is_granted: bool,
    pub assigned_at: Timestamp,
    pub assigned_by: Option<String>,
    pub expires_at: Option<Timestamp>,
    pub user: Option<Box<User>>,
    pub permission: Option<Box<AppPermission>>,
}

impl Default for UserPermission {
    fn default() -> Self {
        Self {
            base: BaseEntity::default(),
            user_id: 0,
            permission_id: 0,
            is_granted: true,
            assigned_at: utc_now(),
            assigned_by: None,
            expires_at: None,
            user: None,
            permission: None,
        }
    }
}

int_enum! {
    /// C# `AuditEventType` (implicit values 0..=29).
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
    pub enum AuditEventType {
        #[default]
        LoginSuccess = 0,
        LoginFailed = 1,
        Logout = 2,
        PasswordChanged = 3,
        PasswordResetRequested = 4,
        PasswordResetCompleted = 5,
        AccountLocked = 6,
        AccountUnlocked = 7,
        PermissionGranted = 8,
        PermissionRevoked = 9,
        RoleAssigned = 10,
        RoleRevoked = 11,
        AccessDenied = 12,
        UserCreated = 13,
        UserUpdated = 14,
        UserDeleted = 15,
        UserActivated = 16,
        UserDeactivated = 17,
        RoleCreated = 18,
        RoleUpdated = 19,
        RoleDeleted = 20,
        SessionCreated = 21,
        SessionExpired = 22,
        SessionTerminated = 23,
        TokenRefreshed = 24,
        SystemStartup = 25,
        SystemShutdown = 26,
        ConfigurationChanged = 27,
        PluginInstalled = 28,
        PluginUninstalled = 29,
    }
}

/// C# `AuditLog`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditLog {
    pub base: BaseEntity,
    pub event_type: AuditEventType,
    pub user_id: Option<i32>,
    pub username: Option<String>,
    pub client_info: Option<String>,
    pub resource: Option<String>,
    pub action: String,
    pub details: Option<String>,
    pub is_success: bool,
    pub error_message: Option<String>,
    pub timestamp: Timestamp,
}

impl Default for AuditLog {
    fn default() -> Self {
        Self {
            base: BaseEntity::default(),
            event_type: AuditEventType::default(),
            user_id: None,
            username: None,
            client_info: None,
            resource: None,
            action: String::new(),
            details: None,
            is_success: false,
            error_message: None,
            timestamp: utc_now(),
        }
    }
}

impl_base_entity!(
    User,
    Role,
    UserRole,
    AppPermission,
    RolePermission,
    UserPermission,
    AuditLog
);
