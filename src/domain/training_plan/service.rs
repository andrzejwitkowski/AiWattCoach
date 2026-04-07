use chrono::{NaiveDate, TimeZone, Utc};
use std::collections::{BTreeMap, BTreeSet};

use crate::domain::{
    ai_workflow::{ValidationIssue, WorkflowPhase, WorkflowStatus},
    identity::Clock,
    intervals::{parse_planned_workout_days, PlannedWorkoutDay},
    workout_summary::WorkoutRecap,
};

use super::{
    BoxFuture, GeneratedTrainingPlan, TrainingPlanDay, TrainingPlanError,
    TrainingPlanGenerationClaimResult, TrainingPlanGenerationOperation,
    TrainingPlanGenerationOperationRepository, TrainingPlanGenerator, TrainingPlanProjectedDay,
    TrainingPlanProjectionRepository, TrainingPlanSnapshot, TrainingPlanSnapshotRepository,
    TrainingPlanWorkoutSummaryPort,
};

pub trait TrainingPlanUseCases: Send + Sync {
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
> where
    Snapshots: TrainingPlanSnapshotRepository + Clone,
    Projections: TrainingPlanProjectionRepository + Clone,
    Operations: TrainingPlanGenerationOperationRepository + Clone,
    Generator: TrainingPlanGenerator + Clone,
    WorkoutSummary: TrainingPlanWorkoutSummaryPort + Clone,
    Time: Clock + Clone,
{
    snapshots: Snapshots,
    projections: Projections,
    operations: Operations,
    generator: Generator,
    workout_summary: WorkoutSummary,
    clock: Time,
}

struct ParsedPlanWindow {
    days_by_date: BTreeMap<String, TrainingPlanDay>,
    issues: Vec<ValidationIssue>,
    invalid_day_sections: Vec<String>,
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
    const STALE_PENDING_TIMEOUT_SECONDS: i64 = 300;
    const SNAPSHOT_DAY_COUNT: usize = 14;
    const MAX_CORRECTION_ATTEMPTS: usize = 2;

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
        }
    }

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
        let active_projected_days = self
            .projections
            .find_active_by_operation_key(operation_key)
            .await?;
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

    fn map_parsed_day(day: PlannedWorkoutDay) -> TrainingPlanDay {
        TrainingPlanDay {
            date: day.date,
            rest_day: day.rest_day,
            workout: day.workout,
        }
    }

    fn split_into_day_blocks(
        &self,
        input: &str,
    ) -> Result<Vec<(String, String)>, TrainingPlanError> {
        let mut blocks = Vec::new();
        let mut current_date: Option<String> = None;
        let mut current_lines = Vec::new();

        for raw_line in input.lines() {
            let line = raw_line.trim();
            if line.is_empty() {
                continue;
            }

            if Self::is_exact_date(line) {
                if let Some(date) = current_date.take() {
                    let mut block = Vec::with_capacity(current_lines.len() + 1);
                    block.push(date.clone());
                    block.extend(current_lines.clone());
                    blocks.push((date, block.join("\n")));
                    current_lines.clear();
                }
                current_date = Some(line.to_string());
                continue;
            }

            if current_date.is_none() {
                return Err(TrainingPlanError::Validation(
                    "content before first date header".to_string(),
                ));
            }

            current_lines.push(line.to_string());
        }

        if let Some(date) = current_date {
            let mut block = Vec::with_capacity(current_lines.len() + 1);
            block.push(date.clone());
            block.extend(current_lines);
            blocks.push((date, block.join("\n")));
        }

        Ok(blocks)
    }

    fn is_exact_date(value: &str) -> bool {
        let bytes = value.as_bytes();
        bytes.len() == 10
            && bytes[0..4].iter().all(u8::is_ascii_digit)
            && bytes[4] == b'-'
            && bytes[5..7].iter().all(u8::is_ascii_digit)
            && bytes[7] == b'-'
            && bytes[8..10].iter().all(u8::is_ascii_digit)
    }

    fn parse_window(&self, raw_window: &str) -> Result<ParsedPlanWindow, TrainingPlanError> {
        let blocks = self.split_into_day_blocks(raw_window)?;
        let mut days_by_date = BTreeMap::new();
        let mut issues = Vec::new();
        let mut invalid_day_sections = Vec::new();

        for (date, block) in blocks {
            if days_by_date.contains_key(&date) {
                return Err(TrainingPlanError::Validation(format!(
                    "duplicate planned workout day: {date}"
                )));
            }

            match parse_planned_workout_days(&block) {
                Ok(parsed) => {
                    if let Some(day) = parsed.days.into_iter().next() {
                        days_by_date.insert(date, Self::map_parsed_day(day));
                    }
                }
                Err(error) => {
                    issues.push(ValidationIssue {
                        scope: date,
                        message: error.to_string(),
                    });
                    invalid_day_sections.push(block);
                }
            }
        }

        Ok(ParsedPlanWindow {
            days_by_date,
            issues,
            invalid_day_sections,
        })
    }

    fn merge_corrections(
        &self,
        base_days: &mut BTreeMap<String, TrainingPlanDay>,
        corrected_days: BTreeMap<String, TrainingPlanDay>,
        invalid_dates: &BTreeSet<String>,
    ) {
        for (date, day) in corrected_days {
            if invalid_dates.contains(&date) {
                base_days.insert(date, day);
            }
        }
    }

    fn days_are_contiguous(days: &[TrainingPlanDay]) -> bool {
        days.windows(2).all(|window| {
            let left = NaiveDate::parse_from_str(&window[0].date, "%Y-%m-%d").ok();
            let right = NaiveDate::parse_from_str(&window[1].date, "%Y-%m-%d").ok();
            match (left, right) {
                (Some(left), Some(right)) => right == left + chrono::Duration::days(1),
                _ => false,
            }
        })
    }

    fn validate_snapshot_days(
        &self,
        days_by_date: &BTreeMap<String, TrainingPlanDay>,
    ) -> Result<Vec<TrainingPlanDay>, TrainingPlanError> {
        let days = days_by_date.values().cloned().collect::<Vec<_>>();
        if days.len() != Self::SNAPSHOT_DAY_COUNT || !Self::days_are_contiguous(&days) {
            return Err(TrainingPlanError::Validation(format!(
                "training plan window must contain exactly {} contiguous dated days",
                Self::SNAPSHOT_DAY_COUNT
            )));
        }
        Ok(days)
    }

    fn build_snapshot(
        &self,
        user_id: &str,
        workout_id: &str,
        operation_key: &str,
        saved_at_epoch_seconds: i64,
        days: Vec<TrainingPlanDay>,
    ) -> Result<TrainingPlanSnapshot, TrainingPlanError> {
        let start_date = days.first().map(|day| day.date.clone()).ok_or_else(|| {
            TrainingPlanError::Validation("training plan window is empty".to_string())
        })?;
        let end_date = days.last().map(|day| day.date.clone()).ok_or_else(|| {
            TrainingPlanError::Validation("training plan window is empty".to_string())
        })?;

        Ok(TrainingPlanSnapshot {
            user_id: user_id.to_string(),
            workout_id: workout_id.to_string(),
            operation_key: operation_key.to_string(),
            saved_at_epoch_seconds,
            start_date,
            end_date,
            days,
            created_at_epoch_seconds: self.clock.now_epoch_seconds(),
        })
    }

    fn build_projected_days(
        &self,
        snapshot: &TrainingPlanSnapshot,
    ) -> Vec<TrainingPlanProjectedDay> {
        let today = self.today_string();
        snapshot
            .days
            .iter()
            .map(|day| TrainingPlanProjectedDay {
                user_id: snapshot.user_id.clone(),
                workout_id: snapshot.workout_id.clone(),
                operation_key: snapshot.operation_key.clone(),
                date: day.date.clone(),
                rest_day: day.rest_day,
                workout: day.workout.clone(),
                active: day.date > today,
                superseded_at_epoch_seconds: None,
                created_at_epoch_seconds: self.clock.now_epoch_seconds(),
                updated_at_epoch_seconds: self.clock.now_epoch_seconds(),
            })
            .collect()
    }

    fn expected_active_projected_dates(&self, snapshot: &TrainingPlanSnapshot) -> BTreeSet<String> {
        let today = self.today_string();
        snapshot
            .days
            .iter()
            .filter(|day| day.date > today)
            .map(|day| day.date.clone())
            .collect()
    }

    fn is_projection_persisted(
        &self,
        snapshot: &TrainingPlanSnapshot,
        active_projected_days: &[TrainingPlanProjectedDay],
    ) -> bool {
        let expected_dates = self.expected_active_projected_dates(snapshot);
        let actual_dates = active_projected_days
            .iter()
            .filter(|day| day.active && day.operation_key == snapshot.operation_key)
            .map(|day| day.date.clone())
            .collect::<BTreeSet<_>>();
        actual_dates == expected_dates
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
        let operation = operation.with_projection_update(self.clock.now_epoch_seconds());
        let operation = self.operations.upsert(operation).await?;
        let (snapshot, active_projected_days) = self
            .projections
            .replace_window(
                snapshot.clone(),
                self.build_projected_days(&snapshot),
                &self.today_string(),
                self.clock.now_epoch_seconds(),
            )
            .await?;
        let operation = self
            .operations
            .upsert(operation.mark_projection_persisted(self.clock.now_epoch_seconds()))
            .await?;
        if !self.is_projection_persisted(&snapshot, &active_projected_days) {
            return Err(TrainingPlanError::Repository(
                "training plan projection persistence incomplete after replace_window".to_string(),
            ));
        }
        let completed = operation.mark_completed(self.clock.now_epoch_seconds());
        self.operations.upsert(completed).await?;

        Ok(GeneratedTrainingPlan {
            snapshot,
            active_projected_days,
            was_generated: true,
        })
    }
}

