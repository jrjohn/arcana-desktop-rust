//! C# `CurrentUserServiceTests` (26).

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use arcana_core::security::{AuthenticatedUser, CurrentUserChanged, CurrentUserService};
use arcana_infra::security::CurrentUserServiceImpl;

fn user(id: i32, username: &str) -> AuthenticatedUser {
    AuthenticatedUser {
        id,
        username: username.into(),
        display_name: "Test User".into(),
        email: Some("test@test.com".into()),
        roles: vec!["Admin".into(), "User".into()],
        permissions: BTreeSet::from(["orders.view".to_owned(), "orders.create".to_owned()]),
    }
}

fn default_user() -> AuthenticatedUser {
    user(1, "testuser")
}

fn signed_in() -> CurrentUserServiceImpl {
    let s = CurrentUserServiceImpl::new();
    s.set_current_user(Some(default_user()));
    s
}

fn capture(s: &CurrentUserServiceImpl) -> Arc<Mutex<Option<CurrentUserChanged>>> {
    let slot = Arc::new(Mutex::new(None));
    let sink = slot.clone();
    s.on_current_user_changed(Box::new(move |e| *sink.lock().unwrap() = Some(e.clone())));
    slot
}

// IsAuthenticated_WhenNoUserSet_ShouldReturnFalse
#[test]
fn is_authenticated_when_no_user_set_should_return_false() {
    assert!(!CurrentUserServiceImpl::new().is_authenticated());
}

// CurrentUser_WhenNoUserSet_ShouldReturnNull
#[test]
fn current_user_when_no_user_set_should_return_null() {
    assert_eq!(CurrentUserServiceImpl::new().current_user(), None);
}

// UserId_WhenNoUserSet_ShouldReturnNull
#[test]
fn user_id_when_no_user_set_should_return_null() {
    assert_eq!(CurrentUserServiceImpl::new().user_id(), None);
}

// Username_WhenNoUserSet_ShouldReturnNull
#[test]
fn username_when_no_user_set_should_return_null() {
    assert_eq!(CurrentUserServiceImpl::new().username(), None);
}

// SetCurrentUser_ValidUser_ShouldSetUser
#[test]
fn set_current_user_valid_user_should_set_user() {
    let s = signed_in();
    assert!(s.is_authenticated());
    assert_eq!(s.current_user(), Some(default_user()));
    assert_eq!(s.user_id(), Some(1));
    assert_eq!(s.username().as_deref(), Some("testuser"));
    assert!(format!("{s:?}").contains("testuser"));
}

// SetCurrentUser_RaisesCurrentUserChangedEvent
#[test]
fn set_current_user_raises_current_user_changed_event() {
    let s = CurrentUserServiceImpl::new();
    let seen = capture(&s);
    s.set_current_user(Some(default_user()));
    let e = seen.lock().unwrap().clone().unwrap();
    assert_eq!(e.new_user, Some(default_user()));
    assert_eq!(e.previous_user, None);
}

// SetCurrentUser_Replaces_ExistingUser_AndRaisesEvent
#[test]
fn set_current_user_replaces_existing_user_and_raises_event() {
    let s = CurrentUserServiceImpl::new();
    s.set_current_user(Some(user(1, "user1")));
    let seen = capture(&s);
    s.set_current_user(Some(user(2, "user2")));
    let e = seen.lock().unwrap().clone().unwrap();
    assert_eq!(e.previous_user, Some(user(1, "user1")));
    assert_eq!(e.new_user, Some(user(2, "user2")));
    assert_eq!(s.current_user(), Some(user(2, "user2")));
}

// ClearCurrentUser_ShouldRemoveUser
#[test]
fn clear_current_user_should_remove_user() {
    let s = signed_in();
    s.clear_current_user();
    assert!(!s.is_authenticated());
    assert_eq!(s.current_user(), None);
}

// ClearCurrentUser_RaisesEvent_WithNullNewUser
#[test]
fn clear_current_user_raises_event_with_null_new_user() {
    let s = signed_in();
    let seen = capture(&s);
    s.clear_current_user();
    assert_eq!(seen.lock().unwrap().clone().unwrap().new_user, None);
}

