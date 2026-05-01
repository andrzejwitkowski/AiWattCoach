use std::collections::HashSet;

use crate::domain::external_sync::{
    CanonicalEntityKind, CanonicalEntityRef, ExternalProvider, ExternalSyncStateRepository,
};

use super::selection::select_visible_workouts_by_day;
use super::{BoxFuture, CompletedWorkout, CompletedWorkoutError, CompletedWorkoutRepository};

#[derive(Clone)]
pub struct AuthoritativeCompletedWorkoutRepository<Workouts, SyncStates>
where
    Workouts: CompletedWorkoutRepository,
    SyncStates: ExternalSyncStateRepository,
{
    workouts: Workouts,
    sync_states: SyncStates,
}

impl<Workouts, SyncStates> AuthoritativeCompletedWorkoutRepository<Workouts, SyncStates>
where
    Workouts: CompletedWorkoutRepository,
    SyncStates: ExternalSyncStateRepository,
{
    pub fn new(workouts: Workouts, sync_states: SyncStates) -> Self {
        Self {
            workouts,
            sync_states,
        }
    }
}

impl<Workouts, SyncStates> CompletedWorkoutRepository
    for AuthoritativeCompletedWorkoutRepository<Workouts, SyncStates>
where
    Workouts: CompletedWorkoutRepository,
    SyncStates: ExternalSyncStateRepository,
{
    fn find_by_user_id_and_completed_workout_id(
        &self,
        user_id: &str,
        completed_workout_id: &str,
    ) -> BoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
        let repository = self.clone();
        let user_id = user_id.to_string();
        let completed_workout_id = completed_workout_id.to_string();
        Box::pin(async move {
            let Some(workout) = repository
                .workouts
                .find_by_user_id_and_completed_workout_id(&user_id, &completed_workout_id)
                .await?
            else {
                return Ok(None);
            };
            let same_day_workouts = repository
                .load_visibility_candidates(&user_id, &workout)
                .await?;
            repository
                .filter_visible(&user_id, same_day_workouts)
                .await
                .map(|workouts| {
                    workouts
                        .into_iter()
                        .find(|workout| workout.completed_workout_id == completed_workout_id)
                })
        })
    }

    fn find_by_user_id_and_source_activity_id(
        &self,
        user_id: &str,
        source_activity_id: &str,
    ) -> BoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
        let repository = self.clone();
        let user_id = user_id.to_string();
        let source_activity_id = source_activity_id.to_string();
        Box::pin(async move {
            let Some(workout) = repository
                .workouts
                .find_by_user_id_and_source_activity_id(&user_id, &source_activity_id)
                .await?
            else {
                return Ok(None);
            };
            let same_day_workouts = repository
                .load_visibility_candidates(&user_id, &workout)
                .await?;
            repository
                .filter_visible(&user_id, same_day_workouts)
                .await
                .map(|workouts| {
                    workouts.into_iter().find(|workout| {
                        workout.source_activity_id.as_deref() == Some(source_activity_id.as_str())
                    })
                })
        })
    }

    fn find_latest_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
        let repository = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let workouts = repository.workouts.list_by_user_id(&user_id).await?;
            let mut visible = repository.filter_visible(&user_id, workouts).await?;
            visible.sort_by(|left, right| {
                right
                    .start_date_local
                    .cmp(&left.start_date_local)
                    .then_with(|| right.completed_workout_id.cmp(&left.completed_workout_id))
            });
            Ok(visible.into_iter().next())
        })
    }

    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>> {
        let repository = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let workouts = repository.workouts.list_by_user_id(&user_id).await?;
            repository.filter_visible(&user_id, workouts).await
        })
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> BoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>> {
        let repository = self.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            let workouts = repository
                .workouts
                .list_by_user_id_and_date_range(&user_id, &oldest, &newest)
                .await?;
            repository.filter_visible(&user_id, workouts).await
        })
    }

    fn upsert(
        &self,
        workout: CompletedWorkout,
    ) -> BoxFuture<Result<CompletedWorkout, CompletedWorkoutError>> {
        self.workouts.upsert(workout)
    }
}

