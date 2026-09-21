mod attempt;

use std::sync::Arc;

use super::super::{
    draft_addresses_quality_checklist, plan_quality_finished_accepted_message,
    plan_quality_finished_best_message, PlanQualityEvaluation, PlanQualityEvaluatorLlmConfigPort,
    PlanQualityProgressPort, TrainingPlanError, TrainingPlanGenerationOperation,
    TrainingPlanSnapshot,
};
use super::ctx::{GenerationIdentity, GenerationPlanning};
use super::structural::CorrectionRoundInput;
use super::{TrainingPlanGenerationService, MAX_CORRECTION_ATTEMPTS};
use crate::domain::{
    calendar_view::CalendarEntryViewRefreshPort,
    identity::Clock,
    training_plan::{
        TrainingPlanGenerationOperationRepository, TrainingPlanGenerator,
        TrainingPlanProjectionRepository, TrainingPlanSnapshotRepository,
        TrainingPlanWorkoutSummaryPort,
    },
};

pub(super) use super::ctx::{
    GenerationIdentity as QualityIdentity, GenerationPlanning as QualityPlanning,
};

/// Plan text and matching envelope commentary evaluated / updated together.
pub(super) struct QualityDraft {
    pub plan_text: String,
    pub description: Option<String>,
}

pub(super) struct PlanQualityLoopInput<'a> {
    pub identity: GenerationIdentity<'a>,
    pub planning: GenerationPlanning<'a>,
    pub snapshot: TrainingPlanSnapshot,
    pub draft: QualityDraft,
    pub operation: TrainingPlanGenerationOperation,
    pub plan_quality_config: &'a Arc<dyn PlanQualityEvaluatorLlmConfigPort>,
    pub plan_quality_progress: Option<&'a Arc<dyn PlanQualityProgressPort>>,
}

pub(super) struct PlanQualityLoopResult {
    pub snapshot: TrainingPlanSnapshot,
    pub operation: TrainingPlanGenerationOperation,
    pub quality_evaluations: Vec<PlanQualityEvaluation>,
    pub shipped_quality: Option<PlanQualityEvaluation>,
    pub quality_progress_messages: Vec<String>,
}

pub(super) struct BestDraft {
    snapshot: TrainingPlanSnapshot,
    evaluation: PlanQualityEvaluation,
}

struct QualityReplanDraft {
    snapshot: TrainingPlanSnapshot,
    draft: QualityDraft,
}

pub(super) struct QualityLoopLimits {
    pub start_attempt: u32,
    pub max_loops: u32,
    pub pass_score: u8,
}

pub(super) struct QualityProgressSink<'a> {
    pub messages: &'a mut Vec<String>,
    pub port: Option<&'a Arc<dyn PlanQualityProgressPort>>,
}

pub(super) struct QualityAttemptLoopCtx<'a> {
    pub identity: &'a GenerationIdentity<'a>,
    pub planning: GenerationPlanning<'a>,
    pub snapshot: &'a mut TrainingPlanSnapshot,
    pub draft: &'a mut QualityDraft,
    pub operation: &'a mut TrainingPlanGenerationOperation,
    pub best: &'a mut Option<BestDraft>,
    pub progress: QualityProgressSink<'a>,
    pub limits: QualityLoopLimits,
}

struct QualityFinalizeInput<'a> {
    identity: &'a GenerationIdentity<'a>,
    snapshot: TrainingPlanSnapshot,
    operation: TrainingPlanGenerationOperation,
    best: Option<BestDraft>,
    accepted: bool,
    max_loops: u32,
    progress: QualityProgressSink<'a>,
}

