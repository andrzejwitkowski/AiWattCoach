use std::collections::HashMap;

use crate::domain::{
    completed_workouts::{
        canonical_completed_workout_id, completed_workout_activity_id, CompletedWorkout,
        CompletedWorkoutRepository,
    },
    workout_summary::{
        BoxFuture, CompletedWorkoutAliasScope, CompletedWorkoutTargetUseCases,
        ResolvedCompletedWorkoutTarget, WorkoutSummaryError,
    },
};

const ALIAS_SCOPE_MARGIN_DAYS: i64 = 7;

#[derive(Clone)]
pub struct CompletedWorkoutTargetAdapter<Repo> {
    repository: Repo,
}

impl<Repo> CompletedWorkoutTargetAdapter<Repo> {
    pub fn new(repository: Repo) -> Self {
        Self { repository }
    }
}

impl<Repo> CompletedWorkoutTargetUseCases for CompletedWorkoutTargetAdapter<Repo>
where
    Repo: CompletedWorkoutRepository + Clone + Send + Sync + 'static,
{
    fn is_completed_workout_target(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<bool, WorkoutSummaryError>> {
        let repository = self.repository.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let resolved = resolve_completed_workout(&repository, &user_id, &workout_id).await?;
            Ok(resolved.is_some())
        })
    }

    fn load_completed_workout(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<Option<CompletedWorkout>, WorkoutSummaryError>> {
        let repository = self.repository.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move { resolve_completed_workout(&repository, &user_id, &workout_id).await })
    }

    fn resolve_completed_workout_target(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<Option<ResolvedCompletedWorkoutTarget>, WorkoutSummaryError>> {
        let repository = self.repository.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let Some(workout) =
                resolve_completed_workout(&repository, &user_id, &workout_id).await?
            else {
                return Ok(None);
            };

            let mut equivalent_workout_ids =
                equivalent_workout_ids_for_workout(&repository, &user_id, &workout, None).await?;
            let preferred_workout_id = workout
                .source_activity_id
                .clone()
                .unwrap_or_else(|| workout.completed_workout_id.clone());
            push_unique_workout_id(&mut equivalent_workout_ids, preferred_workout_id.clone());

            Ok(Some(ResolvedCompletedWorkoutTarget {
                preferred_workout_id,
                equivalent_workout_ids,
            }))
        })
    }

    fn resolve_completed_workout_target_in_scope(
        &self,
        user_id: &str,
        workout_id: &str,
        alias_scope: &CompletedWorkoutAliasScope,
    ) -> BoxFuture<Result<Option<ResolvedCompletedWorkoutTarget>, WorkoutSummaryError>> {
        let repository = self.repository.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let alias_scope = alias_scope.clone();
        Box::pin(async move {
            let siblings = load_alias_scope_siblings(&repository, &user_id, &alias_scope).await?;
            let Some(workout) =
                resolve_completed_workout_from_siblings(&siblings, &user_id, &workout_id)
            else {
                return Ok(None);
            };

            let mut equivalent_workout_ids =
                equivalent_workout_ids_for_workout_with_siblings(&workout, &siblings);
            let preferred_workout_id = workout
                .source_activity_id
                .clone()
                .unwrap_or_else(|| workout.completed_workout_id.clone());
            push_unique_workout_id(&mut equivalent_workout_ids, preferred_workout_id.clone());

            Ok(Some(ResolvedCompletedWorkoutTarget {
                preferred_workout_id,
                equivalent_workout_ids,
            }))
        })
    }

    fn resolve_completed_workout_targets_in_scope(
        &self,
        user_id: &str,
        workout_ids: &[String],
        alias_scope: &CompletedWorkoutAliasScope,
    ) -> BoxFuture<Result<HashMap<String, ResolvedCompletedWorkoutTarget>, WorkoutSummaryError>>
    {
        let repository = self.repository.clone();
        let user_id = user_id.to_string();
        let workout_ids = workout_ids.to_vec();
        let alias_scope = alias_scope.clone();
        Box::pin(async move {
            let siblings = load_alias_scope_siblings(&repository, &user_id, &alias_scope).await?;
            let mut resolved = HashMap::new();

            for workout_id in workout_ids {
                let Some(workout) =
                    resolve_completed_workout_from_siblings(&siblings, &user_id, &workout_id)
                else {
                    continue;
                };

                let mut equivalent_workout_ids =
                    equivalent_workout_ids_for_workout_with_siblings(&workout, &siblings);
                let preferred_workout_id = workout
                    .source_activity_id
                    .clone()
                    .unwrap_or_else(|| workout.completed_workout_id.clone());
                push_unique_workout_id(&mut equivalent_workout_ids, preferred_workout_id.clone());

                resolved.insert(
                    workout_id,
                    ResolvedCompletedWorkoutTarget {
                        preferred_workout_id,
                        equivalent_workout_ids,
                    },
                );
            }

            Ok(resolved)
        })
    }
}

