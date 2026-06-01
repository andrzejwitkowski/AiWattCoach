use std::collections::{HashMap, HashSet};

use super::{
    CompletedWorkoutAliasScope, CompletedWorkoutTargetUseCases, ResolvedCompletedWorkoutTarget,
    WorkoutSummary, WorkoutSummaryError, WorkoutSummaryRepository,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WorkoutSummaryLookupRequest {
    pub requested_workout_id: String,
    pub lookup_workout_ids: Vec<String>,
}

pub(crate) fn identity_workout_summary_lookup(workout_id: String) -> WorkoutSummaryLookupRequest {
    WorkoutSummaryLookupRequest {
        requested_workout_id: workout_id.clone(),
        lookup_workout_ids: vec![workout_id],
    }
}

pub(crate) fn lookup_workout_ids_for_target(
    requested_workout_id: &str,
    resolved_target: &ResolvedCompletedWorkoutTarget,
) -> Vec<String> {
    let mut lookup_workout_ids = Vec::new();
    let mut seen_lookup_workout_ids = HashSet::new();
    push_unique_lookup_workout_id(
        &mut lookup_workout_ids,
        &mut seen_lookup_workout_ids,
        resolved_target.preferred_workout_id.clone(),
    );
    push_unique_lookup_workout_id(
        &mut lookup_workout_ids,
        &mut seen_lookup_workout_ids,
        requested_workout_id.to_string(),
    );
    for equivalent_workout_id in &resolved_target.equivalent_workout_ids {
        push_unique_lookup_workout_id(
            &mut lookup_workout_ids,
            &mut seen_lookup_workout_ids,
            equivalent_workout_id.clone(),
        );
    }
    lookup_workout_ids
}

pub(crate) fn collect_unique_lookup_workout_ids(
    requests: &[WorkoutSummaryLookupRequest],
) -> Vec<String> {
    let mut lookup_workout_ids = Vec::new();
    let mut seen_lookup_workout_ids = HashSet::new();
    for request in requests {
        for workout_id in &request.lookup_workout_ids {
            push_unique_lookup_workout_id(
                &mut lookup_workout_ids,
                &mut seen_lookup_workout_ids,
                workout_id.clone(),
            );
        }
    }
    lookup_workout_ids
}

pub(crate) fn map_lookup_requests_to_summaries(
    lookup_requests: impl IntoIterator<Item = WorkoutSummaryLookupRequest>,
    summaries_by_lookup_id: &HashMap<String, WorkoutSummary>,
) -> HashMap<String, WorkoutSummary> {
    lookup_requests
        .into_iter()
        .filter_map(|request| {
            request.lookup_workout_ids.iter().find_map(|lookup_id| {
                summaries_by_lookup_id
                    .get(lookup_id)
                    .cloned()
                    .map(|summary| (request.requested_workout_id.clone(), summary))
            })
        })
        .collect()
}

pub(crate) fn finalize_presented_summaries(
    mut summaries: Vec<WorkoutSummary>,
) -> Vec<WorkoutSummary> {
    let mut seen_summary_ids = HashSet::new();
    summaries.retain(|summary| seen_summary_ids.insert(summary.id.clone()));

    summaries.sort_by(|left, right| {
        right
            .updated_at_epoch_seconds
            .cmp(&left.updated_at_epoch_seconds)
            .then_with(|| {
                right
                    .created_at_epoch_seconds
                    .cmp(&left.created_at_epoch_seconds)
            })
    });
    summaries
}

pub async fn resolve_workout_summary_lookups_in_scope(
    target_service: Option<&dyn CompletedWorkoutTargetUseCases>,
    user_id: &str,
    requested_workout_ids: &[String],
    alias_scope: &CompletedWorkoutAliasScope,
) -> Result<Vec<WorkoutSummaryLookupRequest>, WorkoutSummaryError> {
    let Some(target_service) = target_service else {
        return Ok(requested_workout_ids
            .iter()
            .map(|workout_id| identity_workout_summary_lookup(workout_id.clone()))
            .collect());
    };

    let resolved_targets = target_service
        .resolve_completed_workout_targets_in_scope(user_id, requested_workout_ids, alias_scope)
        .await?;

    Ok(requested_workout_ids
        .iter()
        .filter_map(|workout_id| {
            let resolved_target = resolved_targets.get(workout_id)?;
            Some(WorkoutSummaryLookupRequest {
                requested_workout_id: workout_id.clone(),
                lookup_workout_ids: lookup_workout_ids_for_target(workout_id, resolved_target),
            })
        })
        .collect())
}

pub async fn load_summaries_by_workout_ids_in_scope(
    repository: &dyn WorkoutSummaryRepository,
    target_service: Option<&dyn CompletedWorkoutTargetUseCases>,
    user_id: &str,
    requested_workout_ids: &[String],
    alias_scope: &CompletedWorkoutAliasScope,
) -> Result<HashMap<String, WorkoutSummary>, WorkoutSummaryError> {
    let lookup_requests = resolve_workout_summary_lookups_in_scope(
        target_service,
        user_id,
        requested_workout_ids,
        alias_scope,
    )
    .await?;

    let lookup_workout_ids = collect_unique_lookup_workout_ids(&lookup_requests);
    if lookup_workout_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let summaries_by_lookup_id = repository
        .find_by_user_id_and_workout_ids(user_id, lookup_workout_ids)
        .await?
        .into_iter()
        .map(|summary| (summary.workout_id.clone(), summary))
        .collect::<HashMap<_, _>>();

    Ok(map_lookup_requests_to_summaries(
        lookup_requests,
        &summaries_by_lookup_id,
    ))
}

fn push_unique_lookup_workout_id(
    workout_ids: &mut Vec<String>,
    seen_workout_ids: &mut HashSet<String>,
    workout_id: String,
) {
    if seen_workout_ids.insert(workout_id.clone()) {
        workout_ids.push(workout_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_workout_ids_for_target_deduplicates_preferred_and_equivalents() {
        let lookup_ids = lookup_workout_ids_for_target(
            "ride-1",
            &ResolvedCompletedWorkoutTarget {
                preferred_workout_id: "wahoo-workout:999".to_string(),
                equivalent_workout_ids: vec!["wahoo-workout:999".to_string(), "ride-1".to_string()],
            },
        );

        assert_eq!(
            lookup_ids,
            vec!["wahoo-workout:999".to_string(), "ride-1".to_string()]
        );
    }

    #[test]
    fn finalize_presented_summaries_deduplicates_and_sorts_by_recency() {
        let summaries = finalize_presented_summaries(vec![
            WorkoutSummary {
                id: "summary-old".to_string(),
                user_id: "user-1".to_string(),
                workout_id: "ride-old".to_string(),
                rpe: None,
                messages: Vec::new(),
                provider_transcript: Vec::new(),
                saved_at_epoch_seconds: None,
                workout_recap_text: None,
                workout_recap_provider: None,
                workout_recap_model: None,
                workout_recap_generated_at_epoch_seconds: None,
                created_at_epoch_seconds: 1,
                updated_at_epoch_seconds: 10,
            },
            WorkoutSummary {
                id: "summary-new".to_string(),
                user_id: "user-1".to_string(),
                workout_id: "ride-new".to_string(),
                rpe: None,
                messages: Vec::new(),
                provider_transcript: Vec::new(),
                saved_at_epoch_seconds: None,
                workout_recap_text: None,
                workout_recap_provider: None,
                workout_recap_model: None,
                workout_recap_generated_at_epoch_seconds: None,
                created_at_epoch_seconds: 2,
                updated_at_epoch_seconds: 20,
            },
            WorkoutSummary {
                id: "summary-new".to_string(),
                user_id: "user-1".to_string(),
                workout_id: "ride-new-dup".to_string(),
                rpe: None,
                messages: Vec::new(),
                provider_transcript: Vec::new(),
                saved_at_epoch_seconds: None,
                workout_recap_text: None,
                workout_recap_provider: None,
                workout_recap_model: None,
                workout_recap_generated_at_epoch_seconds: None,
                created_at_epoch_seconds: 3,
                updated_at_epoch_seconds: 30,
            },
        ]);

        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].workout_id, "ride-new");
        assert_eq!(summaries[1].workout_id, "ride-old");
    }
}