pub(super) enum QualityAttemptAction {
    Accepted,
    Stop,
    Replan {
        feedback: String,
        raise_to_next: String,
    },
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
    pub(super) async fn run_plan_quality_loop(
        &self,
        input: PlanQualityLoopInput<'_>,
    ) -> Result<PlanQualityLoopResult, TrainingPlanError> {
        let PlanQualityLoopInput {
            identity,
            planning,
            mut snapshot,
            mut draft,
            mut operation,
            plan_quality_config,
            plan_quality_progress,
        } = input;

        let max_loops = plan_quality_config
            .get_plan_quality_max_loops(identity.user_id)
            .await?
            .max(1);
        let pass_score = plan_quality_config
            .get_plan_quality_pass_score(identity.user_id)
            .await?;
        let mut quality_progress_messages = Vec::new();
        let mut best =
            self.seed_best_quality_draft(&identity, &operation, &snapshot, &draft.plan_text)?;
        let start_attempt = (operation.quality_evaluations.len() as u32).saturating_add(1);
        let mut accepted = best
            .as_ref()
            .is_some_and(|best_draft| best_draft.evaluation.score >= pass_score);

        if !accepted {
            accepted = self
                .run_quality_evaluation_attempts(QualityAttemptLoopCtx {
                    identity: &identity,
                    planning,
                    snapshot: &mut snapshot,
                    draft: &mut draft,
                    operation: &mut operation,
                    best: &mut best,
                    progress: QualityProgressSink {
                        messages: &mut quality_progress_messages,
                        port: plan_quality_progress,
                    },
                    limits: QualityLoopLimits {
                        start_attempt,
                        max_loops,
                        pass_score,
                    },
                })
                .await?;
        }

        self.finalize_plan_quality_loop(QualityFinalizeInput {
            identity: &identity,
            snapshot,
            operation,
            best,
            accepted,
            max_loops,
            progress: QualityProgressSink {
                messages: &mut quality_progress_messages,
                port: plan_quality_progress,
            },
        })
    }

    fn finalize_plan_quality_loop(
        &self,
        input: QualityFinalizeInput<'_>,
    ) -> Result<PlanQualityLoopResult, TrainingPlanError> {
        let QualityFinalizeInput {
            identity,
            snapshot,
            mut operation,
            best,
            accepted,
            max_loops,
            progress,
        } = input;

        let Some(best) = best else {
            // Evaluator never produced a score (outage / empty resume range). Ship the
            // structurally valid draft without quality metadata rather than failing save.
            return Ok(PlanQualityLoopResult {
                snapshot,
                quality_evaluations: operation.quality_evaluations.clone(),
                shipped_quality: None,
                quality_progress_messages: std::mem::take(progress.messages),
                operation,
            });
        };
        let finished = if accepted {
            plan_quality_finished_accepted_message(best.evaluation.score)
        } else {
            plan_quality_finished_best_message(best.evaluation.score, max_loops)
        };
        emit_progress(
            progress.messages,
            progress.port,
            identity.user_id,
            identity.workout_id,
            finished,
        );

        if let Some(best_plan) = operation.best_quality_plan_response.clone() {
            let best_description = operation.best_quality_plan_description.clone();
            operation = operation.with_shipped_best_raw(
                best_plan,
                best_description,
                self.clock.now_epoch_seconds(),
            );
        }

        Ok(PlanQualityLoopResult {
            snapshot: best.snapshot,
            quality_evaluations: operation.quality_evaluations.clone(),
            shipped_quality: Some(best.evaluation),
            quality_progress_messages: std::mem::take(progress.messages),
            operation,
        })
    }

    fn seed_best_quality_draft(
        &self,
        identity: &GenerationIdentity<'_>,
        operation: &TrainingPlanGenerationOperation,
        current_snapshot: &TrainingPlanSnapshot,
        current_draft: &str,
    ) -> Result<Option<BestDraft>, TrainingPlanError> {
        let (Some(evaluation), Some(best_draft)) = (
            operation.best_quality_evaluation.clone(),
            operation.best_quality_plan_response.clone(),
        ) else {
            // Incomplete stored state is not a verified BestDraft for the current draft.
            return Ok(None);
        };
        if best_draft == current_draft {
            return Ok(Some(BestDraft {
                snapshot: current_snapshot.clone(),
                evaluation,
            }));
        }
        let parsed = self.parse_window(&best_draft)?;
        let days = self.validate_snapshot_days(&parsed.days_by_date)?;
        let snapshot = self.build_snapshot(
            identity.user_id,
            identity.workout_id,
            &operation.operation_key,
            operation.saved_at_epoch_seconds,
            days,
        )?;
        Ok(Some(BestDraft {
            snapshot,
            evaluation,
        }))
    }