impl<Workouts, SyncStates> AuthoritativeCompletedWorkoutRepository<Workouts, SyncStates>
where
    Workouts: CompletedWorkoutRepository,
    SyncStates: ExternalSyncStateRepository,
{
    async fn load_visibility_candidates(
        &self,
        user_id: &str,
        workout: &CompletedWorkout,
    ) -> Result<Vec<CompletedWorkout>, CompletedWorkoutError> {
        let Some(date) = workout.start_date_local.get(..10) else {
            return Ok(vec![workout.clone()]);
        };

        self.workouts
            .list_by_user_id_and_date_range(user_id, date, date)
            .await
    }

    async fn filter_visible(
        &self,
        user_id: &str,
        workouts: Vec<CompletedWorkout>,
    ) -> Result<Vec<CompletedWorkout>, CompletedWorkoutError> {
        if workouts.is_empty() {
            return Ok(Vec::new());
        }

        let canonical_entities = workouts
            .iter()
            .map(|workout| {
                CanonicalEntityRef::new(
                    CanonicalEntityKind::CompletedWorkout,
                    workout.completed_workout_id.clone(),
                )
            })
            .collect::<Vec<_>>();
        let wahoo_entity_ids = self
            .sync_states
            .find_by_provider_and_canonical_entities(
                user_id,
                ExternalProvider::Wahoo,
                &canonical_entities,
            )
            .await
            .map_err(|error| CompletedWorkoutError::Repository(error.to_string()))?
            .into_iter()
            .map(|state| state.canonical_entity.entity_id)
            .collect::<HashSet<_>>();
        Ok(select_visible_workouts_by_day(workouts, &wahoo_entity_ids))
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::{
        completed_workouts::{
            CompletedWorkoutDetails, CompletedWorkoutMetrics, CompletedWorkoutSeries,
            CompletedWorkoutStream,
        },
        external_sync::{
            CanonicalEntityKind, CanonicalEntityRef, ConflictStatus, ExternalProvider,
            ExternalSyncState, ExternalSyncStateRepository, ExternalSyncStatus,
        },
    };

    use super::*;

    #[derive(Clone, Default)]
    struct TestSyncStates {
        states: Vec<ExternalSyncState>,
    }

    impl ExternalSyncStateRepository for TestSyncStates {
        fn upsert(
            &self,
            state: ExternalSyncState,
        ) -> crate::domain::external_sync::BoxFuture<
            Result<ExternalSyncState, crate::domain::external_sync::ExternalSyncRepositoryError>,
        > {
            Box::pin(async move { Ok(state) })
        }

        fn find_by_canonical_entities(
            &self,
            user_id: &str,
            canonical_entities: &[CanonicalEntityRef],
        ) -> crate::domain::external_sync::BoxFuture<
            Result<
                Vec<ExternalSyncState>,
                crate::domain::external_sync::ExternalSyncRepositoryError,
            >,
        > {
            let states = self.states.clone();
            let user_id = user_id.to_string();
            let canonical_entities = canonical_entities.to_vec();
            Box::pin(async move {
                Ok(states
                    .into_iter()
                    .filter(|state| state.user_id == user_id)
                    .filter(|state| canonical_entities.contains(&state.canonical_entity))
                    .collect())
            })
        }

        fn find_by_provider_and_canonical_entity(
            &self,
            _user_id: &str,
            _provider: ExternalProvider,
            _canonical_entity: &CanonicalEntityRef,
        ) -> crate::domain::external_sync::BoxFuture<
            Result<
                Option<ExternalSyncState>,
                crate::domain::external_sync::ExternalSyncRepositoryError,
            >,
        > {
            Box::pin(async { Ok(None) })
        }

        fn find_by_provider_and_canonical_entities(
            &self,
            user_id: &str,
            provider: ExternalProvider,
            canonical_entities: &[CanonicalEntityRef],
        ) -> crate::domain::external_sync::BoxFuture<
            Result<
                Vec<ExternalSyncState>,
                crate::domain::external_sync::ExternalSyncRepositoryError,
            >,
        > {
            let states = self.states.clone();
            let user_id = user_id.to_string();
            let canonical_entities = canonical_entities.to_vec();
            Box::pin(async move {
                Ok(states
                    .into_iter()
                    .filter(|state| state.user_id == user_id)
                    .filter(|state| state.provider == provider)
                    .filter(|state| canonical_entities.contains(&state.canonical_entity))
                    .collect())
            })
        }

        fn delete_by_provider_and_canonical_entity(
            &self,
            _user_id: &str,
            _provider: ExternalProvider,
            _canonical_entity: &CanonicalEntityRef,
        ) -> crate::domain::external_sync::BoxFuture<
            Result<(), crate::domain::external_sync::ExternalSyncRepositoryError>,
        > {
            Box::pin(async { Ok(()) })
        }

        fn find_by_wahoo_plan_id(
            &self,
            user_id: &str,
            wahoo_plan_id: i64,
        ) -> crate::domain::external_sync::BoxFuture<
            Result<
                Option<ExternalSyncState>,
                crate::domain::external_sync::ExternalSyncRepositoryError,
            >,
        > {
            let states = self.states.clone();
            let user_id = user_id.to_string();
            Box::pin(async move {
                Ok(states.into_iter().find(|state| {
                    state.user_id == user_id
                        && state.provider == ExternalProvider::Wahoo
                        && state.wahoo_plan_id == Some(wahoo_plan_id)
                }))
            })
        }

        fn find_by_wahoo_workout_token(
            &self,
            user_id: &str,
            wahoo_workout_token: &str,
        ) -> crate::domain::external_sync::BoxFuture<
            Result<
                Option<ExternalSyncState>,
                crate::domain::external_sync::ExternalSyncRepositoryError,
            >,
        > {
            let states = self.states.clone();
            let user_id = user_id.to_string();
            let wahoo_workout_token = wahoo_workout_token.to_string();
            Box::pin(async move {
                Ok(states.into_iter().find(|state| {
                    state.user_id == user_id
                        && state.provider == ExternalProvider::Wahoo
                        && state.wahoo_workout_token.as_deref()
                            == Some(wahoo_workout_token.as_str())
                }))
            })
        }

        fn find_by_provider_and_external_id(
            &self,
            user_id: &str,
            provider: ExternalProvider,
            external_id: &str,
        ) -> crate::domain::external_sync::BoxFuture<
            Result<
                Option<ExternalSyncState>,
                crate::domain::external_sync::ExternalSyncRepositoryError,
            >,
        > {
            let states = self.states.clone();
            let user_id = user_id.to_string();
            let external_id = external_id.to_string();
            Box::pin(async move {
                Ok(states.into_iter().find(|state| {
                    state.user_id == user_id
                        && state.provider == provider
                        && state.external_id.as_deref() == Some(external_id.as_str())
                }))
            })
        }
    }

    fn sample_workout(id: &str, date: &str) -> CompletedWorkout {
        CompletedWorkout::new(
            id.to_string(),
            "user-1".to_string(),
            format!("{date}T08:00:00"),
            Some(id.to_string()),
            None,
            Some(id.to_string()),
            None,
            Some("Ride".to_string()),
            None,
            false,
            Some(3600),
            Some(1000.0),
            CompletedWorkoutMetrics {
                training_stress_score: Some(10),
                normalized_power_watts: None,
                intensity_factor: None,
                efficiency_factor: None,
                variability_index: None,
                average_power_watts: None,
                ftp_watts: None,
                total_work_joules: None,
                calories: None,
                trimp: None,
                power_load: None,
                heart_rate_load: None,
                pace_load: None,
                strain_score: None,
            },
            CompletedWorkoutDetails {
                intervals: Vec::new(),
                interval_groups: Vec::new(),
                streams: Vec::new(),
                interval_summary: Vec::new(),
                skyline_chart: Vec::new(),
                power_zone_times: Vec::new(),
                heart_rate_zone_times: Vec::new(),
                pace_zone_times: Vec::new(),
                gap_zone_times: Vec::new(),
            },
            None,
        )
    }

    fn with_power_stream(mut workout: CompletedWorkout) -> CompletedWorkout {
        workout.details.streams.push(CompletedWorkoutStream {
            stream_type: "watts".to_string(),
            name: Some("Power".to_string()),
            primary_series: Some(CompletedWorkoutSeries::Integers(vec![180, 240, 310])),
            secondary_series: None,
            value_type_is_array: false,
            custom: false,
            all_null: false,
        });
        workout
    }

    fn wahoo_sync_state(id: &str) -> ExternalSyncState {
        ExternalSyncState {
            user_id: "user-1".to_string(),
            provider: ExternalProvider::Wahoo,
            canonical_entity: CanonicalEntityRef::new(
                CanonicalEntityKind::CompletedWorkout,
                id.to_string(),
            ),
            external_id: Some(id.to_string()),
            wahoo_plan_external_id: None,
            wahoo_plan_id: None,
            wahoo_workout_id: None,
            wahoo_workout_token: None,
            sync_status: ExternalSyncStatus::Synced,
            last_synced_payload_hash: None,
            last_seen_remote_payload_hash: None,
            last_error: None,
            last_synced_at_epoch_seconds: None,
            last_seen_remote_at_epoch_seconds: None,
            conflict_status: ConflictStatus::InSync,
        }
    }

    #[tokio::test]
    async fn keeps_wahoo_visible_when_wahoo_has_power_details() {
        let workouts = super::super::ports::NoopCompletedWorkoutRepository::default();
        workouts
            .upsert(with_power_stream(sample_workout(
                "intervals-activity:1",
                "2026-05-01",
            )))
            .await
            .unwrap();
        workouts
            .upsert(with_power_stream(sample_workout(
                "wahoo-workout:2",
                "2026-05-01",
            )))
            .await
            .unwrap();
        let sync_states = TestSyncStates {
            states: vec![wahoo_sync_state("wahoo-workout:2")],
        };
        let repository = AuthoritativeCompletedWorkoutRepository::new(workouts, sync_states);

        let visible = repository.list_by_user_id("user-1").await.unwrap();

        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].completed_workout_id, "wahoo-workout:2");
    }

    #[tokio::test]
    async fn prefers_other_provider_when_wahoo_lacks_power_details() {
        let workouts = super::super::ports::NoopCompletedWorkoutRepository::default();
        workouts
            .upsert(with_power_stream(sample_workout(
                "intervals-activity:1",
                "2026-05-01",
            )))
            .await
            .unwrap();
        workouts
            .upsert(sample_workout("wahoo-workout:2", "2026-05-01"))
            .await
            .unwrap();
        let sync_states = TestSyncStates {
            states: vec![wahoo_sync_state("wahoo-workout:2")],
        };
        let repository = AuthoritativeCompletedWorkoutRepository::new(workouts, sync_states);

        let visible = repository.list_by_user_id("user-1").await.unwrap();

        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].completed_workout_id, "intervals-activity:1");

        let by_id = repository
            .find_by_user_id_and_completed_workout_id("user-1", "intervals-activity:1")
            .await
            .unwrap();
        assert_eq!(
            by_id
                .as_ref()
                .map(|workout| workout.completed_workout_id.as_str()),
            Some("intervals-activity:1")
        );

        let hidden_wahoo = repository
            .find_by_user_id_and_completed_workout_id("user-1", "wahoo-workout:2")
            .await
            .unwrap();
        assert_eq!(hidden_wahoo, None);

        let by_source_activity = repository
            .find_by_user_id_and_source_activity_id("user-1", "intervals-activity:1")
            .await
            .unwrap();
        assert_eq!(
            by_source_activity
                .as_ref()
                .map(|workout| workout.completed_workout_id.as_str()),
            Some("intervals-activity:1")
        );
    }

    #[tokio::test]
    async fn prefers_wahoo_when_nobody_has_power_details() {
        let workouts = super::super::ports::NoopCompletedWorkoutRepository::default();
        workouts
            .upsert(sample_workout("intervals-activity:1", "2026-05-01"))
            .await
            .unwrap();
        workouts
            .upsert(sample_workout("wahoo-workout:2", "2026-05-01"))
            .await
            .unwrap();
        let sync_states = TestSyncStates {
            states: vec![wahoo_sync_state("wahoo-workout:2")],
        };
        let repository = AuthoritativeCompletedWorkoutRepository::new(workouts, sync_states);

        let visible = repository.list_by_user_id("user-1").await.unwrap();

        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].completed_workout_id, "wahoo-workout:2");
    }

    #[tokio::test]
    async fn keeps_days_independent() {
        let workouts = super::super::ports::NoopCompletedWorkoutRepository::default();
        workouts
            .upsert(with_power_stream(sample_workout(
                "intervals-activity:1",
                "2026-05-01",
            )))
            .await
            .unwrap();
        workouts
            .upsert(sample_workout("wahoo-workout:2", "2026-05-01"))
            .await
            .unwrap();
        workouts
            .upsert(sample_workout("wahoo-workout:3", "2026-05-02"))
            .await
            .unwrap();
        let sync_states = TestSyncStates {
            states: vec![
                wahoo_sync_state("wahoo-workout:2"),
                wahoo_sync_state("wahoo-workout:3"),
            ],
        };
        let repository = AuthoritativeCompletedWorkoutRepository::new(workouts, sync_states);

        let visible = repository.list_by_user_id("user-1").await.unwrap();

        assert_eq!(visible.len(), 2);
        assert!(visible
            .iter()
            .any(|workout| workout.completed_workout_id == "intervals-activity:1"));
        assert!(visible
            .iter()
            .any(|workout| workout.completed_workout_id == "wahoo-workout:3"));
    }
}