async fn load_alias_scope_siblings<Repo>(
    repository: &Repo,
    user_id: &str,
    alias_scope: &CompletedWorkoutAliasScope,
) -> Result<Vec<CompletedWorkout>, WorkoutSummaryError>
where
    Repo: CompletedWorkoutRepository + Clone + Send + Sync + 'static,
{
    let expanded_scope = alias_scope.with_alias_margin_days(ALIAS_SCOPE_MARGIN_DAYS);
    repository
        .list_by_user_id_and_date_range(user_id, &expanded_scope.oldest, &expanded_scope.newest)
        .await
        .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))
}

fn resolve_completed_workout_from_siblings(
    siblings: &[CompletedWorkout],
    user_id: &str,
    workout_id: &str,
) -> Option<CompletedWorkout> {
    if let Some(workout) = siblings.iter().find(|workout| {
        workout.user_id == user_id && workout.source_activity_id.as_deref() == Some(workout_id)
    }) {
        return Some(workout.clone());
    }

    let canonical_id = canonical_completed_workout_id(workout_id);
    siblings
        .iter()
        .find(|workout| workout.user_id == user_id && workout.completed_workout_id == canonical_id)
        .cloned()
}

async fn equivalent_workout_ids_for_workout<Repo>(
    repository: &Repo,
    user_id: &str,
    workout: &CompletedWorkout,
    alias_scope: Option<&CompletedWorkoutAliasScope>,
) -> Result<Vec<String>, WorkoutSummaryError>
where
    Repo: CompletedWorkoutRepository + Clone + Send + Sync + 'static,
{
    let siblings = if let Some(scope) = alias_scope {
        load_alias_scope_siblings(repository, user_id, scope).await?
    } else {
        repository
            .list_by_user_id(user_id)
            .await
            .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?
    };

    Ok(equivalent_workout_ids_for_workout_with_siblings(
        workout, &siblings,
    ))
}

fn equivalent_workout_ids_for_workout_with_siblings(
    workout: &CompletedWorkout,
    siblings: &[CompletedWorkout],
) -> Vec<String> {
    let mut equivalent_workout_ids = Vec::new();
    push_unique_workout_id(
        &mut equivalent_workout_ids,
        workout
            .source_activity_id
            .clone()
            .unwrap_or_else(|| workout.completed_workout_id.clone()),
    );
    push_unique_workout_id(
        &mut equivalent_workout_ids,
        workout.completed_workout_id.clone(),
    );
    if let Some(external_id) = workout.external_id.clone() {
        push_unique_workout_id(&mut equivalent_workout_ids, external_id);
    }

    for sibling in siblings {
        if same_completed_workout_family(workout, sibling) {
            push_unique_workout_id(
                &mut equivalent_workout_ids,
                sibling
                    .source_activity_id
                    .clone()
                    .unwrap_or_else(|| sibling.completed_workout_id.clone()),
            );
            push_unique_workout_id(
                &mut equivalent_workout_ids,
                sibling.completed_workout_id.clone(),
            );
            if let Some(external_id) = sibling.external_id.clone() {
                push_unique_workout_id(&mut equivalent_workout_ids, external_id);
            }
        }
    }

    equivalent_workout_ids
}

