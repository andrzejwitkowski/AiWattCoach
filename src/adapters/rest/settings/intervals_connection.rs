use crate::domain::settings::UserSettings;

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

#[cfg(test)]
mod tests {
    use super::normalize_optional_input;

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
}
