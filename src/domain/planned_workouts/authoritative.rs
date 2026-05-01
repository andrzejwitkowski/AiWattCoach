use std::collections::HashSet;

use chrono::{Duration, NaiveDate};

use crate::domain::{
    completed_workouts::CompletedWorkoutRepository,
    external_sync::{
        CanonicalEntityKind, CanonicalEntityRef, ExternalProvider, ExternalSyncStateRepository,
    },
    planned_completed_links::PlannedCompletedWorkoutLinkRepository,
};

use super::{BoxFuture, PlannedWorkout, PlannedWorkoutError, PlannedWorkoutRepository};

#[derive(Clone)]
pub struct AuthoritativePlannedWorkoutRepository<Planned, Completed, Links, SyncStates>
where
    Planned: PlannedWorkoutRepository,
    Completed: CompletedWorkoutRepository,
    Links: PlannedCompletedWorkoutLinkRepository,
    SyncStates: ExternalSyncStateRepository,
{
    planned_workouts: Planned,
    completed_workouts: Completed,
    planned_completed_links: Links,
    sync_states: SyncStates,
}

impl<Planned, Completed, Links, SyncStates>
    AuthoritativePlannedWorkoutRepository<Planned, Completed, Links, SyncStates>
where
    Planned: PlannedWorkoutRepository,
    Completed: CompletedWorkoutRepository,
    Links: PlannedCompletedWorkoutLinkRepository,
    SyncStates: ExternalSyncStateRepository,
{
    pub fn new(
        planned_workouts: Planned,
        completed_workouts: Completed,
        planned_completed_links: Links,
        sync_states: SyncStates,
    ) -> Self {
        Self {
            planned_workouts,
            completed_workouts,
            planned_completed_links,
            sync_states,
        }
    }
}

impl<Planned, Completed, Links, SyncStates> PlannedWorkoutRepository
    for AuthoritativePlannedWorkoutRepository<Planned, Completed, Links, SyncStates>
where
    Planned: PlannedWorkoutRepository,
    Completed: CompletedWorkoutRepository,
    Links: PlannedCompletedWorkoutLinkRepository,
    SyncStates: ExternalSyncStateRepository,
{
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Vec<PlannedWorkout>, PlannedWorkoutError>> {
        let repository = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let workouts = repository
                .planned_workouts
                .list_by_user_id(&user_id)
                .await?;
            repository.filter_visible(&user_id, workouts, None).await
        })
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> BoxFuture<Result<Vec<PlannedWorkout>, PlannedWorkoutError>> {
        let repository = self.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            let workouts = repository
                .planned_workouts
                .list_by_user_id_and_date_range(&user_id, &oldest, &newest)
                .await?;
            repository
                .filter_visible(&user_id, workouts, Some((oldest, newest)))
                .await
        })
    }

    fn upsert(
        &self,
        workout: PlannedWorkout,
    ) -> BoxFuture<Result<PlannedWorkout, PlannedWorkoutError>> {
        self.planned_workouts.upsert(workout)
    }
}

impl<Planned, Completed, Links, SyncStates>
    AuthoritativePlannedWorkoutRepository<Planned, Completed, Links, SyncStates>
