use crate::domain::settings::SettingsError;

use crate::domain::llm::LlmProvider;

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

#[cfg(test)]
mod tests {
    use super::validate_optional_profile_text;
    use crate::domain::settings::SettingsError;

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
}
