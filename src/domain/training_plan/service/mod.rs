mod correction;
mod parsing;
mod snapshot;

use chrono::{TimeZone, Utc};

use std::collections::{BTreeMap, BTreeSet};

use crate::domain::{
    ai_workflow::{ValidationIssue, WorkflowPhase, WorkflowStatus},
    calendar_view::{CalendarEntryViewRefreshPort, NoopCalendarEntryViewRefresh},
    identity::Clock,
    workout_summary::WorkoutRecap,
};

use super::{
    BoxFuture, GeneratedTrainingPlan, TrainingPlanDay, TrainingPlanError,
    TrainingPlanGenerationClaimResult, TrainingPlanGenerationOperation,
    TrainingPlanGenerationOperationRepository, TrainingPlanGenerator,
    TrainingPlanProjectionRepository, TrainingPlanSnapshot, TrainingPlanSnapshotRepository,
    TrainingPlanWorkoutSummaryPort,
};

pub trait TrainingPlanUseCases: Send + Sync {
    fn generate_recap_for_saved_workout(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<WorkoutRecap, TrainingPlanError>>;

    fn generate_for_saved_workout(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<GeneratedTrainingPlan, TrainingPlanError>>;
}

#[derive(Clone)]
pub struct TrainingPlanGenerationService<
    Snapshots,
    Projections,
    Operations,
    Generator,
    WorkoutSummary,
    Time,
    Refresh = NoopCalendarEntryViewRefresh,
> where
    Snapshots: TrainingPlanSnapshotRepository + Clone,
    Projections: TrainingPlanProjectionRepository + Clone,
    Operations: TrainingPlanGenerationOperationRepository + Clone,
    Generator: TrainingPlanGenerator + Clone,
    WorkoutSummary: TrainingPlanWorkoutSummaryPort + Clone,
    Time: Clock + Clone,
    Refresh: CalendarEntryViewRefreshPort + Clone,
{
    snapshots: Snapshots,
    projections: Projections,
    operations: Operations,
    generator: Generator,
    workout_summary: WorkoutSummary,
    clock: Time,
    refresh: Refresh,
}

pub(super) struct ParsedPlanWindow {
    pub(super) days_by_date: BTreeMap<String, TrainingPlanDay>,
    pub(super) issues: Vec<ValidationIssue>,
    pub(super) invalid_day_sections: Vec<String>,
}

impl<Snapshots, Projections, Operations, Generator, WorkoutSummary, Time>
    TrainingPlanGenerationService<
        Snapshots,
        Projections,
        Operations,
        Generator,
        WorkoutSummary,
        Time,
    >
where
    Snapshots: TrainingPlanSnapshotRepository + Clone,
    Projections: TrainingPlanProjectionRepository + Clone,
    Operations: TrainingPlanGenerationOperationRepository + Clone,
    Generator: TrainingPlanGenerator + Clone,
    WorkoutSummary: TrainingPlanWorkoutSummaryPort + Clone,
    Time: Clock + Clone,
{
    pub fn new(
        snapshots: Snapshots,
        projections: Projections,
        operations: Operations,
        generator: Generator,
        workout_summary: WorkoutSummary,
        clock: Time,
    ) -> Self {
        Self {
            snapshots,
            projections,
            operations,
            generator,
            workout_summary,
            clock,
            refresh: NoopCalendarEntryViewRefresh,
        }
    }

    pub fn with_calendar_view_refresh<NewRefresh>(
        self,
        refresh: NewRefresh,
    ) -> TrainingPlanGenerationService<
        Snapshots,
        Projections,
        Operations,
        Generator,
        WorkoutSummary,
        Time,
        NewRefresh,
    >
    where
        NewRefresh: CalendarEntryViewRefreshPort + Clone,
    {
        TrainingPlanGenerationService {
            snapshots: self.snapshots,
            projections: self.projections,
            operations: self.operations,
            generator: self.generator,
            workout_summary: self.workout_summary,
            clock: self.clock,
            refresh,
        }
    }
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
    const STALE_PENDING_TIMEOUT_SECONDS: i64 = 300;
    const SNAPSHOT_DAY_COUNT: usize = 14;
    const MAX_CORRECTION_ATTEMPTS: usize = 2;

    fn operation_key(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
    ) -> String {
        format!("training-plan:{user_id}:{workout_id}:{saved_at_epoch_seconds}")
    }

    fn stale_pending_before_epoch_seconds(&self) -> i64 {
        self.clock.now_epoch_seconds() - Self::STALE_PENDING_TIMEOUT_SECONDS
    }

    fn today_string(&self) -> String {
        Utc.timestamp_opt(self.clock.now_epoch_seconds(), 0)
            .single()
            .map(|now| now.date_naive().format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "1970-01-01".to_string())
    }

    async fn existing_generated_plan(
        &self,
        operation_key: &str,
    ) -> Result<Option<GeneratedTrainingPlan>, TrainingPlanError> {
        let Some(snapshot) = self.snapshots.find_by_operation_key(operation_key).await? else {
            return Ok(None);
        };
        let today = self.today_string();
        let active_projected_days = self
            .projections
            .find_active_by_operation_key(operation_key)
            .await?
            .into_iter()
            .filter(|day| day.is_active_on(&today))
            .collect();
        Ok(Some(GeneratedTrainingPlan {
            snapshot,
            active_projected_days,
            was_generated: false,
        }))
    }

    async fn existing_generated_plan_with_healed_operation(
        &self,
        operation_key: &str,
    ) -> Result<Option<GeneratedTrainingPlan>, TrainingPlanError> {
        let Some(generated) = self.existing_generated_plan(operation_key).await? else {
            return Ok(None);
        };

        if let Some(operation) = self.operations.find_by_operation_key(operation_key).await? {
            if operation.status == WorkflowStatus::Pending {
                if self
                    .is_projection_persisted(&generated.snapshot, &generated.active_projected_days)
                {
                    self.operations
                        .upsert(operation.mark_completed(self.clock.now_epoch_seconds()))
                        .await?;
                } else {
                    return Ok(None);
                }
            }
        }

        Ok(Some(generated))
    }

    async fn fail_operation(
        &self,
        operation: &TrainingPlanGenerationOperation,
        phase: WorkflowPhase,
        error: TrainingPlanError,
        validation_issues: Vec<ValidationIssue>,
    ) -> Result<TrainingPlanError, TrainingPlanError> {
        let message = error.to_string();
        let failed = operation.mark_failed(
            phase,
            message.clone(),
            validation_issues,
            self.clock.now_epoch_seconds(),
        );
        self.operations.upsert(failed).await?;
        Ok(error)
    }

    fn workout_recap_from_operation(
        &self,
        operation: &TrainingPlanGenerationOperation,
    ) -> Result<WorkoutRecap, TrainingPlanError> {
        Ok(WorkoutRecap::generated(
            operation.workout_recap_text.clone().ok_or_else(|| {
                TrainingPlanError::Repository(
                    "stored training plan operation missing workout recap text".to_string(),
                )
            })?,
            operation.workout_recap_provider.clone().ok_or_else(|| {
                TrainingPlanError::Repository(
                    "stored training plan operation missing workout recap provider".to_string(),
                )
            })?,
            operation.workout_recap_model.clone().ok_or_else(|| {
                TrainingPlanError::Repository(
                    "stored training plan operation missing workout recap model".to_string(),
                )
            })?,
            operation
                .workout_recap_generated_at_epoch_seconds
                .ok_or_else(|| {
                    TrainingPlanError::Repository(
                        "stored training plan operation missing workout recap timestamp"
                            .to_string(),
                    )
                })?,
        ))
    }

    async fn persist_projection(
        &self,
        snapshot: TrainingPlanSnapshot,
        operation: TrainingPlanGenerationOperation,
    ) -> Result<GeneratedTrainingPlan, TrainingPlanError> {
        let today = self.today_string();
        let operation = operation.with_projection_update(self.clock.now_epoch_seconds());
        let operation = self.operations.upsert(operation).await?;
        let (snapshot, projected_days) = self
            .projections
            .replace_window(
                snapshot.clone(),
                self.build_projected_days(&snapshot),
                &today,
                self.clock.now_epoch_seconds(),
            )
            .await?;
        let active_projected_days = projected_days
            .into_iter()
            .filter(|day| day.is_active_on(&today))
            .collect::<Vec<_>>();
        let operation = self
            .operations
            .upsert(operation.mark_projection_persisted(self.clock.now_epoch_seconds()))
            .await?;
        if !self.is_projection_persisted(&snapshot, &active_projected_days) {
            return Err(TrainingPlanError::Repository(
                "training plan projection persistence incomplete after replace_window".to_string(),
            ));
        }
        self.refresh
            .refresh_range_for_user(&snapshot.user_id, &snapshot.start_date, &snapshot.end_date)
            .await
            .map_err(|error| TrainingPlanError::Repository(error.to_string()))?;
        let completed = operation.mark_completed(self.clock.now_epoch_seconds());
        self.operations.upsert(completed).await?;

        Ok(GeneratedTrainingPlan {
            snapshot,
            active_projected_days,
            was_generated: true,
        })
    }
}

impl<Snapshots, Projections, Operations, Generator, WorkoutSummary, Time, Refresh>
    TrainingPlanUseCases
    for TrainingPlanGenerationService<
        Snapshots,
        Projections,
        Operations,
        Generator,
        WorkoutSummary,
        Time,
        Refresh,
    >
where
    Snapshots: TrainingPlanSnapshotRepository + Clone + 'static,
    Projections: TrainingPlanProjectionRepository + Clone + 'static,
    Operations: TrainingPlanGenerationOperationRepository + Clone + 'static,
    Generator: TrainingPlanGenerator + Clone + 'static,
    WorkoutSummary: TrainingPlanWorkoutSummaryPort + Clone + 'static,
    Time: Clock + Clone + 'static,
    Refresh: CalendarEntryViewRefreshPort + Clone + 'static,
{
    fn generate_recap_for_saved_workout(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<WorkoutRecap, TrainingPlanError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let recap = service
                .generator
                .generate_workout_recap(&user_id, &workout_id, saved_at_epoch_seconds)
                .await?;
            service
                .workout_summary
                .persist_workout_recap(&user_id, &workout_id, recap.clone())
                .await?;
            Ok(recap)
        })
    }

