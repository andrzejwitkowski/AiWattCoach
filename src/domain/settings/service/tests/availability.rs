use super::super::*;
use super::support::{InMemoryUserSettingsRepository, TestClock};

#[tokio::test]
async fn update_availability_normalizes_inconsistent_configured_flag() {
    let settings = UserSettings::new_defaults("user-1".to_string(), 1_699_999_000);
    let repository = InMemoryUserSettingsRepository::with_settings(settings);
    let service = UserSettingsService::new(repository, TestClock);

    let updated = service
        .update_availability(
            "user-1",
            AvailabilitySettings {
                configured: true,
                days: super::super::super::model::default_availability_days(),
            },
        )
        .await
        .unwrap();

    assert!(!updated.availability.configured);
    assert!(!updated.availability.is_configured());
}
