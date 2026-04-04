mod app;
mod identity;
mod intervals;
mod llm;
mod settings;

pub(crate) use app::{
    get_json, session_cookie, settings_test_app, settings_test_app_with_athlete_summary,
    settings_test_app_with_intervals, settings_test_app_with_services,
};
pub(crate) use athlete_summary::TestAthleteSummaryService;
pub(crate) use identity::{AdminIdentityErrorService, TestIdentityServiceWithSession};
pub(crate) use intervals::MockIntervalsConnectionTester;
pub(crate) use llm::{MockLlmChatService, TestLlmConfigProvider};
pub(crate) use settings::{RepositoryErrorSettingsService, TestSettingsService};

mod athlete_summary;

pub(crate) fn assert_log_entry_contains(logs: &str, expected_fragments: &[&str]) {
    let matched = logs.lines().any(|line| {
        expected_fragments
            .iter()
            .all(|fragment| line.contains(fragment))
    });

    assert!(
        matched,
        "expected one log entry to contain {expected_fragments:?}, logs were: {logs}"
    );
}
