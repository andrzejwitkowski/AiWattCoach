use super::super::{documents::SettingsDocument, mapping::map_document_availability_to_domain};
use super::support::build_settings_document;
use crate::domain::settings::Weekday;

#[test]
fn settings_document_deserializes_missing_availability_with_full_week_default() {
    let document = serde_json::json!({
        "user_id": "user-1",
        "ai_agents": {},
        "intervals": {},
        "options": {},
        "cycling": {},
        "created_at_epoch_seconds": 1,
        "updated_at_epoch_seconds": 1
    });

    let parsed: SettingsDocument = serde_json::from_value(document).unwrap();

    assert!(!parsed.availability.configured);
    assert_eq!(parsed.availability.days.len(), 7);
    assert!(parsed.availability.days.iter().all(|day| !day.available));
    assert!(!parsed.wahoo.connected);
}

#[test]
fn map_document_to_domain_returns_repository_error_when_required_timestamps_are_missing() {
    let error = super::super::mapping::map_document_to_domain(SettingsDocument {
        created_at_epoch_seconds: None,
        created_at: None,
        updated_at_epoch_seconds: Some(1),
        updated_at: None,
        ..build_settings_document("user-1", 1)
    })
    .unwrap_err();

    assert_eq!(
        error,
        crate::domain::settings::SettingsError::Repository(
            "missing created_at timestamp".to_string()
        )
    );
}

#[test]
fn map_document_availability_to_domain_falls_back_for_legacy_empty_days() {
    let availability =
        map_document_availability_to_domain(super::super::documents::AvailabilityDocument {
            configured: false,
            days: Vec::new(),
        });

    assert!(!availability.configured);
    assert_eq!(availability.days.len(), 7);
    assert!(availability.days.iter().all(|day| !day.available));
}

#[test]
fn map_document_availability_to_domain_repairs_case_and_missing_days() {
    let availability =
        map_document_availability_to_domain(super::super::documents::AvailabilityDocument {
            configured: true,
            days: vec![
                super::super::documents::AvailabilityDayDocument {
                    weekday: " MON ".to_string(),
                    available: true,
                    max_duration_minutes: Some(60),
                },
                super::super::documents::AvailabilityDayDocument {
                    weekday: Weekday::Tue.as_str().to_string(),
                    available: false,
                    max_duration_minutes: Some(90),
                },
            ],
        });

    assert!(!availability.is_configured());
    assert_eq!(availability.days.len(), 7);
    assert_eq!(availability.days[0].weekday, Weekday::Mon);
    assert_eq!(availability.days[0].max_duration_minutes, Some(60));
    assert_eq!(availability.days[1].weekday, Weekday::Tue);
    assert_eq!(availability.days[1].max_duration_minutes, None);
    assert!(availability.days[2..].iter().all(|day| !day.available));
}

#[test]
fn map_document_availability_to_domain_sanitizes_invalid_duration_without_resetting_week() {
    let availability =
        map_document_availability_to_domain(super::super::documents::AvailabilityDocument {
            configured: true,
            days: vec![
                super::super::documents::AvailabilityDayDocument {
                    weekday: Weekday::Mon.as_str().to_string(),
                    available: true,
                    max_duration_minutes: Some(45),
                },
                super::super::documents::AvailabilityDayDocument {
                    weekday: Weekday::Tue.as_str().to_string(),
                    available: false,
                    max_duration_minutes: None,
                },
                super::super::documents::AvailabilityDayDocument {
                    weekday: Weekday::Wed.as_str().to_string(),
                    available: true,
                    max_duration_minutes: Some(90),
                },
                super::super::documents::AvailabilityDayDocument {
                    weekday: Weekday::Thu.as_str().to_string(),
                    available: false,
                    max_duration_minutes: None,
                },
                super::super::documents::AvailabilityDayDocument {
                    weekday: Weekday::Fri.as_str().to_string(),
                    available: false,
                    max_duration_minutes: None,
                },
                super::super::documents::AvailabilityDayDocument {
                    weekday: Weekday::Sat.as_str().to_string(),
                    available: false,
                    max_duration_minutes: None,
                },
                super::super::documents::AvailabilityDayDocument {
                    weekday: Weekday::Sun.as_str().to_string(),
                    available: false,
                    max_duration_minutes: None,
                },
            ],
        });

    assert!(availability.is_configured());
    assert_eq!(availability.days[0].weekday, Weekday::Mon);
    assert!(!availability.days[0].available);
    assert_eq!(availability.days[0].max_duration_minutes, None);
    assert_eq!(availability.days[2].weekday, Weekday::Wed);
    assert!(availability.days[2].available);
    assert_eq!(availability.days[2].max_duration_minutes, Some(90));
}

#[test]
fn map_document_availability_to_domain_keeps_partial_legacy_week_unconfigured() {
    let availability =
        map_document_availability_to_domain(super::super::documents::AvailabilityDocument {
            configured: true,
            days: vec![
                super::super::documents::AvailabilityDayDocument {
                    weekday: Weekday::Mon.as_str().to_string(),
                    available: true,
                    max_duration_minutes: Some(60),
                },
                super::super::documents::AvailabilityDayDocument {
                    weekday: Weekday::Tue.as_str().to_string(),
                    available: false,
                    max_duration_minutes: None,
                },
            ],
        });

    assert!(!availability.configured);
    assert!(!availability.is_configured());
    assert!(availability.days[0].available);
    assert_eq!(availability.days[0].max_duration_minutes, Some(60));
}

#[test]
fn map_document_availability_to_domain_treats_duplicate_weekdays_as_unconfigured() {
    let availability =
        map_document_availability_to_domain(super::super::documents::AvailabilityDocument {
            configured: true,
            days: vec![
                super::super::documents::AvailabilityDayDocument {
                    weekday: Weekday::Mon.as_str().to_string(),
                    available: true,
                    max_duration_minutes: Some(60),
                },
                super::super::documents::AvailabilityDayDocument {
                    weekday: Weekday::Mon.as_str().to_string(),
                    available: false,
                    max_duration_minutes: None,
                },
                super::super::documents::AvailabilityDayDocument {
                    weekday: Weekday::Tue.as_str().to_string(),
                    available: false,
                    max_duration_minutes: None,
                },
                super::super::documents::AvailabilityDayDocument {
                    weekday: Weekday::Wed.as_str().to_string(),
                    available: false,
                    max_duration_minutes: None,
                },
                super::super::documents::AvailabilityDayDocument {
                    weekday: Weekday::Thu.as_str().to_string(),
                    available: false,
                    max_duration_minutes: None,
                },
                super::super::documents::AvailabilityDayDocument {
                    weekday: Weekday::Fri.as_str().to_string(),
                    available: false,
                    max_duration_minutes: None,
                },
                super::super::documents::AvailabilityDayDocument {
                    weekday: Weekday::Sat.as_str().to_string(),
                    available: false,
                    max_duration_minutes: None,
                },
                super::super::documents::AvailabilityDayDocument {
                    weekday: Weekday::Sun.as_str().to_string(),
                    available: false,
                    max_duration_minutes: None,
                },
            ],
        });

    assert!(!availability.configured);
    assert!(!availability.is_configured());
}
