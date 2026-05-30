use std::sync::Arc;

use super::super::*;
use super::support::{
    FailingFtpHistoryRepository, InMemoryUserSettingsRepository, RecordingCacheRepository,
    RecordingFtpHistoryRepository, RecordingTrainingLoadRecomputeService, TestClock,
};
use crate::domain::training_load::{FtpHistoryEntry, FtpHistoryRepository, FtpSource};

#[tokio::test]
async fn update_cycling_seeds_initial_ftp_history_and_recomputes_from_settings_created_date() {
    let mut settings = UserSettings::new_defaults("user-1".to_string(), 1_699_315_200);
    settings.cycling.ftp_watts = Some(280);
    let repository = InMemoryUserSettingsRepository::with_settings(settings);
    let cache_repository = Arc::new(RecordingCacheRepository::default());
    let ftp_history_repository = RecordingFtpHistoryRepository::default();
    let recompute_service = Arc::new(RecordingTrainingLoadRecomputeService::default());
    let service = UserSettingsService::new(repository, TestClock)
        .with_llm_context_cache_repository(cache_repository.clone())
        .with_ftp_history_repository(ftp_history_repository.clone())
        .with_training_load_recompute_service(recompute_service.clone());

    service
        .update_cycling(
            "user-1",
            CyclingSettings {
                ftp_watts: Some(290),
                ..CyclingSettings::default()
            },
        )
        .await
        .unwrap();

    let history = ftp_history_repository.stored();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].effective_from_date, "2023-11-07");
    assert_eq!(history[0].ftp_watts, 280);
    assert_eq!(history[1].effective_from_date, "2023-11-14");
    assert_eq!(history[1].ftp_watts, 290);
    assert_eq!(history[1].source, FtpSource::Settings);
    assert_eq!(
        recompute_service.calls(),
        vec![(
            "user-1".to_string(),
            "2023-11-07".to_string(),
            1_700_000_000,
        )]
    );
    assert_eq!(cache_repository.deleted_users(), vec!["user-1".to_string()]);
}

#[tokio::test]
async fn update_cycling_skips_ftp_history_when_ftp_is_unchanged() {
    let mut settings = UserSettings::new_defaults("user-1".to_string(), 1_699_315_200);
    settings.cycling.ftp_watts = Some(280);
    let repository = InMemoryUserSettingsRepository::with_settings(settings);
    let ftp_history_repository = RecordingFtpHistoryRepository::default();
    FtpHistoryRepository::upsert(
        &ftp_history_repository,
        FtpHistoryEntry {
            user_id: "user-1".to_string(),
            effective_from_date: "2023-11-07".to_string(),
            ftp_watts: 280,
            source: FtpSource::Settings,
            created_at_epoch_seconds: 1_699_315_200,
            updated_at_epoch_seconds: 1_699_315_200,
        },
    )
    .await
    .unwrap();
    let recompute_service = Arc::new(RecordingTrainingLoadRecomputeService::default());
    let service = UserSettingsService::new(repository, TestClock)
        .with_ftp_history_repository(ftp_history_repository.clone())
        .with_training_load_recompute_service(recompute_service.clone());

    service
        .update_cycling(
            "user-1",
            CyclingSettings {
                ftp_watts: Some(280),
                ..CyclingSettings::default()
            },
        )
        .await
        .unwrap();

    assert_eq!(ftp_history_repository.stored().len(), 1);
    assert!(recompute_service.calls().is_empty());
}

#[tokio::test]
async fn update_cycling_keeps_saved_settings_when_ftp_history_write_fails() {
    let mut settings = UserSettings::new_defaults("user-1".to_string(), 1_699_315_200);
    settings.cycling.ftp_watts = Some(280);
    let repository = InMemoryUserSettingsRepository::with_settings(settings);
    let service = UserSettingsService::new(repository.clone(), TestClock)
        .with_ftp_history_repository(FailingFtpHistoryRepository);

    let updated = service
        .update_cycling(
            "user-1",
            CyclingSettings {
                ftp_watts: Some(290),
                ..CyclingSettings::default()
            },
        )
        .await
        .unwrap();

    assert_eq!(updated.cycling.ftp_watts, Some(290));
    assert_eq!(
        repository
            .find_by_user_id("user-1")
            .await
            .unwrap()
            .and_then(|settings| settings.cycling.ftp_watts),
        Some(290)
    );
}

#[tokio::test]
async fn update_cycling_clears_effective_ftp_history_and_recomputes() {
    let mut settings = UserSettings::new_defaults("user-1".to_string(), 1_699_315_200);
    settings.cycling.ftp_watts = Some(280);
    let repository = InMemoryUserSettingsRepository::with_settings(settings);
    let ftp_history_repository = RecordingFtpHistoryRepository::default();
    FtpHistoryRepository::upsert(
        &ftp_history_repository,
        FtpHistoryEntry {
            user_id: "user-1".to_string(),
            effective_from_date: "2023-11-07".to_string(),
            ftp_watts: 280,
            source: FtpSource::Settings,
            created_at_epoch_seconds: 1_699_315_200,
            updated_at_epoch_seconds: 1_699_315_200,
        },
    )
    .await
    .unwrap();
    let recompute_service = Arc::new(RecordingTrainingLoadRecomputeService::default());
    let service = UserSettingsService::new(repository, TestClock)
        .with_ftp_history_repository(ftp_history_repository.clone())
        .with_training_load_recompute_service(recompute_service.clone());

    let updated = service
        .update_cycling(
            "user-1",
            CyclingSettings {
                ftp_watts: None,
                ..CyclingSettings::default()
            },
        )
        .await
        .unwrap();

    assert_eq!(updated.cycling.ftp_watts, None);
    let history = ftp_history_repository.stored();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].ftp_watts, 280);
    assert_eq!(
        recompute_service.calls(),
        vec![(
            "user-1".to_string(),
            "2023-11-07".to_string(),
            1_700_000_000,
        )]
    );
}
