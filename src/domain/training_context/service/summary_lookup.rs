use std::collections::HashMap;

use chrono::NaiveDate;

use crate::domain::{
    intervals::Event,
    training_context::model::RecentWorkoutRecapContext,
    workout_summary::{
        alias_batch_lookup::load_summaries_by_workout_ids_in_scope, CompletedWorkoutAliasScope,
        CompletedWorkoutTargetUseCases, WorkoutSummary, WorkoutSummaryError,
        WorkoutSummaryRepository,
    },
};

pub(super) struct WorkoutSummaryDateScope<'a> {
    pub recent_start: NaiveDate,
    pub focus_date: NaiveDate,
    pub activity_dates_by_id: &'a HashMap<String, NaiveDate>,
}

pub(super) struct LoadedWorkoutSummaryFields {
    pub rpe_by_workout_id: HashMap<String, u8>,
    pub recap_by_workout_id: HashMap<String, String>,
    pub recent_workout_recaps: Vec<RecentWorkoutRecapContext>,
}

pub(super) async fn load_workout_summary_fields(
    repository: &dyn WorkoutSummaryRepository,
    target_service: &dyn CompletedWorkoutTargetUseCases,
    user_id: &str,
    activity_ids: &[String],
    events: &[Event],
    date_scope: &WorkoutSummaryDateScope<'_>,
) -> Result<LoadedWorkoutSummaryFields, WorkoutSummaryError> {
    let mut requested_workout_ids = activity_ids.to_vec();
    for event in events {
        push_unique_workout_id(&mut requested_workout_ids, event.id.to_string());
    }

    if requested_workout_ids.is_empty() {
        return Ok(empty_summary_fields());
    }

    let alias_scope = CompletedWorkoutAliasScope {
        oldest: date_scope.recent_start.format("%Y-%m-%d").to_string(),
        newest: date_scope.focus_date.format("%Y-%m-%d").to_string(),
    };

    let summaries_by_requested_id = load_summaries_by_workout_ids_in_scope(
        repository,
        Some(target_service),
        user_id,
        &requested_workout_ids,
        &alias_scope,
    )
    .await?;

    Ok(map_loaded_workout_summary_fields(
        &summaries_by_requested_id,
        date_scope,
    ))
}

fn empty_summary_fields() -> LoadedWorkoutSummaryFields {
    LoadedWorkoutSummaryFields {
        rpe_by_workout_id: HashMap::new(),
        recap_by_workout_id: HashMap::new(),
        recent_workout_recaps: Vec::new(),
    }
}

fn map_loaded_workout_summary_fields(
    summaries_by_requested_id: &HashMap<String, WorkoutSummary>,
    date_scope: &WorkoutSummaryDateScope<'_>,
) -> LoadedWorkoutSummaryFields {
    let mut rpe_by_workout_id = HashMap::new();
    let mut recap_by_workout_id = HashMap::new();
    for (workout_id, summary) in summaries_by_requested_id {
        if let Some(rpe) = summary.rpe {
            rpe_by_workout_id.insert(workout_id.clone(), rpe);
        }
        if let Some(recap) = summary
            .workout_recap_text
            .as_deref()
            .filter(|text| !text.is_empty())
        {
            recap_by_workout_id.insert(workout_id.clone(), recap.to_string());
        }
    }

    let recent_workout_recaps = build_recent_workout_recaps(
        date_scope.recent_start,
        date_scope.focus_date,
        summaries_by_requested_id,
        date_scope.activity_dates_by_id,
    );

    LoadedWorkoutSummaryFields {
        rpe_by_workout_id,
        recap_by_workout_id,
        recent_workout_recaps,
    }
}

