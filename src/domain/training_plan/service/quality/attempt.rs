use super::super::super::{
    format_quality_feedback, plan_quality_attempt_message, PlanQualityEvaluationInput,
    TrainingPlanError,
};
use super::super::ctx::GenerationPlanning;
use super::super::quality_evidence::extract_plan_quality_evidence;
use super::super::TrainingPlanGenerationService;
use super::{BestDraft, QualityAttemptAction, QualityAttemptLoopCtx};
use crate::domain::{
    calendar_view::CalendarEntryViewRefreshPort,
    identity::Clock,
    training_plan::{
        TargetEventRequirement, TrainingPlanGenerationOperationRepository, TrainingPlanGenerator,
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
    pub(super) async fn run_quality_evaluation_attempts(
        &self,
        mut ctx: QualityAttemptLoopCtx<'_>,
    ) -> Result<bool, TrainingPlanError> {
        let Some(availability_summary) = self
            .load_plan_quality_availability(ctx.identity, ctx.operation)
            .await?
        else {
            return Ok(false);
        };
        let target_event = self
            .load_plan_target_event_requirement(ctx.identity, ctx.operation)
            .await?;

        let mut accepted = false;
        let start_attempt = ctx.limits.start_attempt;
        let max_loops = ctx.limits.max_loops;

        for attempt in start_attempt..=max_loops {
            match self
                .run_one_quality_evaluation_attempt(&mut ctx, attempt, &availability_summary)
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
                            ctx.identity,
                            GenerationPlanning {
                                planning_context: &mut *ctx.planning.planning_context,
                                planning_context_loaded: &mut *ctx.planning.planning_context_loaded,
                            },
                            ctx.operation,
                            &feedback,
                            &raise_to_next,
                            target_event.as_ref(),
                        )
                        .await
                    {
                        Ok(replan) => {
                            *ctx.snapshot = replan.snapshot;
                            *ctx.draft = replan.draft;
                        }
                        Err(error) => {
                            tracing::warn!(
                                operation_key = %ctx.operation.operation_key,
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
        identity: &super::super::ctx::GenerationIdentity<'_>,
        operation: &crate::domain::training_plan::TrainingPlanGenerationOperation,
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

    async fn load_plan_target_event_requirement(
        &self,
        identity: &super::super::ctx::GenerationIdentity<'_>,
        operation: &crate::domain::training_plan::TrainingPlanGenerationOperation,
    ) -> Result<Option<TargetEventRequirement>, TrainingPlanError> {
        match self
            .generator
            .plan_target_event_requirement(identity.user_id, identity.workout_id)
            .await
        {
            Ok(target) => Ok(target),
            Err(error) => {
                tracing::warn!(
                    operation_key = %operation.operation_key,
                    error = %error,
                    "plan target event requirement lookup failed; skipping discipline gate"
                );
                Ok(None)
            }
        }
    }

    async fn run_one_quality_evaluation_attempt(
        &self,
        ctx: &mut QualityAttemptLoopCtx<'_>,
        attempt: u32,
        availability_summary: &str,
    ) -> Result<QualityAttemptAction, TrainingPlanError> {
        let evidence = extract_plan_quality_evidence(ctx.operation);
        let mut evaluation = match self
            .generator
            .evaluate_plan_quality(PlanQualityEvaluationInput {
                user_id: ctx.identity.user_id,
                workout_id: ctx.identity.workout_id,
                saved_at_epoch_seconds: ctx.identity.saved_at_epoch_seconds,
                workout_recap: ctx.identity.recap,
                planning_context: ctx.planning.planning_context.as_ref(),
                draft_plan_text: &ctx.draft.plan_text,
                draft_plan_description: ctx.draft.description.as_deref(),
                evidence: evidence.as_ref(),
                availability_summary: Some(availability_summary),
            })
            .await
        {
            Ok(evaluation) => evaluation,
            Err(error) => {
                tracing::warn!(
                    operation_key = %ctx.operation.operation_key,
                    attempt,
                    error = %error,
                    "plan quality evaluator failed; shipping best available draft"
                );
                return Ok(QualityAttemptAction::Stop);
            }
        };
        evaluation.attempt = attempt;

        tracing::info!(
            operation_key = %ctx.operation.operation_key,
            attempt,
            score = evaluation.score,
            raise_to_next = %evaluation.raise_to_next,
            "plan quality evaluation attempt"
        );

        super::emit_progress(
            ctx.progress.messages,
            ctx.progress.port,
            ctx.identity.user_id,
            ctx.identity.workout_id,
            plan_quality_attempt_message(
                attempt,
                ctx.limits.max_loops,
                evaluation.score,
                &evaluation.critique,
                &evaluation.raise_to_next,
            ),
        );

        let is_better = ctx
            .best
            .as_ref()
            .is_none_or(|previous| evaluation.score >= previous.evaluation.score);
        if is_better {
            *ctx.best = Some(BestDraft {
                snapshot: ctx.snapshot.clone(),
                evaluation: evaluation.clone(),
            });
        }

        *ctx.operation = self
            .operations
            .upsert(ctx.operation.with_quality_evaluation(
                evaluation.clone(),
                is_better.then(|| (ctx.draft.plan_text.clone(), ctx.draft.description.clone())),
                self.clock.now_epoch_seconds(),
            ))
            .await?;

        if evaluation.score >= ctx.limits.pass_score {
            return Ok(QualityAttemptAction::Accepted);
        }
        if attempt == ctx.limits.max_loops {
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
