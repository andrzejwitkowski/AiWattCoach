use crate::domain::settings::{AvailabilityDay, AvailabilitySettings, SettingsError, Weekday};

use crate::domain::llm::LlmProvider;

pub const ALLOWED_AVAILABILITY_MINUTES: [u16; 10] = [30, 60, 90, 120, 150, 180, 210, 240, 270, 300];

pub fn is_allowed_availability_duration(minutes: u16) -> bool {
    ALLOWED_AVAILABILITY_MINUTES.contains(&minutes)
}

pub fn validate_cycling_age(age: Option<u32>) -> Result<Option<u32>, SettingsError> {
    match age {
        Some(v) if v == 0 || v > 120 => Err(SettingsError::Validation(
            "age must be between 1 and 120".to_string(),
        )),
        _ => Ok(age),
    }
}

pub fn validate_cycling_height(height_cm: Option<u32>) -> Result<Option<u32>, SettingsError> {
    match height_cm {
        Some(v) if v == 0 || v > 300 => Err(SettingsError::Validation(
            "heightCm must be between 1 and 300".to_string(),
        )),
        _ => Ok(height_cm),
    }
}

pub fn validate_cycling_weight(weight_kg: Option<f64>) -> Result<Option<f64>, SettingsError> {
    match weight_kg {
        Some(v) if v <= 0.0 || v > 500.0 => Err(SettingsError::Validation(
            "weightKg must be between 0 and 500".to_string(),
        )),
        _ => Ok(weight_kg),
    }
}

pub fn validate_cycling_ftp(ftp_watts: Option<u32>) -> Result<Option<u32>, SettingsError> {
    match ftp_watts {
        Some(v) if v == 0 || v > 2500 => Err(SettingsError::Validation(
            "ftpWatts must be between 1 and 2500".to_string(),
        )),
        _ => Ok(ftp_watts),
    }
}

pub fn validate_cycling_hr(hr_max_bpm: Option<u32>) -> Result<Option<u32>, SettingsError> {
    match hr_max_bpm {
        Some(v) if v == 0 || v > 300 => Err(SettingsError::Validation(
            "hrMaxBpm must be between 1 and 300".to_string(),
        )),
        _ => Ok(hr_max_bpm),
    }
}

pub fn validate_cycling_vo2(vo2_max: Option<f64>) -> Result<Option<f64>, SettingsError> {
    match vo2_max {
        Some(v) if v <= 0.0 || v > 100.0 => Err(SettingsError::Validation(
            "vo2Max must be between 0 and 100".to_string(),
        )),
        _ => Ok(vo2_max),
    }
}

pub fn validate_optional_profile_text(
    field_name: &str,
    value: Option<String>,
    max_chars: usize,
) -> Result<Option<String>, SettingsError> {
    match value {
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }

            if trimmed.chars().count() > max_chars {
                return Err(SettingsError::Validation(format!(
                    "{field_name} must be {max_chars} characters or fewer"
                )));
            }

            Ok(Some(trimmed.to_string()))
        }
        None => Ok(None),
    }
}

pub fn validate_ai_provider(
    provider: Option<LlmProvider>,
) -> Result<Option<LlmProvider>, SettingsError> {
    Ok(provider)
}

pub fn validate_ai_model(model: Option<String>) -> Result<Option<String>, SettingsError> {
    match model {
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(SettingsError::Validation(
                    "selectedModel must not be empty".to_string(),
                ));
            }

            if trimmed.len() > 200 {
                return Err(SettingsError::Validation(
                    "selectedModel must be 200 characters or fewer".to_string(),
                ));
            }

            Ok(Some(trimmed.to_string()))
        }
        None => Ok(None),
    }
}

pub fn validate_availability(
    availability: AvailabilitySettings,
) -> Result<AvailabilitySettings, SettingsError> {
    if availability.days.len() != 7 {
        return Err(SettingsError::Validation(
            "availability must contain exactly 7 days".to_string(),
        ));
    }

    let mut seen = std::collections::BTreeSet::new();
    for day in &availability.days {
        validate_availability_day(day)?;
        if !seen.insert(day.weekday) {
            return Err(SettingsError::Validation(format!(
                "availability contains duplicate weekday '{}'",
                day.weekday
            )));
        }
    }

    let actual = seen.into_iter().collect::<Vec<_>>();
    let expected = Weekday::ALL.to_vec();
    if actual != expected {
        return Err(SettingsError::Validation(
            "availability must contain exactly mon, tue, wed, thu, fri, sat, sun".to_string(),
        ));
    }

    Ok(AvailabilitySettings::from_days(availability.days))
}