// HasPermission_WhenUserHasPermission_ShouldReturnTrue
#[test]
fn has_permission_when_user_has_permission_should_return_true() {
    assert!(signed_in().has_permission("orders.view"));
}

// HasPermission_WhenUserDoesNotHavePermission_ShouldReturnFalse
#[test]
fn has_permission_when_user_does_not_have_permission_should_return_false() {
    assert!(!signed_in().has_permission("admin.delete"));
}

// HasPermission_WhenNoUser_ShouldReturnFalse
#[test]
fn has_permission_when_no_user_should_return_false() {
    assert!(!CurrentUserServiceImpl::new().has_permission("orders.view"));
}

// HasAnyPermission_WhenUserHasAtLeastOne_ShouldReturnTrue
#[test]
fn has_any_permission_when_user_has_at_least_one_should_return_true() {
    assert!(signed_in().has_any_permission(&["orders.view", "admin.delete"]));
}

// HasAnyPermission_WhenUserHasNone_ShouldReturnFalse
#[test]
fn has_any_permission_when_user_has_none_should_return_false() {
    assert!(!signed_in().has_any_permission(&["admin.delete", "admin.create"]));
}

// HasAnyPermission_WhenNoUser_ShouldReturnFalse
#[test]
fn has_any_permission_when_no_user_should_return_false() {
    assert!(!CurrentUserServiceImpl::new().has_any_permission(&["orders.view"]));
}

// HasAllPermissions_WhenUserHasAll_ShouldReturnTrue
#[test]
fn has_all_permissions_when_user_has_all_should_return_true() {
    assert!(signed_in().has_all_permissions(&["orders.view", "orders.create"]));
}

// HasAllPermissions_WhenUserMissingOne_ShouldReturnFalse
#[test]
fn has_all_permissions_when_user_missing_one_should_return_false() {
    assert!(!signed_in().has_all_permissions(&["orders.view", "admin.delete"]));
}

// HasAllPermissions_WhenNoUser_ShouldReturnFalse
#[test]
fn has_all_permissions_when_no_user_should_return_false() {
    assert!(!CurrentUserServiceImpl::new().has_all_permissions(&["orders.view"]));
}

// IsInRole_WhenUserHasRole_ShouldReturnTrue
#[test]
fn is_in_role_when_user_has_role_should_return_true() {
    assert!(signed_in().is_in_role("Admin"));
}

// IsInRole_CaseInsensitive_ShouldReturnTrue
#[test]
fn is_in_role_case_insensitive_should_return_true() {
    assert!(signed_in().is_in_role("admin"));
}

// IsInRole_WhenUserDoesNotHaveRole_ShouldReturnFalse
#[test]
fn is_in_role_when_user_does_not_have_role_should_return_false() {
    assert!(!signed_in().is_in_role("SuperAdmin"));
}

// IsInRole_WhenNoUser_ShouldReturnFalse
#[test]
fn is_in_role_when_no_user_should_return_false() {
    assert!(!CurrentUserServiceImpl::new().is_in_role("Admin"));
}

// IsInAnyRole_WhenUserHasAtLeastOne_ShouldReturnTrue
#[test]
fn is_in_any_role_when_user_has_at_least_one_should_return_true() {
    assert!(signed_in().is_in_any_role(&["Admin", "SuperAdmin"]));
}

// IsInAnyRole_WhenUserHasNone_ShouldReturnFalse
#[test]
fn is_in_any_role_when_user_has_none_should_return_false() {
    assert!(!signed_in().is_in_any_role(&["SuperAdmin", "Guest"]));
}

// IsInAnyRole_WhenNoUser_ShouldReturnFalse
#[test]
fn is_in_any_role_when_no_user_should_return_false() {
    assert!(!CurrentUserServiceImpl::new().is_in_any_role(&["Admin"]));
}

// ThreadSafety_SetCurrentUser_ShouldNotThrow
#[test]
fn thread_safety_set_current_user_should_not_throw() {
    let s = Arc::new(CurrentUserServiceImpl::new());
    let handles: Vec<_> = (0..20)
        .map(|i| {
            let s = s.clone();
            std::thread::spawn(move || {
                s.set_current_user(Some(user(i, &format!("user{i}"))));
                let _ = s.current_user();
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    assert!(s.is_authenticated());
}