    fn generate_for_saved_workout(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<GeneratedTrainingPlan, TrainingPlanError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let operation_key =
                service.operation_key(&user_id, &workout_id, saved_at_epoch_seconds);
            if let Some(existing) = service
                .existing_generated_plan_with_healed_operation(&operation_key)
                .await?
            {
                return Ok(existing);
            }

            let pending = TrainingPlanGenerationOperation::pending(
                operation_key.clone(),
                user_id.clone(),
                workout_id.clone(),
                saved_at_epoch_seconds,
                service.clock.now_epoch_seconds(),
            );

            let mut operation = match service
                .operations
                .claim_pending(pending, service.stale_pending_before_epoch_seconds())
                .await?
            {
                TrainingPlanGenerationClaimResult::Claimed(operation) => operation,
                TrainingPlanGenerationClaimResult::Existing(existing) => match existing.status {
                    WorkflowStatus::Completed => {
                        if let Some(generated) =
                            service.existing_generated_plan(&operation_key).await?
                        {
                            return Ok(generated);
                        }
                        return Err(TrainingPlanError::Unavailable(
                            "training plan generation already completed without stored snapshot"
                                .to_string(),
                        ));
                    }
                    WorkflowStatus::Pending => {
                        return Err(TrainingPlanError::Unavailable(
                            "training plan generation already in progress".to_string(),
                        ));
                    }
                    WorkflowStatus::Failed => existing,
                },
            };

            let recap = if operation.workout_recap_text.is_some() {
                match service.workout_recap_from_operation(&operation) {
                    Ok(recap) => recap,
                    Err(error) => {
                        return Err(service
                            .fail_operation(
                                &operation,
                                WorkflowPhase::WorkoutRecap,
                                error,
                                Vec::new(),
                            )
                            .await?)
                    }
                }
            } else {
                let recap = match service
                    .generator
                    .generate_workout_recap(&user_id, &workout_id, saved_at_epoch_seconds)
                    .await
                {
                    Ok(recap) => recap,
                    Err(error) => {
                        return Err(service
                            .fail_operation(
                                &operation,
                                WorkflowPhase::WorkoutRecap,
                                error,
                                Vec::new(),
                            )
                            .await?)
                    }
                };
                operation = service
                    .operations
                    .upsert(operation.with_workout_recap(
                        recap.text.clone(),
                        recap.provider.clone(),
                        recap.model.clone(),
                        service.clock.now_epoch_seconds(),
                    ))
                    .await?;
                service
                    .workout_summary
                    .persist_workout_recap(&user_id, &workout_id, recap.clone())
                    .await?;
                recap
            };

            let raw_plan_response =
                if let Some(raw_plan_response) = operation.raw_plan_response.clone() {
                    raw_plan_response
                } else {
                    let raw_plan_response = match service
                        .generator
                        .generate_initial_plan_window(
                            &user_id,
                            &workout_id,
                            saved_at_epoch_seconds,
                            &recap,
                        )
                        .await
                    {
                        Ok(raw_plan_response) => raw_plan_response,
                        Err(error) => {
                            return Err(service
                                .fail_operation(
                                    &operation,
                                    WorkflowPhase::InitialGeneration,
                                    error,
                                    Vec::new(),
                                )
                                .await?)
                        }
                    };
                    operation = service
                        .operations
                        .upsert(operation.with_raw_plan_response(
                            raw_plan_response.clone(),
                            service.clock.now_epoch_seconds(),
                        ))
                        .await?;
                    raw_plan_response
                };

            let parsed = match service.parse_window(&raw_plan_response) {
                Ok(parsed) => parsed,
                Err(error) => {
                    return Err(service
                        .fail_operation(
                            &operation,
                            WorkflowPhase::InitialGeneration,
                            error,
                            operation.validation_issues.clone(),
                        )
                        .await?)
                }
            };

            let mut days_by_date = parsed.days_by_date;
            let mut issues = parsed.issues;
            let mut invalid_day_sections = parsed.invalid_day_sections;
            if operation.validation_issues != issues {
                operation =
                    service
                        .operations
                        .upsert(operation.with_validation_issues(
                            issues.clone(),
                            service.clock.now_epoch_seconds(),
                        ))
                        .await?;
            }

            let mut invalid_dates = issues
                .iter()
                .map(|issue| issue.scope.clone())
                .collect::<BTreeSet<_>>();

            if !invalid_dates.is_empty() {
                if let Some(raw_correction_response) = operation.raw_correction_response.clone() {
                    let corrected = match service.parse_window(&raw_correction_response) {
                        Ok(corrected) => corrected,
                        Err(error) => {
                            return Err(service
                                .fail_operation(
                                    &operation,
                                    WorkflowPhase::Correction,
                                    error,
                                    operation.validation_issues.clone(),
                                )
                                .await?)
                        }
                    };
                    let corrected_dates = corrected
                        .days_by_date
                        .keys()
                        .cloned()
                        .collect::<BTreeSet<_>>();
                    service.merge_corrections(
                        &mut days_by_date,
                        corrected.days_by_date,
                        &invalid_dates,
                    );
                    let corrected_invalid_dates = corrected
                        .issues
                        .iter()
                        .map(|issue| issue.scope.clone())
                        .collect::<BTreeSet<_>>();
                    issues = correction::merge_unresolved_issues(
                        &issues,
                        &corrected.issues,
                        &corrected_dates,
                        &corrected_invalid_dates,
                    );
                    invalid_day_sections = correction::merge_invalid_day_sections(
                        &invalid_day_sections,
                        &corrected.invalid_day_sections,
                        &corrected_dates,
                        &corrected_invalid_dates,
                    );
                    if operation.validation_issues != issues {
                        operation = service
                            .operations
                            .upsert(operation.with_validation_issues(
                                issues.clone(),
                                service.clock.now_epoch_seconds(),
                            ))
                            .await?;
                    }
                    invalid_dates = issues
                        .iter()
                        .map(|issue| issue.scope.clone())
                        .collect::<BTreeSet<_>>();
                }

                let correction_attempts_recorded = operation
                    .attempts
                    .iter()
                    .filter(|attempt| attempt.phase == WorkflowPhase::Correction)
                    .count();
                let correction_attempts_remaining =
                    Self::MAX_CORRECTION_ATTEMPTS.saturating_sub(correction_attempts_recorded);
                for _ in 0..correction_attempts_remaining {
                    if issues.is_empty() {
                        break;
                    }

                    let correction_response = match service
                        .generator
                        .correct_invalid_days(
                            &user_id,
                            &workout_id,
                            saved_at_epoch_seconds,
                            &recap,
                            &invalid_day_sections.join("\n\n"),
                            issues.clone(),
                        )
                        .await
                    {
                        Ok(correction_response) => correction_response,
                        Err(error) => {
                            return Err(service
                                .fail_operation(
                                    &operation,
                                    WorkflowPhase::Correction,
                                    error,
                                    operation.validation_issues.clone(),
                                )
                                .await?)
                        }
                    };
                    operation = service
                        .operations
                        .upsert(operation.with_correction_response(
                            correction_response.clone(),
                            service.clock.now_epoch_seconds(),
                        ))
                        .await?;

                    let corrected = match service.parse_window(&correction_response) {
                        Ok(corrected) => corrected,
                        Err(error) => {
                            return Err(service
                                .fail_operation(
                                    &operation,
                                    WorkflowPhase::Correction,
                                    error,
                                    operation.validation_issues.clone(),
                                )
                                .await?)
                        }
                    };
                    let corrected_dates = corrected
                        .days_by_date
                        .keys()
                        .cloned()
                        .collect::<BTreeSet<_>>();
                    service.merge_corrections(
                        &mut days_by_date,
                        corrected.days_by_date,
                        &invalid_dates,
                    );
                    let corrected_invalid_dates = corrected
                        .issues
                        .iter()
                        .map(|issue| issue.scope.clone())
                        .collect::<BTreeSet<_>>();
                    issues = correction::merge_unresolved_issues(
                        &issues,
                        &corrected.issues,
                        &corrected_dates,
                        &corrected_invalid_dates,
                    );
                    invalid_day_sections = correction::merge_invalid_day_sections(
                        &invalid_day_sections,
                        &corrected.invalid_day_sections,
                        &corrected_dates,
                        &corrected_invalid_dates,
                    );
                    if operation.validation_issues != issues {
                        operation = service
                            .operations
                            .upsert(operation.with_validation_issues(
                                issues.clone(),
                                service.clock.now_epoch_seconds(),
                            ))
                            .await?;
                    }
                    invalid_dates = issues
                        .iter()
                        .map(|issue| issue.scope.clone())
                        .collect::<BTreeSet<_>>();
                }

                if !issues.is_empty() {
                    let failed = operation.mark_failed(
                        WorkflowPhase::Correction,
                        "training plan generation failed validation".to_string(),
                        issues.clone(),
                        service.clock.now_epoch_seconds(),
                    );
                    service.operations.upsert(failed).await?;
                    return Err(TrainingPlanError::Unavailable(
                        "training plan generation failed validation".to_string(),
                    ));
                }
            }

            let days = match service.validate_snapshot_days(&days_by_date) {
                Ok(days) => days,
                Err(error) => {
                    return Err(service
                        .fail_operation(
                            &operation,
                            WorkflowPhase::InitialGeneration,
                            error,
                            operation.validation_issues.clone(),
                        )
                        .await?)
                }
            };

            let snapshot = match service.build_snapshot(
                &user_id,
                &workout_id,
                &operation.operation_key,
                saved_at_epoch_seconds,
                days,
            ) {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    return Err(service
                        .fail_operation(
                            &operation,
                            WorkflowPhase::ProjectionUpdate,
                            error,
                            operation.validation_issues.clone(),
                        )
                        .await?)
                }
            };

            service.persist_projection(snapshot, operation).await
        })
    }
}
