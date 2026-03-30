use aiwattcoach::domain::identity::{validate_session_ttl_against_current_time, IdentityError};

#[test]
fn validate_session_ttl_against_current_time_rejects_bson_overflowing_ttl() {
    let error = validate_session_ttl_against_current_time(i64::MAX / 1000, 1).unwrap_err();

    assert!(
        matches!(error, IdentityError::External(message) if message.contains("SESSION_TTL_HOURS"))
    );
}
