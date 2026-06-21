use std::collections::HashSet;

use super::model::{RenderedTrainingContext, TrainingContext};

mod payloads;

#[cfg(test)]
mod tests;

use payloads::{StablePayload, VolatilePayload};

pub fn render_training_context(context: &TrainingContext) -> RenderedTrainingContext {
    let stable_payload = StablePayload::from_context(context);
    let volatile_payload = VolatilePayload::from_context(context);
    let stable_context =
        serde_json::to_string(&stable_payload).expect("stable training context should serialize");
    let volatile_context = serde_json::to_string(&volatile_payload)
        .expect("volatile training context should serialize");
    let approximate_tokens =
        approximate_token_count(&stable_context) + approximate_token_count(&volatile_context);

    RenderedTrainingContext {
        stable_context,
        volatile_context,
        approximate_tokens,
    }
}

pub fn approximate_token_count(value: &str) -> usize {
    value.chars().count().div_ceil(3)
}

pub(crate) fn recent_workout_activity_ids(context: &TrainingContext) -> HashSet<&str> {
    context
        .recent_days
        .iter()
        .flat_map(|day| day.workouts.iter())
        .map(|workout| workout.activity_id.as_str())
        .collect()
}

fn is_empty_slice<T>(value: &[T]) -> bool {
    value.is_empty()
}
