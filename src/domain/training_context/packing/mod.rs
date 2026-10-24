use super::model::{RenderedTrainingContext, TrainingContext};

mod header_table;
mod payloads;

#[cfg(test)]
mod tests;

use payloads::{StablePayload, VolatilePayload};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PackMode {
    #[default]
    Full,
    Lean,
}

impl PackMode {
    pub fn is_lean(self) -> bool {
        matches!(self, Self::Lean)
    }
}

// ponytail: fixed lean caps; raise only if planning quality drops with long gaps.
pub(crate) const LEAN_HISTORY_WORKOUTS: usize = 7;
pub(crate) const LEAN_LOAD_TREND_DAYS: usize = 7;
pub(crate) const LEAN_RECENT_DAYS: usize = 4;

pub(crate) fn last_n<T>(items: &[T], n: usize) -> &[T] {
    &items[items.len().saturating_sub(n)..]
}

pub fn render_training_context(context: &TrainingContext) -> RenderedTrainingContext {
    render_training_context_with_mode(context, PackMode::Full)
}

pub fn render_training_context_with_mode(
    context: &TrainingContext,
    mode: PackMode,
) -> RenderedTrainingContext {
    let stable_payload = StablePayload::from_context(context, mode);
    let volatile_payload = VolatilePayload::from_context(context, mode);
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