pub(super) fn build_recent_workout_recaps(
    recent_start: NaiveDate,
    focus_date: NaiveDate,
    summaries_by_requested_id: &HashMap<String, WorkoutSummary>,
    activity_dates_by_id: &HashMap<String, NaiveDate>,
) -> Vec<RecentWorkoutRecapContext> {
    use std::collections::HashSet;

    let mut seen_summary_ids = HashSet::new();
    let mut entries = summaries_by_requested_id.iter().collect::<Vec<_>>();
    entries.sort_by_key(|(workout_id, _)| *workout_id);

    let mut recaps = entries
        .into_iter()
        .filter_map(|(workout_id, summary)| {
            if !seen_summary_ids.insert(summary.id.clone()) {
                return None;
            }
            let recap = summary
                .workout_recap_text
                .as_deref()
                .filter(|text| !text.trim().is_empty())?;
            let workout_date = activity_dates_by_id.get(workout_id)?;
            if *workout_date < recent_start || *workout_date > focus_date {
                return None;
            }
            Some(RecentWorkoutRecapContext {
                date: workout_date.format("%Y-%m-%d").to_string(),
                workout_id: workout_id.clone(),
                rpe: summary.rpe,
                recap: recap.to_string(),
            })
        })
        .collect::<Vec<_>>();

    recaps.sort_by(|left, right| {
        right
            .date
            .cmp(&left.date)
            .then_with(|| right.workout_id.cmp(&left.workout_id))
    });
    recaps
}

fn push_unique_workout_id(workout_ids: &mut Vec<String>, workout_id: String) {
    if !workout_ids.iter().any(|existing| existing == &workout_id) {
        workout_ids.push(workout_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::workout_summary::WorkoutSummary;

    fn sample_summary(id: &str, workout_id: &str, recap: &str, rpe: u8) -> WorkoutSummary {
        WorkoutSummary {
            id: id.to_string(),
            user_id: "user-1".to_string(),
            workout_id: workout_id.to_string(),
            rpe: Some(rpe),
            messages: Vec::new(),
            provider_transcript: Vec::new(),
            saved_at_epoch_seconds: Some(1),
            workout_recap_text: Some(recap.to_string()),
            workout_recap_provider: Some("openrouter".to_string()),
            workout_recap_model: Some("test".to_string()),
            workout_recap_generated_at_epoch_seconds: Some(1),
            created_at_epoch_seconds: 1,
            updated_at_epoch_seconds: 1,
        }
    }

    #[test]
    fn build_recent_workout_recaps_filters_by_date_and_deduplicates() {
        let recent_start = NaiveDate::from_ymd_opt(2026, 5, 26).expect("valid date");
        let focus_date = NaiveDate::from_ymd_opt(2026, 6, 1).expect("valid date");
        let mut summaries = HashMap::new();
        summaries.insert(
            "ride-1".to_string(),
            sample_summary("summary-1", "ride-1", "Race recap", 8),
        );
        summaries.insert(
            "ride-1-alias".to_string(),
            sample_summary("summary-1", "ride-1-alias", "Race recap", 8),
        );
        summaries.insert(
            "ride-old".to_string(),
            sample_summary("summary-old", "ride-old", "Old recap", 5),
        );

        let mut activity_dates = HashMap::new();
        activity_dates.insert(
            "ride-1".to_string(),
            NaiveDate::from_ymd_opt(2026, 5, 31).expect("valid date"),
        );
        activity_dates.insert(
            "ride-1-alias".to_string(),
            NaiveDate::from_ymd_opt(2026, 5, 31).expect("valid date"),
        );
        activity_dates.insert(
            "ride-old".to_string(),
            NaiveDate::from_ymd_opt(2026, 5, 20).expect("valid date"),
        );

        let recaps =
            build_recent_workout_recaps(recent_start, focus_date, &summaries, &activity_dates);

        assert_eq!(recaps.len(), 1);
        assert_eq!(recaps[0].date, "2026-05-31");
        assert_eq!(recaps[0].workout_id, "ride-1");
        assert_eq!(recaps[0].recap, "Race recap");
        assert_eq!(recaps[0].rpe, Some(8));
    }
}
