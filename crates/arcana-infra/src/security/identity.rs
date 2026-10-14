//! Loading a user's role/permission graph (the EF `Include(...).ThenInclude(...)` chains in
//! C# `AuthServiceImpl` / `AuthorizationServiceImpl`).

use std::collections::BTreeSet;

use arcana_core::security::AuthenticatedUser;
use arcana_core::{AppResult, Repository, Timestamp, utc_now};
use arcana_data::{Db, SqliteRepository};
use arcana_domain::{AppPermission, Role, RolePermission, User, UserPermission, UserRole};

/// Case-insensitive role membership (C# `StringComparer.OrdinalIgnoreCase`).
pub(crate) fn role_matches(roles: &[String], role: &str) -> bool {
    let wanted = role.to_uppercase();
    roles.iter().any(|r| r.to_uppercase() == wanted)
}

fn active(expires_at: Option<Timestamp>, now: Timestamp) -> bool {
    expires_at.is_none_or(|e| e > now)
}

/// One role assignment with the role's permission codes.
pub(crate) struct RoleGrant {
    pub link: UserRole,
    pub role_name: String,
    pub permission_codes: Vec<String>,
}

/// A user with everything needed to compute roles and permissions. Soft-deleted rows are
/// already excluded by the repositories' query filter (C# EF global filter).
pub(crate) struct UserGraph {
    pub user: User,
    pub roles: Vec<RoleGrant>,
    pub direct: Vec<(UserPermission, String)>,
}

/// Read access to the identity tables.
#[derive(Clone)]
pub(crate) struct IdentityStore {
    pub users: SqliteRepository<User>,
    user_roles: SqliteRepository<UserRole>,
    roles: SqliteRepository<Role>,
    role_permissions: SqliteRepository<RolePermission>,
    user_permissions: SqliteRepository<UserPermission>,
    permissions: SqliteRepository<AppPermission>,
}

impl IdentityStore {
    pub fn new(db: &Db) -> Self {
        Self {
            users: SqliteRepository::new(db.clone()),
            user_roles: SqliteRepository::new(db.clone()),
            roles: SqliteRepository::new(db.clone()),
            role_permissions: SqliteRepository::new(db.clone()),
            user_permissions: SqliteRepository::new(db.clone()),
            permissions: SqliteRepository::new(db.clone()),
        }
    }

    async fn permission_code(&self, id: i32) -> AppResult<Option<String>> {
        Ok(self.permissions.get_by_id(id).await?.map(|p| p.code))
    }

    /// Role names of the user's unexpired assignments (C# `GetUserRolesAsync`).
    pub async fn role_names(&self, user_id: i32) -> AppResult<Vec<String>> {
        let now = utc_now();
        let links = self
            .user_roles
            .find(&|ur: &UserRole| ur.user_id == user_id && active(ur.expires_at, now))
            .await?;
        let mut names = Vec::new();
        for link in links {
            if let Some(role) = self.roles.get_by_id(link.role_id).await? {
                names.push(role.name);
            }
        }
        Ok(names)
    }

    /// Load the graph for an already-fetched user.
    pub async fn graph(&self, user: User) -> AppResult<UserGraph> {
        let uid = user.base.id;
        let mut roles = Vec::new();
        for link in self
            .user_roles
            .find(&|ur: &UserRole| ur.user_id == uid)
            .await?
        {
            let Some(role) = self.roles.get_by_id(link.role_id).await? else {
                continue;
            };
            let rid = role.base.id;
            let mut permission_codes = Vec::new();
            for rp in self
                .role_permissions
                .find(&|rp: &RolePermission| rp.role_id == rid)
                .await?
            {
                if let Some(code) = self.permission_code(rp.permission_id).await? {
                    permission_codes.push(code);
                }
            }
            roles.push(RoleGrant {
                link,
                role_name: role.name,
                permission_codes,
            });
        }
        let mut direct = Vec::new();
        for up in self
            .user_permissions
            .find(&|up: &UserPermission| up.user_id == uid)
            .await?
        {
            if let Some(code) = self.permission_code(up.permission_id).await? {
                direct.push((up, code));
            }
        }
        Ok(UserGraph {
            user,
            roles,
            direct,
        })
    }
}

impl UserGraph {
    /// Effective permissions: role grants, then direct grants added / denies removed.
    ///
    /// `skip_expired_roles`: C# `AuthorizationServiceImpl` ignores expired role assignments
    /// here, while `AuthServiceImpl.BuildAuthenticatedUser` does not. Both are kept as-is.
    pub fn permissions(&self, skip_expired_roles: bool) -> BTreeSet<String> {
        let now = utc_now();
        let mut set = BTreeSet::new();
        for g in &self.roles {
            if !skip_expired_roles || active(g.link.expires_at, now) {
                set.extend(g.permission_codes.iter().cloned());
            }
        }
        for (up, code) in &self.direct {
            if active(up.expires_at, now) {
                if up.is_granted {
                    set.insert(code.clone());
                } else {
                    set.remove(code);
                }
            }
        }
        set
    }

    /// C# `BuildAuthenticatedUser`.
    pub fn authenticated_user(&self) -> AuthenticatedUser {
        let now = utc_now();
        AuthenticatedUser {
            id: self.user.base.id,
            username: self.user.username.clone(),
            display_name: self.user.display_name.clone(),
            email: self.user.email.clone(),
            roles: self
                .roles
                .iter()
                .filter(|g| active(g.link.expires_at, now))
                .map(|g| g.role_name.clone())
                .collect(),
            permissions: self.permissions(false),
        }
    }
}
