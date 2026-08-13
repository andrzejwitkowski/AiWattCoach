use super::super::service::validate_request;
use crate::domain::races::RaceError;

#[test]
fn validate_request_rejects_distance_above_upper_bound() {
    let err = validate_request("2026-09-12", "Race", 10_000_001).unwrap_err();
    assert!(matches!(err, RaceError::Validation(_)));
}

#[test]
fn validate_request_rejects_invalid_date_format() {
    for bad_date in [
        "2026-9-1",
        "26-09-12",
        "2026/09/12",
        "not-a-date",
        "2026-13-01",
        "2026-02-30",
    ] {
        let err = validate_request(bad_date, "Race", 100_000).unwrap_err();
        assert!(
            matches!(err, RaceError::Validation(_)),
            "expected Validation error for date {bad_date:?}, got {err:?}"
        );
    }
}

#[test]
fn validate_request_accepts_valid_date() {
    validate_request("2026-09-12", "Race", 100_000).expect("should accept valid date");
}
