mod ai_agents;
mod availability;
mod cycling;
mod intervals;
mod support;

#[tokio::test]
async fn find_settings_does_not_create_defaults_when_missing() {
    let repository = support::InMemoryUserSettingsRepository::default();
    let service = super::UserSettingsService::new(repository, support::TestClock);

    let found =
        crate::domain::settings::service::UserSettingsUseCases::find_settings(&service, "user-1")
            .await
            .unwrap();

    assert!(found.is_none());
}