    async fn regenerate_structurally_valid_snapshot(
        &self,
        identity: &GenerationIdentity<'_>,
        planning: GenerationPlanning<'_>,
        operation: &mut TrainingPlanGenerationOperation,
        quality_feedback: &str,
        raise_to_next: &str,
    ) -> Result<QualityReplanDraft, TrainingPlanError> {
        let GenerationPlanning {
            planning_context,
            planning_context_loaded,
        } = planning;
        let replan = self
            .generate_quality_replan_draft(
                identity,
                planning_context,
                planning_context_loaded,
                operation,
                quality_feedback,
            )
            .await?;
        if draft_addresses_quality_checklist(replan.draft.description.as_deref(), raise_to_next) {
            return Ok(replan);
        }

        tracing::warn!(
            operation_key = %operation.operation_key,
            raise_to_next,
            "plan quality replan omitted Adjustment rules in description; retrying once with sharper feedback"
        );
        let sharper = format!(
            "CHECKLIST NOT MET: draft description omitted required \"Adjustment rules\" section.\n{quality_feedback}"
        );
        let retry = self
            .generate_quality_replan_draft(
                identity,
                planning_context,
                planning_context_loaded,
                operation,
                &sharper,
            )
            .await?;
        if draft_addresses_quality_checklist(retry.draft.description.as_deref(), raise_to_next) {
            Ok(retry)
        } else {
            Err(TrainingPlanError::Validation(
                "quality replan description omitted required Adjustment rules".to_string(),
            ))
        }
    }

    async fn generate_quality_replan_draft(
        &self,
        identity: &GenerationIdentity<'_>,
        planning_context: &mut Option<crate::domain::training_plan::TrainingPlanPlanningContext>,
        planning_context_loaded: &mut bool,
        operation: &mut TrainingPlanGenerationOperation,
        quality_feedback: &str,
    ) -> Result<QualityReplanDraft, TrainingPlanError> {
        self.ensure_planning_context_loaded(
            planning_context,
            planning_context_loaded,
            identity.user_id,
            identity.workout_id,
        )
        .await?;

        let raw_plan_output = self
            .generator
            .generate_initial_plan_window_with_state(
                identity.user_id,
                identity.workout_id,
                identity.saved_at_epoch_seconds,
                identity.recap,
                planning_context.as_ref(),
                None,
                Some(self.initial_plan_checkpoint(operation)),
                Some(quality_feedback),
            )
            .await?;
        let raw_plan_tool_loop_state = raw_plan_output.tool_loop_state;
        let raw_plan_description = raw_plan_output.description;
        let raw_plan_response = raw_plan_output.raw_response;
        *operation = self
            .operations
            .upsert(operation.with_raw_plan_payload(
                raw_plan_response.clone(),
                raw_plan_description.clone(),
                raw_plan_tool_loop_state,
                self.clock.now_epoch_seconds(),
            ))
            .await?;

        let parsed = self.parse_window(&raw_plan_response)?;
        let mut days_by_date = parsed.days_by_date;
        let mut issues = parsed.issues;
        let mut invalid_day_sections = parsed.invalid_day_sections;
        if operation.validation_issues != issues {
            *operation = self
                .operations
                .upsert(
                    operation
                        .with_validation_issues(issues.clone(), self.clock.now_epoch_seconds()),
                )
                .await?;
        }

        self.apply_correction_rounds(
            CorrectionRoundInput {
                identity: GenerationIdentity {
                    user_id: identity.user_id,
                    workout_id: identity.workout_id,
                    saved_at_epoch_seconds: identity.saved_at_epoch_seconds,
                    recap: identity.recap,
                },
                planning: GenerationPlanning {
                    planning_context,
                    planning_context_loaded,
                },
                days_by_date: &mut days_by_date,
                issues: &mut issues,
                invalid_day_sections: &mut invalid_day_sections,
                operation,
                restored_tool_loop_state: None,
            },
            MAX_CORRECTION_ATTEMPTS,
        )
        .await?;

        if !issues.is_empty() {
            return Err(TrainingPlanError::Unavailable(
                "training plan generation failed validation after quality replan".to_string(),
            ));
        }

        let days = self.validate_snapshot_days(&days_by_date)?;
        let plan_text = self.render_plan_window(&days_by_date);
        *operation = self
            .operations
            .upsert(
                operation
                    .with_aligned_raw_plan_text(plan_text.clone(), self.clock.now_epoch_seconds()),
            )
            .await?;
        let snapshot = self.build_snapshot(
            identity.user_id,
            identity.workout_id,
            &operation.operation_key,
            identity.saved_at_epoch_seconds,
            days,
        )?;
        Ok(QualityReplanDraft {
            snapshot,
            draft: QualityDraft {
                plan_text,
                description: raw_plan_description,
            },
        })
    }
}

pub(super) fn emit_progress(
    messages: &mut Vec<String>,
    progress: Option<&Arc<dyn PlanQualityProgressPort>>,
    user_id: &str,
    workout_id: &str,
    message: String,
) {
    if let Some(progress) = progress {
        progress.on_quality_progress(user_id, workout_id, message.clone());
    }
    messages.push(message);
}