fn validate_availability_day(day: &AvailabilityDay) -> Result<(), SettingsError> {
    match (day.available, day.max_duration_minutes) {
        (true, Some(duration)) if is_allowed_availability_duration(duration) => Ok(()),
        (true, Some(duration)) => Err(SettingsError::Validation(format!(
            "availability duration {duration} is invalid; expected one of {ALLOWED_AVAILABILITY_MINUTES:?}"
        ))),
        (true, None) => Err(SettingsError::Validation(format!(
            "availability day '{}' requires maxDurationMinutes when available",
            day.weekday
        ))),
        (false, None) => Ok(()),
        (false, Some(_)) => Err(SettingsError::Validation(format!(
            "availability day '{}' must not define maxDurationMinutes when unavailable",
            day.weekday
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_availability, validate_optional_profile_text};
    use crate::domain::settings::{AvailabilityDay, AvailabilitySettings, SettingsError, Weekday};

    #[test]
    fn validate_optional_profile_text_trims_value() {
        let value = validate_optional_profile_text(
            "athletePrompt",
            Some("  stage race focus  ".to_string()),
            100,
        )
        .unwrap();

        assert_eq!(value, Some("stage race focus".to_string()));
    }

    #[test]
    fn validate_optional_profile_text_returns_none_for_blank_values() {
        assert_eq!(
            validate_optional_profile_text("athletePrompt", Some("".to_string()), 100).unwrap(),
            None
        );
        assert_eq!(
            validate_optional_profile_text("athletePrompt", Some("   ".to_string()), 100).unwrap(),
            None
        );
    }

    #[test]
    fn validate_optional_profile_text_allows_exact_max_length() {
        let exact = "a".repeat(5);

        let value =
            validate_optional_profile_text("athletePrompt", Some(exact.clone()), 5).unwrap();

        assert_eq!(value, Some(exact));
    }

    #[test]
    fn validate_optional_profile_text_rejects_over_max_length() {
        let error =
            validate_optional_profile_text("athletePrompt", Some("a".repeat(6)), 5).unwrap_err();

        assert_eq!(
            error,
            SettingsError::Validation("athletePrompt must be 5 characters or fewer".to_string())
        );
    }

    #[test]
    fn validate_availability_accepts_explicit_week() {
        let availability = AvailabilitySettings {
            configured: true,
            days: vec![
                AvailabilityDay {
                    weekday: Weekday::Mon,
                    available: true,
                    max_duration_minutes: Some(60),
                },
                AvailabilityDay {
                    weekday: Weekday::Tue,
                    available: false,
                    max_duration_minutes: None,
                },
                AvailabilityDay {
                    weekday: Weekday::Wed,
                    available: true,
                    max_duration_minutes: Some(90),
                },
                AvailabilityDay {
                    weekday: Weekday::Thu,
                    available: false,
                    max_duration_minutes: None,
                },
                AvailabilityDay {
                    weekday: Weekday::Fri,
                    available: true,
                    max_duration_minutes: Some(120),
                },
                AvailabilityDay {
                    weekday: Weekday::Sat,
                    available: true,
                    max_duration_minutes: Some(180),
                },
                AvailabilityDay {
                    weekday: Weekday::Sun,
                    available: false,
                    max_duration_minutes: None,
                },
            ],
        };

        assert_eq!(
            validate_availability(availability.clone()).unwrap(),
            availability
        );
    }

    #[test]
    fn validate_availability_rejects_invalid_duration_for_available_day() {
        let error = validate_availability(AvailabilitySettings {
            configured: true,
            days: vec![
                AvailabilityDay {
                    weekday: Weekday::Mon,
                    available: true,
                    max_duration_minutes: Some(45),
                },
                AvailabilityDay {
                    weekday: Weekday::Tue,
                    available: false,
                    max_duration_minutes: None,
                },
                AvailabilityDay {
                    weekday: Weekday::Wed,
                    available: false,
                    max_duration_minutes: None,
                },
                AvailabilityDay {
                    weekday: Weekday::Thu,
                    available: false,
                    max_duration_minutes: None,
                },
                AvailabilityDay {
                    weekday: Weekday::Fri,
                    available: false,
                    max_duration_minutes: None,
                },
                AvailabilityDay {
                    weekday: Weekday::Sat,
                    available: false,
                    max_duration_minutes: None,
                },
                AvailabilityDay {
                    weekday: Weekday::Sun,
                    available: false,
                    max_duration_minutes: None,
                },
            ],
        })
        .unwrap_err();

        assert!(
            matches!(error, SettingsError::Validation(message) if message.contains("availability duration 45 is invalid"))
        );
    }

    #[test]
    fn validate_availability_rejects_duration_for_unavailable_day() {
        let error = validate_availability(AvailabilitySettings {
            configured: true,
            days: vec![
                AvailabilityDay {
                    weekday: Weekday::Mon,
                    available: false,
                    max_duration_minutes: Some(60),
                },
                AvailabilityDay {
                    weekday: Weekday::Tue,
                    available: false,
                    max_duration_minutes: None,
                },
                AvailabilityDay {
                    weekday: Weekday::Wed,
                    available: false,
                    max_duration_minutes: None,
                },
                AvailabilityDay {
                    weekday: Weekday::Thu,
                    available: false,
                    max_duration_minutes: None,
                },
                AvailabilityDay {
                    weekday: Weekday::Fri,
                    available: false,
                    max_duration_minutes: None,
                },
                AvailabilityDay {
                    weekday: Weekday::Sat,
                    available: false,
                    max_duration_minutes: None,
                },
                AvailabilityDay {
                    weekday: Weekday::Sun,
                    available: false,
                    max_duration_minutes: None,
                },
            ],
        })
        .unwrap_err();

        assert!(
            matches!(error, SettingsError::Validation(message) if message.contains("must not define maxDurationMinutes"))
        );
    }

    #[test]
    fn validate_availability_derives_not_configured_when_all_days_unavailable() {
        let availability = AvailabilitySettings {
            configured: true,
            days: vec![
                AvailabilityDay {
                    weekday: Weekday::Mon,
                    available: false,
                    max_duration_minutes: None,
                },
                AvailabilityDay {
                    weekday: Weekday::Tue,
                    available: false,
                    max_duration_minutes: None,
                },
                AvailabilityDay {
                    weekday: Weekday::Wed,
                    available: false,
                    max_duration_minutes: None,
                },
                AvailabilityDay {
                    weekday: Weekday::Thu,
                    available: false,
                    max_duration_minutes: None,
                },
                AvailabilityDay {
                    weekday: Weekday::Fri,
                    available: false,
                    max_duration_minutes: None,
                },
                AvailabilityDay {
                    weekday: Weekday::Sat,
                    available: false,
                    max_duration_minutes: None,
                },
                AvailabilityDay {
                    weekday: Weekday::Sun,
                    available: false,
                    max_duration_minutes: None,
                },
            ],
        };

        let validated = validate_availability(availability).unwrap();

        assert!(!validated.configured);
        assert!(!validated.is_configured());
    }

    #[test]
    fn validate_availability_ignores_incoming_false_configured_when_days_are_available() {
        let availability = AvailabilitySettings {
            configured: false,
            days: vec![
                AvailabilityDay {
                    weekday: Weekday::Mon,
                    available: true,
                    max_duration_minutes: Some(60),
                },
                AvailabilityDay {
                    weekday: Weekday::Tue,
                    available: false,
                    max_duration_minutes: None,
                },
                AvailabilityDay {
                    weekday: Weekday::Wed,
                    available: false,
                    max_duration_minutes: None,
                },
                AvailabilityDay {
                    weekday: Weekday::Thu,
                    available: false,
                    max_duration_minutes: None,
                },
                AvailabilityDay {
                    weekday: Weekday::Fri,
                    available: false,
                    max_duration_minutes: None,
                },
                AvailabilityDay {
                    weekday: Weekday::Sat,
                    available: false,
                    max_duration_minutes: None,
                },
                AvailabilityDay {
                    weekday: Weekday::Sun,
                    available: false,
                    max_duration_minutes: None,
                },
            ],
        };

        let validated = validate_availability(availability).unwrap();

        assert!(validated.configured);
        assert!(validated.is_configured());
    }
}
