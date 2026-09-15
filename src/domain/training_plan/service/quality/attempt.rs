use std::sync::Arc;

use super::super::super::{
    format_quality_feedback, plan_quality_attempt_message, PlanQualityEvaluationInput,
    PlanQualityProgressPort, TrainingPlanError, TrainingPlanGenerationOperation,
    TrainingPlanSnapshot,
};
use super::super::ctx::{GenerationIdentity, GenerationPlanning};
use super::super::quality_evidence::extract_plan_quality_evidence;
use super::super::TrainingPlanGenerationService;
use super::{BestDraft, QualityAttemptAction};
use crate::domain::{
    calendar_view::CalendarEntryViewRefreshPort,
    identity::Clock,
    training_plan::{
        TrainingPlanGenerationOperationRepository, TrainingPlanGenerator,
        TrainingPlanProjectionRepository, TrainingPlanSnapshotRepository,
        TrainingPlanWorkoutSummaryPort,
    },
};

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
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn run_quality_evaluation_attempts(
        &self,
        identity: &GenerationIdentity<'_>,
        planning: GenerationPlanning<'_>,
        snapshot: &mut TrainingPlanSnapshot,
        draft_plan_text: &mut String,
        operation: &mut TrainingPlanGenerationOperation,
        best: &mut Option<BestDraft>,
        quality_progress_messages: &mut Vec<String>,
        plan_quality_progress: Option<&Arc<dyn PlanQualityProgressPort>>,
        start_attempt: u32,
        max_loops: u32,
        pass_score: u8,
    ) -> Result<bool, TrainingPlanError> {
        let Some(availability_summary) = self
            .load_plan_quality_availability(identity, operation)
            .await?
        else {
            return Ok(false);
        };

        let mut accepted = false;
        for attempt in start_attempt..=max_loops {
            match self
                .run_one_quality_evaluation_attempt(
                    identity,
                    planning.planning_context.as_ref(),
                    snapshot,
                    draft_plan_text,
                    operation,
                    best,
                    quality_progress_messages,
                    plan_quality_progress,
                    attempt,
                    max_loops,
                    pass_score,
                    &availability_summary,
                )
                .await?
            {
                QualityAttemptAction::Accepted => {
                    accepted = true;
                    break;
                }
                QualityAttemptAction::Stop => break,
                QualityAttemptAction::Replan {
                    feedback,
                    raise_to_next,
                } => {
                    match self
                        .regenerate_structurally_valid_snapshot(
                            identity,
                            GenerationPlanning {
                                planning_context: &mut *planning.planning_context,
                                planning_context_loaded: &mut *planning.planning_context_loaded,
                            },
                            operation,
                            &feedback,
                            &raise_to_next,
                        )
                        .await
                    {
                        Ok((next_snapshot, next_draft)) => {
                            *snapshot = next_snapshot;
                            *draft_plan_text = next_draft;
                        }
                        Err(error) => {
                            tracing::warn!(
                                operation_key = %operation.operation_key,
                                attempt,
                                error = %error,
                                "plan quality replan failed validation; shipping best draft so far"
                            );
                            break;
                        }
                    }
                }
            }
        }
        Ok(accepted)
    }

    async fn load_plan_quality_availability(
        &self,
        identity: &GenerationIdentity<'_>,
        operation: &TrainingPlanGenerationOperation,
    ) -> Result<Option<String>, TrainingPlanError> {
        match self
            .generator
            .plan_quality_availability_summary(identity.user_id, identity.workout_id)
            .await
        {
            Ok(summary) => Ok(Some(summary)),
            Err(error) => {
                tracing::warn!(
                    operation_key = %operation.operation_key,
                    error = %error,
                    "plan quality availability summary failed; shipping best available draft"
                );
                Ok(None)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_one_quality_evaluation_attempt(
        &self,
        identity: &GenerationIdentity<'_>,
        planning_context: Option<&crate::domain::training_plan::TrainingPlanPlanningContext>,
        snapshot: &TrainingPlanSnapshot,
        draft_plan_text: &str,
        operation: &mut TrainingPlanGenerationOperation,
        best: &mut Option<BestDraft>,
        quality_progress_messages: &mut Vec<String>,
        plan_quality_progress: Option<&Arc<dyn PlanQualityProgressPort>>,
        attempt: u32,
        max_loops: u32,
        pass_score: u8,
        availability_summary: &str,
    ) -> Result<QualityAttemptAction, TrainingPlanError> {
        let evidence = extract_plan_quality_evidence(operation);
        let mut evaluation = match self
            .generator
            .evaluate_plan_quality(PlanQualityEvaluationInput {
                user_id: identity.user_id,
                workout_id: identity.workout_id,
                saved_at_epoch_seconds: identity.saved_at_epoch_seconds,
                workout_recap: identity.recap,
                planning_context,
                draft_plan_text,
                evidence: evidence.as_ref(),
                availability_summary: Some(availability_summary),
            })
            .await
        {
            Ok(evaluation) => evaluation,
            Err(error) => {
                tracing::warn!(
                    operation_key = %operation.operation_key,
                    attempt,
                    error = %error,
                    "plan quality evaluator failed; shipping best available draft"
                );
                return Ok(QualityAttemptAction::Stop);
            }
        };
        evaluation.attempt = attempt;

        tracing::info!(
            operation_key = %operation.operation_key,
            attempt,
            score = evaluation.score,
            raise_to_next = %evaluation.raise_to_next,
            "plan quality evaluation attempt"
        );

        super::emit_progress(
            quality_progress_messages,
            plan_quality_progress,
            identity.user_id,
            identity.workout_id,
            plan_quality_attempt_message(
                attempt,
                max_loops,
                evaluation.score,
                &evaluation.critique,
                &evaluation.raise_to_next,
            ),
        );

        let is_better = best
            .as_ref()
            .is_none_or(|previous| evaluation.score >= previous.evaluation.score);
        if is_better {
            *best = Some(BestDraft {
                snapshot: snapshot.clone(),
                evaluation: evaluation.clone(),
            });
        }

        *operation = self
            .operations
            .upsert(operation.with_quality_evaluation(
                evaluation.clone(),
                is_better.then(|| draft_plan_text.to_string()),
                self.clock.now_epoch_seconds(),
            ))
            .await?;

        if evaluation.score >= pass_score {
            return Ok(QualityAttemptAction::Accepted);
        }
        if attempt == max_loops {
            return Ok(QualityAttemptAction::Stop);
        }

        Ok(QualityAttemptAction::Replan {
            feedback: format_quality_feedback(
                evaluation.score,
                &evaluation.critique,
                &evaluation.raise_to_next,
            ),
            raise_to_next: evaluation.raise_to_next.clone(),
        })
    }
}