impl<Snapshots, Projections, Operations, Generator, WorkoutSummary, Time> TrainingPlanUseCases
    for TrainingPlanGenerationService<
        Snapshots,
        Projections,
        Operations,
        Generator,
        WorkoutSummary,
        Time,
    >
where
    Snapshots: TrainingPlanSnapshotRepository + Clone + 'static,
    Projections: TrainingPlanProjectionRepository + Clone + 'static,
    Operations: TrainingPlanGenerationOperationRepository + Clone + 'static,
    Generator: TrainingPlanGenerator + Clone + 'static,
    WorkoutSummary: TrainingPlanWorkoutSummaryPort + Clone + 'static,
    Time: Clock + Clone + 'static,
{
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

            if operation.workout_recap_text.is_some() && operation.raw_plan_response.is_none() {
                service
                    .workout_summary
                    .persist_workout_recap(&user_id, &workout_id, recap.clone())
                    .await?;
            }

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
                    issues = merge_unresolved_issues(
                        &issues,
                        &corrected.issues,
                        &corrected_dates,
                        &corrected_invalid_dates,
                    );
                    invalid_day_sections = merge_invalid_day_sections(
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
                    issues = merge_unresolved_issues(
                        &issues,
                        &corrected.issues,
                        &corrected_dates,
                        &corrected_invalid_dates,
                    );
                    invalid_day_sections = merge_invalid_day_sections(
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

fn merge_unresolved_issues(
    previous: &[ValidationIssue],
    corrected: &[ValidationIssue],
    corrected_dates: &BTreeSet<String>,
    corrected_invalid_dates: &BTreeSet<String>,
) -> Vec<ValidationIssue> {
    let mut merged_by_scope = previous
        .iter()
        .filter(|issue| {
            !corrected_dates.contains(&issue.scope)
                || corrected_invalid_dates.contains(&issue.scope)
        })
        .map(|issue| (issue.scope.clone(), issue.clone()))
        .collect::<BTreeMap<_, _>>();

    for issue in corrected {
        merged_by_scope.insert(issue.scope.clone(), issue.clone());
    }

    merged_by_scope.into_values().collect()
}

fn merge_invalid_day_sections(
    previous_sections: &[String],
    corrected_sections: &[String],
    corrected_dates: &BTreeSet<String>,
    corrected_invalid_dates: &BTreeSet<String>,
) -> Vec<String> {
    let mut merged_by_date = previous_sections
        .iter()
        .filter_map(|section| {
            section
                .lines()
                .next()
                .filter(|date| {
                    !corrected_dates.contains(*date) || corrected_invalid_dates.contains(*date)
                })
                .map(|date| (date.to_string(), section.clone()))
        })
        .collect::<BTreeMap<_, _>>();

    for section in corrected_sections {
        if let Some(date) = section.lines().next() {
            merged_by_date.insert(date.to_string(), section.clone());
        }
    }

    merged_by_date.into_values().collect()
}