fn same_completed_workout_family(left: &CompletedWorkout, right: &CompletedWorkout) -> bool {
    if left.user_id != right.user_id {
        return false;
    }

    if left.completed_workout_id == right.completed_workout_id {
        return true;
    }

    if same_non_empty_option(
        left.planned_workout_id.as_deref(),
        right.planned_workout_id.as_deref(),
    ) {
        return true;
    }

    same_non_empty_option(left.external_id.as_deref(), right.external_id.as_deref())
        || completed_workout_activity_id(&left.completed_workout_id)
            == completed_workout_activity_id(&right.completed_workout_id)
}

fn same_non_empty_option(left: Option<&str>, right: Option<&str>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => !left.is_empty() && left == right,
        _ => false,
    }
}

fn push_unique_workout_id(equivalent_workout_ids: &mut Vec<String>, workout_id: String) {
    if !equivalent_workout_ids.contains(&workout_id) {
        equivalent_workout_ids.push(workout_id);
    }
}

async fn resolve_completed_workout<Repo>(
    repository: &Repo,
    user_id: &str,
    workout_id: &str,
) -> Result<Option<crate::domain::completed_workouts::CompletedWorkout>, WorkoutSummaryError>
where
    Repo: CompletedWorkoutRepository + Clone + Send + Sync + 'static,
{
    match repository
        .find_by_user_id_and_source_activity_id(user_id, workout_id)
        .await
    {
        Ok(Some(workout)) => Ok(Some(workout)),
        Ok(None) => repository
            .find_by_user_id_and_completed_workout_id(
                user_id,
                &canonical_completed_workout_id(workout_id),
            )
            .await
            .map_err(|error| WorkoutSummaryError::Repository(error.to_string())),
        Err(error) => Err(WorkoutSummaryError::Repository(error.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::CompletedWorkoutTargetAdapter;
    use crate::domain::{
        completed_workouts::{
            BoxFuture as CompletedWorkoutBoxFuture, CompletedWorkout, CompletedWorkoutDetails,
            CompletedWorkoutError, CompletedWorkoutMetrics, CompletedWorkoutRepository,
        },
        workout_summary::{CompletedWorkoutAliasScope, CompletedWorkoutTargetUseCases},
    };

    #[derive(Clone)]
    struct StubCompletedWorkoutRepository {
        workout: CompletedWorkout,
    }

    impl CompletedWorkoutRepository for StubCompletedWorkoutRepository {
        fn find_by_user_id_and_completed_workout_id(
            &self,
            user_id: &str,
            completed_workout_id: &str,
        ) -> CompletedWorkoutBoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>>
        {
            let workout = self.workout.clone();
            let user_id = user_id.to_string();
            let completed_workout_id = completed_workout_id.to_string();
            Box::pin(async move {
                Ok((workout.user_id == user_id
                    && workout.completed_workout_id == completed_workout_id)
                    .then_some(workout))
            })
        }

        fn find_by_user_id_and_source_activity_id(
            &self,
            user_id: &str,
            source_activity_id: &str,
        ) -> CompletedWorkoutBoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>>
        {
            let workout = self.workout.clone();
            let user_id = user_id.to_string();
            let source_activity_id = source_activity_id.to_string();
            Box::pin(async move {
                Ok((workout.user_id == user_id
                    && workout.source_activity_id.as_deref() == Some(source_activity_id.as_str()))
                .then_some(workout))
            })
        }

        fn find_latest_by_user_id(
            &self,
            _user_id: &str,
        ) -> CompletedWorkoutBoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>>
        {
            Box::pin(async { Ok(None) })
        }

        fn list_by_user_id(
            &self,
            _user_id: &str,
        ) -> CompletedWorkoutBoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>>
        {
            Box::pin(async { Ok(Vec::new()) })
        }

        fn list_by_user_id_and_date_range(
            &self,
            user_id: &str,
            oldest: &str,
            newest: &str,
        ) -> CompletedWorkoutBoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>>
        {
            let workout = self.workout.clone();
            let user_id = user_id.to_string();
            let oldest = oldest.to_string();
            let newest = newest.to_string();
            Box::pin(async move {
                if workout.user_id != user_id {
                    return Ok(Vec::new());
                }

                let workout_date = workout.start_date_local.get(..10).unwrap_or("");
                if workout_date >= oldest.as_str() && workout_date <= newest.as_str() {
                    Ok(vec![workout])
                } else {
                    Ok(Vec::new())
                }
            })
        }

        fn upsert(
            &self,
            workout: CompletedWorkout,
        ) -> CompletedWorkoutBoxFuture<Result<CompletedWorkout, CompletedWorkoutError>> {
            Box::pin(async move { Ok(workout) })
        }
    }

    #[tokio::test]
    async fn resolve_completed_workout_target_includes_external_id_alias() {
        let repository = StubCompletedWorkoutRepository {
            workout: CompletedWorkout {
                completed_workout_id: "wahoo-workout:459893292".to_string(),
                user_id: "user-1".to_string(),
                start_date_local: "2026-05-27T13:10:35.000Z".to_string(),
                source_activity_id: Some("i151959404".to_string()),
                planned_workout_id: None,
                name: Some("Aerobic Endurance".to_string()),
                description: None,
                activity_type: Some("Ride".to_string()),
                external_id: Some("459893292".to_string()),
                trainer: false,
                duration_seconds: Some(5283),
                distance_meters: Some(44_718.45),
                metrics: CompletedWorkoutMetrics::default(),
                details: CompletedWorkoutDetails {
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
                details_unavailable_reason: None,
                power_curve_5s: None,
            },
        };
        let adapter = CompletedWorkoutTargetAdapter::new(repository);

        let resolved = adapter
            .resolve_completed_workout_target("user-1", "i151959404")
            .await
            .expect("resolver should succeed")
            .expect("target should resolve");

        assert_eq!(resolved.preferred_workout_id, "i151959404");
        assert_eq!(
            resolved.equivalent_workout_ids,
            vec![
                "i151959404".to_string(),
                "wahoo-workout:459893292".to_string(),
                "459893292".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn resolve_completed_workout_target_in_scope_uses_date_range_lookup() {
        let repository = StubCompletedWorkoutRepository {
            workout: CompletedWorkout {
                completed_workout_id: "wahoo-workout:459893292".to_string(),
                user_id: "user-1".to_string(),
                start_date_local: "2026-05-27T13:10:35.000Z".to_string(),
                source_activity_id: Some("i151959404".to_string()),
                planned_workout_id: None,
                name: Some("Aerobic Endurance".to_string()),
                description: None,
                activity_type: Some("Ride".to_string()),
                external_id: Some("459893292".to_string()),
                trainer: false,
                duration_seconds: Some(5283),
                distance_meters: Some(44_718.45),
                metrics: CompletedWorkoutMetrics::default(),
                details: CompletedWorkoutDetails {
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
                details_unavailable_reason: None,
                power_curve_5s: None,
            },
        };
        let adapter = CompletedWorkoutTargetAdapter::new(repository);
        let scope = CompletedWorkoutAliasScope {
            oldest: "2026-05-20".to_string(),
            newest: "2026-05-28".to_string(),
        };

        let resolved = adapter
            .resolve_completed_workout_target_in_scope("user-1", "i151959404", &scope)
            .await
            .expect("resolver should succeed")
            .expect("target should resolve");

        assert_eq!(resolved.preferred_workout_id, "i151959404");
    }
}
