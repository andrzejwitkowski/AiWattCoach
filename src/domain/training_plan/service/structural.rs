use std::collections::BTreeMap;
use std::collections::BTreeSet;

use crate::domain::{
    ai_workflow::{ValidationIssue, WorkflowPhase},
    calendar_view::CalendarEntryViewRefreshPort,
    identity::Clock,
    llm_tools::LlmToolLoopState,
    training_plan::{
        TrainingPlanDay, TrainingPlanError, TrainingPlanGenerationOperation,
        TrainingPlanGenerationOperationRepository, TrainingPlanGenerator,
        TrainingPlanProjectionRepository, TrainingPlanSnapshotRepository,
        TrainingPlanWorkoutSummaryPort,
    },
};

use super::ctx::{GenerationIdentity, GenerationPlanning};
use super::{
    correction, description_log_metadata, TrainingPlanGenerationService, MAX_CORRECTION_ATTEMPTS,
};

pub(super) struct CorrectionRoundInput<'a> {
    pub identity: GenerationIdentity<'a>,
    pub planning: GenerationPlanning<'a>,
    pub days_by_date: &'a mut BTreeMap<String, TrainingPlanDay>,
    pub issues: &'a mut Vec<ValidationIssue>,
    pub invalid_day_sections: &'a mut Vec<String>,
    pub operation: &'a mut TrainingPlanGenerationOperation,
    pub restored_tool_loop_state: Option<LlmToolLoopState>,
}

impl<Snapshots, Projections, Operations, Generator, WorkoutSummary, Time, Refresh>
    TrainingPlanGenerationService<
        Snapshots,
        Projections,
        Operations,
        Generator,
        WorkoutSummary,
        Time,
        Refresh,
    >
