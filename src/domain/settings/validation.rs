use crate::domain::settings::SettingsError;

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