where
    Planned: PlannedWorkoutRepository,
    Completed: CompletedWorkoutRepository,
    Links: PlannedCompletedWorkoutLinkRepository,
    SyncStates: ExternalSyncStateRepository,
{
    async fn filter_visible(
        &self,
        user_id: &str,
        workouts: Vec<PlannedWorkout>,
        date_range: Option<(String, String)>,
    ) -> Result<Vec<PlannedWorkout>, PlannedWorkoutError> {
        if workouts.is_empty() {
            return Ok(Vec::new());
        }

        let canonical_entities = workouts
            .iter()
            .map(|workout| {
                CanonicalEntityRef::new(
                    CanonicalEntityKind::PlannedWorkout,
                    workout.planned_workout_id.clone(),
                )
            })
            .collect::<Vec<_>>();
        let externally_owned_ids = self
            .sync_states
            .find_by_provider_and_canonical_entities(
                user_id,
                ExternalProvider::Intervals,
                &canonical_entities,
            )
            .await
            .map_err(|error| PlannedWorkoutError::Repository(error.to_string()))?
            .into_iter()
            .map(|state| state.canonical_entity.entity_id)
            .collect::<HashSet<_>>();
        let completed_workouts = match date_range {
            Some((oldest, newest)) => {
                let (completed_oldest, completed_newest) =
                    expanded_completed_workout_range(&oldest, &newest)?;
                self.completed_workouts
                    .list_by_user_id_and_date_range(user_id, &completed_oldest, &completed_newest)
                    .await
                    .map_err(|error| PlannedWorkoutError::Repository(error.to_string()))?
            }
            None => self
                .completed_workouts
                .list_by_user_id(user_id)
                .await
                .map_err(|error| PlannedWorkoutError::Repository(error.to_string()))?,
        };
        // Planned-workout authority currently comes only from Intervals. Wahoo contributes
        // completed workouts but does not import planned workouts into this repository.
        let planned_ids_with_authoritative_completed = completed_workouts
            .iter()
            .filter_map(|workout| workout.planned_workout_id.clone())
            .collect::<HashSet<_>>();
        let authoritative_completed_ids = completed_workouts
            .iter()
            .map(|workout| workout.completed_workout_id.clone())
            .collect::<HashSet<_>>();
        let candidate_planned_ids = workouts
            .iter()
            .filter(|workout| {
                !externally_owned_ids.contains(&workout.planned_workout_id)
                    && !is_legacy_external_planned_workout_id(&workout.planned_workout_id)
                    && !planned_ids_with_authoritative_completed
                        .contains(&workout.planned_workout_id)
            })
            .map(|workout| workout.planned_workout_id.clone())
            .collect::<Vec<_>>();
        let planned_ids_with_authoritative_links = self
            .load_planned_ids_with_authoritative_links(
                user_id,
                &candidate_planned_ids,
                &authoritative_completed_ids,
            )
            .await?;

        let mut visible = Vec::with_capacity(workouts.len());
        for workout in workouts {
            if externally_owned_ids.contains(&workout.planned_workout_id)
                || is_legacy_external_planned_workout_id(&workout.planned_workout_id)
            {
                continue;
            }

            if planned_ids_with_authoritative_completed.contains(&workout.planned_workout_id)
                || planned_ids_with_authoritative_links.contains(&workout.planned_workout_id)
            {
                continue;
            }

            visible.push(workout);
        }

        Ok(visible)
    }

    async fn load_planned_ids_with_authoritative_links(
        &self,
        user_id: &str,
        planned_workout_ids: &[String],
        authoritative_completed_ids: &HashSet<String>,
    ) -> Result<HashSet<String>, PlannedWorkoutError> {
        let links = self
            .planned_completed_links
            .find_by_planned_workout_ids(user_id, planned_workout_ids)
            .await
            .map_err(|error| PlannedWorkoutError::Repository(error.to_string()))?;
        Ok(links
            .into_iter()
            .filter(|link| authoritative_completed_ids.contains(&link.completed_workout_id))
            .map(|link| link.planned_workout_id)
            .collect())
    }
}

fn expanded_completed_workout_range(
    oldest: &str,
    newest: &str,
) -> Result<(String, String), PlannedWorkoutError> {
    let oldest = NaiveDate::parse_from_str(oldest, "%Y-%m-%d")
        .map_err(|error| PlannedWorkoutError::Repository(error.to_string()))?;
    let newest = NaiveDate::parse_from_str(newest, "%Y-%m-%d")
        .map_err(|error| PlannedWorkoutError::Repository(error.to_string()))?;
    Ok((
        (oldest - Duration::days(1)).format("%Y-%m-%d").to_string(),
        (newest + Duration::days(1)).format("%Y-%m-%d").to_string(),
    ))
}

fn is_legacy_external_planned_workout_id(planned_workout_id: &str) -> bool {
    planned_workout_id.starts_with("intervals-event:")
}