where
    Snapshots: TrainingPlanSnapshotRepository + Clone,
    Projections: TrainingPlanProjectionRepository + Clone,
    Operations: TrainingPlanGenerationOperationRepository + Clone,
    Generator: TrainingPlanGenerator + Clone,
    WorkoutSummary: TrainingPlanWorkoutSummaryPort + Clone,
    Time: Clock + Clone,
    Refresh: CalendarEntryViewRefreshPort + Clone,
{
    /// Resume from a stored correction checkpoint if present, then run remaining LLM correction rounds.
    /// On exhaustion with leftover issues, marks the operation failed and returns Unavailable.
    pub(super) async fn resolve_structural_corrections(
        &self,
        identity: GenerationIdentity<'_>,
        planning: GenerationPlanning<'_>,
        days_by_date: &mut BTreeMap<String, TrainingPlanDay>,
        issues: &mut Vec<ValidationIssue>,
        invalid_day_sections: &mut Vec<String>,
        operation: &mut TrainingPlanGenerationOperation,
    ) -> Result<(), TrainingPlanError> {
        if issues.is_empty() {
            return Ok(());
        }

        if let Err(error) = self
            .apply_stored_correction_checkpoint(
                days_by_date,
                issues,
                invalid_day_sections,
                operation,
            )
            .await
        {
            return Err(self
                .fail_operation(
                    operation,
                    WorkflowPhase::Correction,
                    error,
                    operation.validation_issues.clone(),
                )
                .await?);
        }

        let correction_attempts_recorded = operation
            .attempts
            .iter()
            .filter(|attempt| attempt.phase == WorkflowPhase::Correction)
            .count();
        let correction_attempts_remaining =
            MAX_CORRECTION_ATTEMPTS.saturating_sub(correction_attempts_recorded);
        let restored_tool_loop_state = operation
            .raw_correction_response
            .is_none()
            .then(|| operation.correction_tool_loop_state.clone())
            .flatten();

        if let Err(error) = self
            .apply_correction_rounds(
                CorrectionRoundInput {
                    identity,
                    planning,
                    days_by_date,
                    issues,
                    invalid_day_sections,
                    operation,
                    restored_tool_loop_state,
                },
                correction_attempts_remaining,
            )
            .await
        {
            return Err(self
                .fail_operation(
                    operation,
                    WorkflowPhase::Correction,
                    error,
                    operation.validation_issues.clone(),
                )
                .await?);
        }

        if !issues.is_empty() {
            let failed = operation.mark_failed(
                WorkflowPhase::Correction,
                "training plan generation failed validation".to_string(),
                issues.clone(),
                self.clock.now_epoch_seconds(),
            );
            self.operations.upsert(failed).await?;
            return Err(TrainingPlanError::Unavailable(
                "training plan generation failed validation".to_string(),
            ));
        }

        Ok(())
    }

    async fn apply_stored_correction_checkpoint(
        &self,
        days_by_date: &mut BTreeMap<String, TrainingPlanDay>,
        issues: &mut Vec<ValidationIssue>,
        invalid_day_sections: &mut Vec<String>,
        operation: &mut TrainingPlanGenerationOperation,
    ) -> Result<(), TrainingPlanError> {
        let Some(raw_correction_response) = operation.raw_correction_response.clone() else {
            return Ok(());
        };

        let invalid_dates = issues
            .iter()
            .map(|issue| issue.scope.clone())
            .collect::<BTreeSet<_>>();
        let corrected = self.parse_window(&raw_correction_response)?;
        let corrected_dates = corrected
            .days_by_date
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        self.merge_corrections(days_by_date, corrected.days_by_date, &invalid_dates);
        let corrected_invalid_dates = corrected
            .issues
            .iter()
            .map(|issue| issue.scope.clone())
            .collect::<BTreeSet<_>>();
        *issues = correction::merge_unresolved_issues(
            issues,
            &corrected.issues,
            &corrected_dates,
            &corrected_invalid_dates,
        );
        *invalid_day_sections = correction::merge_invalid_day_sections(
            invalid_day_sections,
            &corrected.invalid_day_sections,
            &corrected_dates,
            &corrected_invalid_dates,
        );
        if operation.validation_issues != *issues {
            *operation = self
                .operations
                .upsert(
                    operation
                        .with_validation_issues(issues.clone(), self.clock.now_epoch_seconds()),
                )
                .await?;
        }
        Ok(())
    }

    pub(super) async fn apply_correction_rounds(
        &self,
        input: CorrectionRoundInput<'_>,
        attempts_remaining: usize,
    ) -> Result<(), TrainingPlanError> {
        let CorrectionRoundInput {
            identity,
            planning,
            days_by_date,
            issues,
            invalid_day_sections,
            operation,
            mut restored_tool_loop_state,
        } = input;
        let GenerationIdentity {
            user_id,
            workout_id,
            saved_at_epoch_seconds,
            recap,
        } = identity;
        let GenerationPlanning {
            planning_context,
            planning_context_loaded,
        } = planning;

        let mut invalid_dates = issues
            .iter()
            .map(|issue| issue.scope.clone())
            .collect::<BTreeSet<_>>();

        for _ in 0..attempts_remaining {
            if issues.is_empty() {
                break;
            }

            self.ensure_planning_context_loaded(
                planning_context,
                planning_context_loaded,
                user_id,
                workout_id,
            )
            .await?;

            let correction_response = self
                .generator
                .correct_invalid_days_with_state(
                    user_id,
                    workout_id,
                    saved_at_epoch_seconds,
                    recap,
                    planning_context.as_ref(),
                    &invalid_day_sections.join("\n\n"),
                    issues.clone(),
                    restored_tool_loop_state.take(),
                    Some(self.correction_checkpoint(operation)),
                )
                .await?;
            let correction_tool_loop_state = correction_response.tool_loop_state;
            let correction_description = correction_response.description;
            let correction_response = correction_response.raw_response;
            let (has_description, description_chars, _) =
                description_log_metadata(correction_description.as_deref());
            *operation = self
                .operations
                .upsert(operation.with_correction_payload(
                    correction_response.clone(),
                    correction_description,
                    correction_tool_loop_state,
                    self.clock.now_epoch_seconds(),
                ))
                .await?;
            tracing::info!(
                operation_key = %operation.operation_key,
                phase = "correction",
                has_description,
                description_chars,
                plan_chars = correction_response.chars().count(),
                "stored training plan llm envelope"
            );

            let corrected = self.parse_window(&correction_response)?;
            let corrected_dates = corrected
                .days_by_date
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>();
            self.merge_corrections(days_by_date, corrected.days_by_date, &invalid_dates);
            let corrected_invalid_dates = corrected
                .issues
                .iter()
                .map(|issue| issue.scope.clone())
                .collect::<BTreeSet<_>>();
            *issues = correction::merge_unresolved_issues(
                issues,
                &corrected.issues,
                &corrected_dates,
                &corrected_invalid_dates,
            );
            *invalid_day_sections = correction::merge_invalid_day_sections(
                invalid_day_sections,
                &corrected.invalid_day_sections,
                &corrected_dates,
                &corrected_invalid_dates,
            );
            if operation.validation_issues != *issues {
                *operation = self
                    .operations
                    .upsert(
                        operation
                            .with_validation_issues(issues.clone(), self.clock.now_epoch_seconds()),
                    )
                    .await?;
            }
            invalid_dates = issues
                .iter()
                .map(|issue| issue.scope.clone())
                .collect::<BTreeSet<_>>();
        }

        Ok(())
    }
}
