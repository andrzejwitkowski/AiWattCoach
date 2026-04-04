use crate::domain::athlete_summary::AthleteSummaryState;

use super::dto::AthleteSummaryDto;

pub(super) fn map_summary_state_to_dto(state: AthleteSummaryState) -> AthleteSummaryDto {
    AthleteSummaryDto {
        exists: state.summary.is_some(),
        stale: state.stale,
        summary_text: state
            .summary
            .as_ref()
            .map(|summary| summary.summary_text.clone()),
        generated_at_epoch_seconds: state
            .summary
            .as_ref()
            .map(|summary| summary.generated_at_epoch_seconds),
        updated_at_epoch_seconds: state
            .summary
            .as_ref()
            .map(|summary| summary.updated_at_epoch_seconds),
    }
}