#[cfg(test)]
mod tests {
    use crate::domain::{
        completed_workouts::{
            AuthoritativeCompletedWorkoutRepository, CompletedWorkout, CompletedWorkoutDetails,
            CompletedWorkoutMetrics, CompletedWorkoutRepository,
        },
        external_sync::{
            CanonicalEntityKind, CanonicalEntityRef, ConflictStatus, ExternalProvider,
            ExternalSyncState, ExternalSyncStateRepository, ExternalSyncStatus,
        },
        planned_completed_links::{
            PlannedCompletedWorkoutLink, PlannedCompletedWorkoutLinkMatchSource,
            PlannedCompletedWorkoutLinkRepository,
        },
    };

    use super::*;

    #[derive(Clone, Default)]
    struct TestCompletedWorkouts {
        workouts: Vec<CompletedWorkout>,
    }

    impl CompletedWorkoutRepository for TestCompletedWorkouts {
        fn find_by_user_id_and_completed_workout_id(
            &self,
            user_id: &str,
            completed_workout_id: &str,
        ) -> crate::domain::completed_workouts::BoxFuture<
            Result<
                Option<CompletedWorkout>,
                crate::domain::completed_workouts::CompletedWorkoutError,
            >,
        > {
            let workouts = self.workouts.clone();
            let user_id = user_id.to_string();
            let completed_workout_id = completed_workout_id.to_string();
            Box::pin(async move {
                Ok(workouts.into_iter().find(|workout| {
                    workout.user_id == user_id
                        && workout.completed_workout_id == completed_workout_id
                }))
            })
        }

        fn find_by_user_id_and_source_activity_id(
            &self,
            user_id: &str,
            source_activity_id: &str,
        ) -> crate::domain::completed_workouts::BoxFuture<
            Result<
                Option<CompletedWorkout>,
                crate::domain::completed_workouts::CompletedWorkoutError,
            >,
        > {
            let workouts = self.workouts.clone();
            let user_id = user_id.to_string();
            let source_activity_id = source_activity_id.to_string();
            Box::pin(async move {
                Ok(workouts.into_iter().find(|workout| {
                    workout.user_id == user_id
                        && workout.source_activity_id.as_deref()
                            == Some(source_activity_id.as_str())
                }))
            })
        }

        fn find_latest_by_user_id(
            &self,
            user_id: &str,
        ) -> crate::domain::completed_workouts::BoxFuture<
            Result<
                Option<CompletedWorkout>,
                crate::domain::completed_workouts::CompletedWorkoutError,
            >,
        > {
            let mut workouts = self
                .workouts
                .iter()
                .filter(|workout| workout.user_id == user_id)
                .cloned()
                .collect::<Vec<_>>();
            Box::pin(async move {
                workouts.sort_by(|left, right| right.start_date_local.cmp(&left.start_date_local));
                Ok(workouts.into_iter().next())
            })
        }

        fn list_by_user_id(
            &self,
            user_id: &str,
        ) -> crate::domain::completed_workouts::BoxFuture<
            Result<Vec<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
        > {
            let workouts = self
                .workouts
                .iter()
                .filter(|workout| workout.user_id == user_id)
                .cloned()
                .collect::<Vec<_>>();
            Box::pin(async move { Ok(workouts) })
        }

        fn list_by_user_id_and_date_range(
            &self,
            user_id: &str,
            oldest: &str,
            newest: &str,
        ) -> crate::domain::completed_workouts::BoxFuture<
            Result<Vec<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
        > {
            let workouts = self
                .workouts
                .iter()
                .filter(|workout| workout.user_id == user_id)
                .filter(|workout| {
                    let date = workout.start_date_local.get(..10).unwrap_or_default();
                    date >= oldest && date <= newest
                })
                .cloned()
                .collect::<Vec<_>>();
            Box::pin(async move { Ok(workouts) })
        }

        fn upsert(
            &self,
            workout: CompletedWorkout,
        ) -> crate::domain::completed_workouts::BoxFuture<
            Result<CompletedWorkout, crate::domain::completed_workouts::CompletedWorkoutError>,
        > {
            Box::pin(async move { Ok(workout) })
        }
    }

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

    #[derive(Clone, Default)]
    struct TestPlannedCompletedLinks {
        links: Vec<PlannedCompletedWorkoutLink>,
    }

