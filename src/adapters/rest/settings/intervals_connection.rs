use crate::domain::settings::{IntervalsConfig, UserSettings};

use super::dto::{
    test_connection_response, TestIntervalsConnectionRequest, TestIntervalsConnectionResponse,
};

pub(super) struct MergedCredentials {
    pub(super) api_key: String,
    pub(super) athlete_id: String,
    pub(super) used_saved_api_key: bool,
    pub(super) used_saved_athlete_id: bool,
}

pub(super) fn merge_connection_credentials(
    body: TestIntervalsConnectionRequest,
    current: &UserSettings,
) -> Result<MergedCredentials, TestIntervalsConnectionResponse> {
    let transient_api_key = normalize_optional_input(body.api_key);
    let transient_athlete_id = normalize_optional_input(body.athlete_id);

    let transient_api_key_not_provided = transient_api_key.is_none();
    let transient_athlete_id_not_provided = transient_athlete_id.is_none();

    let merged = merge_credentials(
        transient_api_key,
        transient_athlete_id,
        current.intervals.api_key.clone(),
        current.intervals.athlete_id.clone(),
    );

    merged.ok_or_else(|| {
        test_connection_response(
            false,
            "Both API key and athlete ID are required.",
            transient_api_key_not_provided && current.intervals.api_key.is_some(),
            transient_athlete_id_not_provided && current.intervals.athlete_id.is_some(),
            false,
        )
    })
}

fn normalize_optional_input(value: Option<String>) -> Option<String> {
    value.and_then(|candidate| {
        let trimmed = candidate.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn merge_credentials(
    transient_api_key: Option<String>,
    transient_athlete_id: Option<String>,
    saved_api_key: Option<String>,
    saved_athlete_id: Option<String>,
) -> Option<MergedCredentials> {
    let effective_api_key = transient_api_key.clone().or(saved_api_key.clone());
    let effective_athlete_id = transient_athlete_id.clone().or(saved_athlete_id.clone());

    let used_saved_api_key = transient_api_key.is_none() && saved_api_key.is_some();
    let used_saved_athlete_id = transient_athlete_id.is_none() && saved_athlete_id.is_some();

    match (effective_api_key, effective_athlete_id) {
        (Some(api_key), Some(athlete_id)) => Some(MergedCredentials {
            api_key,
            athlete_id,
            used_saved_api_key,
            used_saved_athlete_id,
        }),
        _ => None,
    }
}

pub(super) fn build_persisted_intervals_config(credentials: &MergedCredentials) -> IntervalsConfig {
    IntervalsConfig {
        api_key: Some(credentials.api_key.clone()),
        athlete_id: Some(credentials.athlete_id.clone()),
        connected: true,
    }
}

pub(super) fn should_persist_tested_credentials(
    credentials: &MergedCredentials,
    current: &UserSettings,
) -> bool {
    current.intervals.api_key.as_deref() != Some(credentials.api_key.as_str())
        || current.intervals.athlete_id.as_deref() != Some(credentials.athlete_id.as_str())
        || !current.intervals.connected
}

pub(super) fn can_persist_tested_credentials(
    initial: &UserSettings,
    latest: &UserSettings,
) -> bool {
    initial.intervals == latest.intervals
}

#[cfg(test)]
mod tests {
    use super::{
        can_persist_tested_credentials, normalize_optional_input,
        should_persist_tested_credentials, MergedCredentials,
    };
    use crate::domain::settings::UserSettings;

    #[test]
    fn normalize_optional_input_trims_non_empty_values() {
        assert_eq!(
            normalize_optional_input(Some("  athlete-123  ".to_string())),
            Some("athlete-123".to_string())
        );
    }

    #[test]
    fn normalize_optional_input_returns_none_for_whitespace_only_values() {
        assert_eq!(normalize_optional_input(Some("   ".to_string())), None);
    }

    #[test]
    fn normalize_optional_input_preserves_none() {
        assert_eq!(normalize_optional_input(None), None);
    }

    #[test]
    fn should_persist_tested_credentials_returns_true_when_connection_is_disconnected() {
        let mut current = UserSettings::new_defaults("user-1".to_string(), 1000);
        current.intervals.api_key = Some("saved-api-key".to_string());
        current.intervals.athlete_id = Some("saved-athlete-id".to_string());
        current.intervals.connected = false;

        let credentials = MergedCredentials {
            api_key: "saved-api-key".to_string(),
            athlete_id: "saved-athlete-id".to_string(),
            used_saved_api_key: true,
            used_saved_athlete_id: true,
        };

        assert!(should_persist_tested_credentials(&credentials, &current));
    }

    #[test]
    fn can_persist_tested_credentials_rejects_changed_saved_fallbacks() {
        let mut initial = UserSettings::new_defaults("user-1".to_string(), 1000);
        initial.intervals.api_key = Some("old-api-key".to_string());
        initial.intervals.athlete_id = Some("saved-athlete-id".to_string());

        let mut latest = initial.clone();
        latest.intervals.api_key = Some("new-api-key".to_string());

        assert!(!can_persist_tested_credentials(&initial, &latest));
    }

    #[test]
    fn can_persist_tested_credentials_allows_unchanged_intervals_state() {
        let mut initial = UserSettings::new_defaults("user-1".to_string(), 1000);
        initial.intervals.api_key = Some("saved-api-key".to_string());
        initial.intervals.athlete_id = Some("saved-athlete-id".to_string());

        let latest = initial.clone();

        assert!(can_persist_tested_credentials(&initial, &latest));
    }
}
