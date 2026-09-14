use std::sync::Arc;

use super::super::{
    format_quality_feedback, plan_quality_attempt_message, plan_quality_finished_accepted_message,
    plan_quality_finished_best_message, PlanQualityEvaluation, PlanQualityEvaluatorLlmConfigPort,
    PlanQualityProgressPort, TrainingPlanError, TrainingPlanGenerationOperation,
    TrainingPlanSnapshot, PLAN_QUALITY_PASS_SCORE,
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

pub(super) struct PlanQualityLoopInput<'a> {
    pub identity: GenerationIdentity<'a>,
    pub planning: GenerationPlanning<'a>,
    pub snapshot: TrainingPlanSnapshot,
    pub draft_plan_text: String,
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

struct BestDraft {
    snapshot: TrainingPlanSnapshot,
    evaluation: PlanQualityEvaluation,
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
            mut draft_plan_text,
            mut operation,
            plan_quality_config,
            plan_quality_progress,
        } = input;

        let max_loops = plan_quality_config
            .get_plan_quality_max_loops(identity.user_id)
            .await?
            .max(1);
        let mut quality_progress_messages = Vec::new();
        let mut best =
            self.seed_best_quality_draft(&identity, &operation, &snapshot, &draft_plan_text)?;
        let start_attempt = (operation.quality_evaluations.len() as u32).saturating_add(1);
        let mut accepted = best
            .as_ref()
            .is_some_and(|draft| draft.evaluation.score >= PLAN_QUALITY_PASS_SCORE);

        if !accepted {
            for attempt in start_attempt..=max_loops {
                let mut evaluation = match self
                    .generator
                    .evaluate_plan_quality(
                        identity.user_id,
                        identity.workout_id,
                        identity.saved_at_epoch_seconds,
                        identity.recap,
                        planning.planning_context.as_ref(),
                        &draft_plan_text,
                    )
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
                        break;
                    }
                };
                evaluation.attempt = attempt;

                emit_progress(
                    &mut quality_progress_messages,
                    plan_quality_progress,
                    identity.user_id,
                    identity.workout_id,
                    plan_quality_attempt_message(
                        attempt,
                        max_loops,
                        evaluation.score,
                        &evaluation.critique,
                    ),
                );

                let is_better = best
                    .as_ref()
                    .is_none_or(|previous| evaluation.score >= previous.evaluation.score);
                if is_better {
                    best = Some(BestDraft {
                        snapshot: snapshot.clone(),
                        evaluation: evaluation.clone(),
                    });
                }

                operation = self
                    .operations
                    .upsert(operation.with_quality_evaluation(
                        evaluation.clone(),
                        is_better.then(|| draft_plan_text.clone()),
                        self.clock.now_epoch_seconds(),
                    ))
                    .await?;

                if evaluation.score >= PLAN_QUALITY_PASS_SCORE {
                    accepted = true;
                    break;
                }
                if attempt == max_loops {
                    break;
                }

                let feedback = format_quality_feedback(evaluation.score, &evaluation.critique);
                match self
                    .regenerate_structurally_valid_snapshot(
                        &identity,
                        GenerationPlanning {
                            planning_context: &mut *planning.planning_context,
                            planning_context_loaded: &mut *planning.planning_context_loaded,
                        },
                        &mut operation,
                        &feedback,
                    )
                    .await
                {
                    Ok((next_snapshot, next_draft)) => {
                        snapshot = next_snapshot;
                        draft_plan_text = next_draft;
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

        let best = match best {
            Some(best) => best,
            None => {
                // Evaluator never produced a score (outage / empty resume range). Ship the
                // structurally valid draft without quality metadata rather than failing save.
                return Ok(PlanQualityLoopResult {
                    snapshot,
                    quality_evaluations: operation.quality_evaluations.clone(),
                    shipped_quality: None,
                    quality_progress_messages,
                    operation,
                });
            }
        };
        let finished = if accepted {
            plan_quality_finished_accepted_message(best.evaluation.score)
        } else {
            plan_quality_finished_best_message(best.evaluation.score, max_loops)
        };
        emit_progress(
            &mut quality_progress_messages,
            plan_quality_progress,
            identity.user_id,
            identity.workout_id,
            finished,
        );

        Ok(PlanQualityLoopResult {
            snapshot: best.snapshot,
            quality_evaluations: operation.quality_evaluations.clone(),
            shipped_quality: Some(best.evaluation),
            quality_progress_messages,
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
            // Recover inconsistent stored state: evaluations without best draft fields.
            let Some(evaluation) = operation.best_quality_evaluation.clone().or_else(|| {
                operation
                    .quality_evaluations
                    .iter()
                    .max_by_key(|evaluation| (evaluation.score, evaluation.attempt))
                    .cloned()
            }) else {
                return Ok(None);
            };
            return Ok(Some(BestDraft {
                snapshot: current_snapshot.clone(),
                evaluation,
            }));
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
    ) -> Result<(TrainingPlanSnapshot, String), TrainingPlanError> {
        self.ensure_planning_context_loaded(
            planning.planning_context,
            planning.planning_context_loaded,
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
                planning.planning_context.as_ref(),
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
                raw_plan_description,
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
                planning,
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
        let snapshot = self.build_snapshot(
            identity.user_id,
            identity.workout_id,
            &operation.operation_key,
            identity.saved_at_epoch_seconds,
            days,
        )?;
        Ok((snapshot, raw_plan_response))
    }
}

fn emit_progress(
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