    impl PlannedCompletedWorkoutLinkRepository for TestPlannedCompletedLinks {
        fn find_by_planned_workout_id(
            &self,
            user_id: &str,
            planned_workout_id: &str,
        ) -> crate::domain::planned_completed_links::BoxFuture<
            Result<
                Option<PlannedCompletedWorkoutLink>,
                crate::domain::planned_completed_links::PlannedCompletedWorkoutLinkError,
            >,
        > {
            let links = self.links.clone();
            let user_id = user_id.to_string();
            let planned_workout_id = planned_workout_id.to_string();
            Box::pin(async move {
                Ok(links.into_iter().find(|link| {
                    link.user_id == user_id && link.planned_workout_id == planned_workout_id
                }))
            })
        }

        fn find_by_completed_workout_id(
            &self,
            user_id: &str,
            completed_workout_id: &str,
        ) -> crate::domain::planned_completed_links::BoxFuture<
            Result<
                Option<PlannedCompletedWorkoutLink>,
                crate::domain::planned_completed_links::PlannedCompletedWorkoutLinkError,
            >,
        > {
            let links = self.links.clone();
            let user_id = user_id.to_string();
            let completed_workout_id = completed_workout_id.to_string();
            Box::pin(async move {
                Ok(links.into_iter().find(|link| {
                    link.user_id == user_id && link.completed_workout_id == completed_workout_id
                }))
            })
        }

        fn find_by_planned_workout_ids(
            &self,
            user_id: &str,
            planned_workout_ids: &[String],
        ) -> crate::domain::planned_completed_links::BoxFuture<
            Result<
                Vec<PlannedCompletedWorkoutLink>,
                crate::domain::planned_completed_links::PlannedCompletedWorkoutLinkError,
            >,
        > {
            let links = self.links.clone();
            let user_id = user_id.to_string();
            let planned_workout_ids = planned_workout_ids.to_vec();
            Box::pin(async move {
                Ok(links
                    .into_iter()
                    .filter(|link| {
                        link.user_id == user_id
                            && planned_workout_ids.contains(&link.planned_workout_id)
                    })
                    .collect())
            })
        }

        fn upsert(
            &self,
            link: PlannedCompletedWorkoutLink,
        ) -> crate::domain::planned_completed_links::BoxFuture<
            Result<
                PlannedCompletedWorkoutLink,
                crate::domain::planned_completed_links::PlannedCompletedWorkoutLinkError,
            >,
        > {
            Box::pin(async move { Ok(link) })
        }

        fn delete_by_completed_workout_id(
            &self,
            _user_id: &str,
            _completed_workout_id: &str,
        ) -> crate::domain::planned_completed_links::BoxFuture<
            Result<(), crate::domain::planned_completed_links::PlannedCompletedWorkoutLinkError>,
        > {
            Box::pin(async { Ok(()) })
        }
    }

    fn sample_planned_workout(id: &str, date: &str) -> PlannedWorkout {
        PlannedWorkout::new(
            id.to_string(),
            "user-1".to_string(),
            date.to_string(),
            crate::domain::planned_workouts::PlannedWorkoutContent { lines: Vec::new() },
        )
    }

