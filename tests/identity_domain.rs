use aiwattcoach::domain::identity::{
    assign_roles, authorize_admin_access, is_valid_email, normalize_email, AppUser, AuthSession,
    GoogleIdentity, IdentityError, LoginState, Role,
};

#[test]
fn normalize_email_trims_and_lowercases_values() {
    assert_eq!(
        normalize_email("  Athlete@Example.COM "),
        "athlete@example.com"
    );
}

#[test]
fn is_valid_email_accepts_simple_address() {
    assert!(is_valid_email("athlete@example.com"));
}

#[test]
fn is_valid_email_rejects_invalid_shapes() {
    assert!(!is_valid_email("not-an-email"));
    assert!(!is_valid_email("@example.com"));
    assert!(!is_valid_email("athlete@@example.com"));
    assert!(!is_valid_email("athlete@example"));
    assert!(!is_valid_email("athlete@.example.com"));
    assert!(!is_valid_email("athlete@example."));
    assert!(!is_valid_email("athlete@exa..mple.com"));
    assert!(!is_valid_email("athle\u{0001}te@example.com"));
}

#[test]
fn assign_roles_always_includes_user() {
    let roles = assign_roles("athlete@example.com", &[]);

    assert_eq!(roles, vec![Role::User]);
}

#[test]
fn assign_roles_adds_admin_when_email_is_in_admin_list() {
    let roles = assign_roles("admin@example.com", &["admin@example.com".to_string()]);

    assert_eq!(roles, vec![Role::User, Role::Admin]);
}

#[test]
fn google_identity_requires_verified_email() {
    let error = GoogleIdentity::new(
        "google-subject-1",
        "athlete@example.com",
        false,
        Some("Athlete".to_string()),
        None,
    )
    .unwrap_err();

    assert_eq!(error, IdentityError::EmailNotVerified);
}

#[test]
fn app_user_is_admin_only_when_role_is_present() {
    let user = AppUser::new(
        "user-1".to_string(),
        "google-subject-1".to_string(),
        "athlete@example.com".to_string(),
        vec![Role::User, Role::Admin],
        Some("Athlete".to_string()),
        None,
        true,
    );

    assert!(user.is_admin());
}

#[test]
fn authorize_admin_access_rejects_non_admins() {
    let user = AppUser::new(
        "user-1".to_string(),
        "google-subject-1".to_string(),
        "athlete@example.com".to_string(),
        vec![Role::User],
        Some("Athlete".to_string()),
        None,
        true,
    );

    let error = authorize_admin_access(&user).unwrap_err();

    assert_eq!(error, IdentityError::Forbidden);
}

#[test]
fn auth_session_is_expired_at_exact_expiry_boundary() {
    let session = AuthSession::new("session-1".to_string(), "user-1".to_string(), 100, 10);

    assert!(session.is_expired(100));
}

#[test]
fn login_state_is_expired_at_exact_expiry_boundary() {
    let login_state = LoginState::new("state-1".to_string(), Some("/app".to_string()), 100, 10);

    assert!(login_state.is_expired(100));
}