    fn sample_completed_workout(
        id: &str,
        date: &str,
        planned_workout_id: Option<&str>,
    ) -> CompletedWorkout {
        CompletedWorkout::new(
            id.to_string(),
            "user-1".to_string(),
            format!("{date}T08:00:00"),
            Some(id.to_string()),
            planned_workout_id.map(ToString::to_string),
            None,
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

    #[tokio::test]
    async fn hides_planned_workout_when_authoritative_completed_links_to_it() {
        let planned_workouts = super::super::ports::NoopPlannedWorkoutRepository::default();
        planned_workouts
            .upsert(sample_planned_workout("planned-1", "2026-05-01"))
            .await
            .unwrap();
        let workouts = TestCompletedWorkouts {
            workouts: vec![sample_completed_workout(
                "wahoo-workout:1",
                "2026-05-01",
                Some("planned-1"),
            )],
        };
        let sync_states = TestSyncStates {
            states: vec![ExternalSyncState {
                user_id: "user-1".to_string(),
                provider: ExternalProvider::Wahoo,
                canonical_entity: CanonicalEntityRef::new(
                    CanonicalEntityKind::CompletedWorkout,
                    "wahoo-workout:1".to_string(),
                ),
                external_id: Some("1".to_string()),
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
            }],
        };
        let completed_workouts =
            AuthoritativeCompletedWorkoutRepository::new(workouts, sync_states.clone());
        let repository = AuthoritativePlannedWorkoutRepository::new(
            planned_workouts,
            completed_workouts,
            TestPlannedCompletedLinks::default(),
            sync_states,
        );

        let visible = repository.list_by_user_id("user-1").await.unwrap();

        assert!(visible.is_empty());
    }

    #[tokio::test]
    async fn hides_legacy_externally_imported_planned_workouts() {
        let planned_workouts = super::super::ports::NoopPlannedWorkoutRepository::default();
        planned_workouts
            .upsert(sample_planned_workout("intervals-event:144", "2026-05-01"))
            .await
            .unwrap();
        let repository = AuthoritativePlannedWorkoutRepository::new(
            planned_workouts,
            TestCompletedWorkouts::default(),
            TestPlannedCompletedLinks::default(),
            TestSyncStates::default(),
        );

        let visible = repository.list_by_user_id("user-1").await.unwrap();

        assert!(visible.is_empty());
    }

    #[tokio::test]
    async fn hides_planned_workout_when_link_points_to_authoritative_completed_workout() {
        let planned_workouts = super::super::ports::NoopPlannedWorkoutRepository::default();
        planned_workouts
            .upsert(sample_planned_workout("planned-1", "2026-05-01"))
            .await
            .unwrap();
        let workouts = TestCompletedWorkouts {
            workouts: vec![sample_completed_workout(
                "wahoo-workout:1",
                "2026-05-01",
                None,
            )],
        };
        let sync_states = TestSyncStates {
            states: vec![ExternalSyncState {
                user_id: "user-1".to_string(),
                provider: ExternalProvider::Wahoo,
                canonical_entity: CanonicalEntityRef::new(
                    CanonicalEntityKind::CompletedWorkout,
                    "wahoo-workout:1".to_string(),
                ),
                external_id: Some("1".to_string()),
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
            }],
        };
        let completed_workouts =
            AuthoritativeCompletedWorkoutRepository::new(workouts, sync_states.clone());
        let repository = AuthoritativePlannedWorkoutRepository::new(
            planned_workouts,
            completed_workouts,
            TestPlannedCompletedLinks {
                links: vec![PlannedCompletedWorkoutLink::new(
                    "user-1".to_string(),
                    "planned-1".to_string(),
                    "wahoo-workout:1".to_string(),
                    PlannedCompletedWorkoutLinkMatchSource::Explicit,
                    1_700_000_000,
                )],
            },
            sync_states,
        );

        let visible = repository.list_by_user_id("user-1").await.unwrap();

        assert!(visible.is_empty());
    }

    #[tokio::test]
    async fn hides_planned_workout_when_authoritative_completed_workout_crosses_date_boundary() {
        let planned_workouts = super::super::ports::NoopPlannedWorkoutRepository::default();
        planned_workouts
            .upsert(sample_planned_workout("planned-1", "2026-05-01"))
            .await
            .unwrap();
        let workouts = TestCompletedWorkouts {
            workouts: vec![sample_completed_workout(
                "wahoo-workout:1",
                "2026-05-02",
                Some("planned-1"),
            )],
        };
        let sync_states = TestSyncStates {
            states: vec![ExternalSyncState {
                user_id: "user-1".to_string(),
                provider: ExternalProvider::Wahoo,
                canonical_entity: CanonicalEntityRef::new(
                    CanonicalEntityKind::CompletedWorkout,
                    "wahoo-workout:1".to_string(),
                ),
                external_id: Some("1".to_string()),
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
            }],
        };
        let completed_workouts =
            AuthoritativeCompletedWorkoutRepository::new(workouts, sync_states.clone());
        let repository = AuthoritativePlannedWorkoutRepository::new(
            planned_workouts,
            completed_workouts,
            TestPlannedCompletedLinks::default(),
            sync_states,
        );

        let visible = repository
            .list_by_user_id_and_date_range("user-1", "2026-05-01", "2026-05-01")
            .await
            .unwrap();

        assert!(visible.is_empty());
    }
}
