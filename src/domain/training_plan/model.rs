use crate::domain::{
    ai_workflow::{AttemptRecord, ValidationIssue, WorkflowPhase, WorkflowStatus},
    intervals::PlannedWorkout,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrainingPlanError {
    Unavailable(String),
    Repository(String),
    Validation(String),
}

impl std::fmt::Display for TrainingPlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(message) => write!(f, "{message}"),
            Self::Repository(message) => write!(f, "{message}"),
            Self::Validation(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for TrainingPlanError {}

#[derive(Clone, Debug, PartialEq)]
pub struct TrainingPlanDay {
    pub date: String,
    pub rest_day: bool,
    pub workout: Option<PlannedWorkout>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrainingPlanSnapshot {
    pub user_id: String,
    pub workout_id: String,
    pub operation_key: String,
    pub saved_at_epoch_seconds: i64,
    pub start_date: String,
    pub end_date: String,
    pub days: Vec<TrainingPlanDay>,
    pub created_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrainingPlanProjectedDay {
    pub user_id: String,
    pub workout_id: String,
    pub operation_key: String,
    pub date: String,
    pub rest_day: bool,
    pub workout: Option<PlannedWorkout>,
    pub superseded_at_epoch_seconds: Option<i64>,
    pub created_at_epoch_seconds: i64,
    pub updated_at_epoch_seconds: i64,
}

impl TrainingPlanProjectedDay {
    pub fn is_active_on(&self, today: &str) -> bool {
        self.superseded_at_epoch_seconds.is_none() && self.date.as_str() > today
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrainingPlanFailureState {
    pub phase: WorkflowPhase,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrainingPlanGenerationOperation {
    pub operation_key: String,
    pub user_id: String,
    pub workout_id: String,
    pub saved_at_epoch_seconds: i64,
    pub status: WorkflowStatus,
    pub workout_recap_text: Option<String>,
    pub workout_recap_provider: Option<String>,
    pub workout_recap_model: Option<String>,
    pub workout_recap_generated_at_epoch_seconds: Option<i64>,
    pub projection_persisted_at_epoch_seconds: Option<i64>,
    pub raw_plan_response: Option<String>,
    pub raw_correction_response: Option<String>,
    pub validation_issues: Vec<ValidationIssue>,
    pub attempts: Vec<AttemptRecord>,
    pub failure: Option<TrainingPlanFailureState>,
    pub started_at_epoch_seconds: i64,
    pub last_attempt_at_epoch_seconds: i64,
    pub attempt_count: u32,
    pub created_at_epoch_seconds: i64,
    pub updated_at_epoch_seconds: i64,
}

impl TrainingPlanGenerationOperation {
    pub fn pending(
        operation_key: String,
        user_id: String,
        workout_id: String,
        saved_at_epoch_seconds: i64,
        now_epoch_seconds: i64,
    ) -> Self {
        Self {
            operation_key,
            user_id,
            workout_id,
            saved_at_epoch_seconds,
            status: WorkflowStatus::Pending,
            workout_recap_text: None,
            workout_recap_provider: None,
            workout_recap_model: None,
            workout_recap_generated_at_epoch_seconds: None,
            projection_persisted_at_epoch_seconds: None,
            raw_plan_response: None,
            raw_correction_response: None,
            validation_issues: Vec::new(),
            attempts: Vec::new(),
            failure: None,
            started_at_epoch_seconds: now_epoch_seconds,
            last_attempt_at_epoch_seconds: now_epoch_seconds,
            attempt_count: 1,
            created_at_epoch_seconds: now_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
        }
    }

    pub fn reclaim(&self, now_epoch_seconds: i64) -> Self {
        Self {
            operation_key: self.operation_key.clone(),
            user_id: self.user_id.clone(),
            workout_id: self.workout_id.clone(),
            saved_at_epoch_seconds: self.saved_at_epoch_seconds,
            status: WorkflowStatus::Pending,
            workout_recap_text: self.workout_recap_text.clone(),
            workout_recap_provider: self.workout_recap_provider.clone(),
            workout_recap_model: self.workout_recap_model.clone(),
            workout_recap_generated_at_epoch_seconds: self.workout_recap_generated_at_epoch_seconds,
            projection_persisted_at_epoch_seconds: self.projection_persisted_at_epoch_seconds,
            raw_plan_response: self.raw_plan_response.clone(),
            raw_correction_response: self.raw_correction_response.clone(),
            validation_issues: self.validation_issues.clone(),
            attempts: self.attempts.clone(),
            failure: None,
            started_at_epoch_seconds: self.started_at_epoch_seconds,
            last_attempt_at_epoch_seconds: now_epoch_seconds,
            attempt_count: self.attempt_count.saturating_add(1),
            created_at_epoch_seconds: self.created_at_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
        }
    }

    fn clone_pending_update(&self, updated_at_epoch_seconds: i64) -> Self {
        Self {
            operation_key: self.operation_key.clone(),
            user_id: self.user_id.clone(),
            workout_id: self.workout_id.clone(),
            saved_at_epoch_seconds: self.saved_at_epoch_seconds,
            status: WorkflowStatus::Pending,
            workout_recap_text: self.workout_recap_text.clone(),
            workout_recap_provider: self.workout_recap_provider.clone(),
            workout_recap_model: self.workout_recap_model.clone(),
            workout_recap_generated_at_epoch_seconds: self.workout_recap_generated_at_epoch_seconds,
            projection_persisted_at_epoch_seconds: self.projection_persisted_at_epoch_seconds,
            raw_plan_response: self.raw_plan_response.clone(),
            raw_correction_response: self.raw_correction_response.clone(),
            validation_issues: self.validation_issues.clone(),
            attempts: self.attempts.clone(),
            failure: None,
            started_at_epoch_seconds: self.started_at_epoch_seconds,
            last_attempt_at_epoch_seconds: self.last_attempt_at_epoch_seconds,
            attempt_count: self.attempt_count,
            created_at_epoch_seconds: self.created_at_epoch_seconds,
            updated_at_epoch_seconds,
        }
    }

    pub fn with_workout_recap(
        &self,
        text: String,
        provider: String,
        model: String,
        recorded_at_epoch_seconds: i64,
    ) -> Self {
        let mut attempts = self.attempts.clone();
        if self.workout_recap_text.is_none() {
            attempts.push(AttemptRecord {
                phase: WorkflowPhase::WorkoutRecap,
                attempt_number: attempts
                    .iter()
                    .filter(|attempt| attempt.phase == WorkflowPhase::WorkoutRecap)
                    .count() as u32
                    + 1,
                recorded_at_epoch_seconds,
            });
        }
        attempts.push(AttemptRecord {
            phase: WorkflowPhase::InitialGeneration,
            attempt_number: attempts
                .iter()
                .filter(|attempt| attempt.phase == WorkflowPhase::InitialGeneration)
                .count() as u32
                + 1,
            recorded_at_epoch_seconds,
        });

        let mut updated = self.clone_pending_update(recorded_at_epoch_seconds);
        updated.workout_recap_text = Some(text);
        updated.workout_recap_provider = Some(provider);
        updated.workout_recap_model = Some(model);
        updated.workout_recap_generated_at_epoch_seconds = Some(recorded_at_epoch_seconds);
        updated.attempts = attempts;
        updated
    }

    pub fn with_raw_plan_response(
        &self,
        raw_plan_response: String,
        recorded_at_epoch_seconds: i64,
    ) -> Self {
        let mut updated = self.clone_pending_update(recorded_at_epoch_seconds);
        updated.raw_plan_response = Some(raw_plan_response);
        updated
    }

    pub fn with_validation_issues(
        &self,
        validation_issues: Vec<ValidationIssue>,
        updated_at_epoch_seconds: i64,
    ) -> Self {
        let mut updated = self.clone_pending_update(updated_at_epoch_seconds);
        updated.validation_issues = validation_issues;
        updated
    }

    pub fn with_correction_response(
        &self,
        raw_correction_response: String,
        recorded_at_epoch_seconds: i64,
    ) -> Self {
        let mut attempts = self.attempts.clone();
        let correction_attempt_number = attempts
            .iter()
            .filter(|attempt| attempt.phase == WorkflowPhase::Correction)
            .count() as u32
            + 1;
        attempts.push(AttemptRecord {
            phase: WorkflowPhase::Correction,
            attempt_number: correction_attempt_number,
            recorded_at_epoch_seconds,
        });

        let mut updated = self.clone_pending_update(recorded_at_epoch_seconds);
        updated.raw_correction_response = Some(raw_correction_response);
        updated.attempts = attempts;
        updated
    }

    pub fn with_projection_update(&self, recorded_at_epoch_seconds: i64) -> Self {
        let mut attempts = self.attempts.clone();
        attempts.push(AttemptRecord {
            phase: WorkflowPhase::ProjectionUpdate,
            attempt_number: attempts
                .iter()
                .filter(|attempt| attempt.phase == WorkflowPhase::ProjectionUpdate)
                .count() as u32
                + 1,
            recorded_at_epoch_seconds,
        });

        let mut updated = self.clone_pending_update(recorded_at_epoch_seconds);
        updated.projection_persisted_at_epoch_seconds = None;
        updated.attempts = attempts;
        updated
    }

    pub fn mark_projection_persisted(&self, recorded_at_epoch_seconds: i64) -> Self {
        let mut updated = self.clone_pending_update(recorded_at_epoch_seconds);
        updated.projection_persisted_at_epoch_seconds = Some(recorded_at_epoch_seconds);
        updated
    }

    pub fn mark_completed(&self, updated_at_epoch_seconds: i64) -> Self {
        Self {
            operation_key: self.operation_key.clone(),
            user_id: self.user_id.clone(),
            workout_id: self.workout_id.clone(),
            saved_at_epoch_seconds: self.saved_at_epoch_seconds,
            status: WorkflowStatus::Completed,
            workout_recap_text: self.workout_recap_text.clone(),
            workout_recap_provider: self.workout_recap_provider.clone(),
            workout_recap_model: self.workout_recap_model.clone(),
            workout_recap_generated_at_epoch_seconds: self.workout_recap_generated_at_epoch_seconds,
            projection_persisted_at_epoch_seconds: self.projection_persisted_at_epoch_seconds,
            raw_plan_response: self.raw_plan_response.clone(),
            raw_correction_response: self.raw_correction_response.clone(),
            validation_issues: self.validation_issues.clone(),
            attempts: self.attempts.clone(),
            failure: None,
            started_at_epoch_seconds: self.started_at_epoch_seconds,
            last_attempt_at_epoch_seconds: self.last_attempt_at_epoch_seconds,
            attempt_count: self.attempt_count,
            created_at_epoch_seconds: self.created_at_epoch_seconds,
            updated_at_epoch_seconds,
        }
    }

    pub fn mark_failed(
        &self,
        phase: WorkflowPhase,
        message: String,
        validation_issues: Vec<ValidationIssue>,
        updated_at_epoch_seconds: i64,
    ) -> Self {
        Self {
            operation_key: self.operation_key.clone(),
            user_id: self.user_id.clone(),
            workout_id: self.workout_id.clone(),
            saved_at_epoch_seconds: self.saved_at_epoch_seconds,
            status: WorkflowStatus::Failed,
            workout_recap_text: self.workout_recap_text.clone(),
            workout_recap_provider: self.workout_recap_provider.clone(),
            workout_recap_model: self.workout_recap_model.clone(),
            workout_recap_generated_at_epoch_seconds: self.workout_recap_generated_at_epoch_seconds,
            projection_persisted_at_epoch_seconds: self.projection_persisted_at_epoch_seconds,
            raw_plan_response: self.raw_plan_response.clone(),
            raw_correction_response: self.raw_correction_response.clone(),
            validation_issues,
            attempts: self.attempts.clone(),
            failure: Some(TrainingPlanFailureState { phase, message }),
            started_at_epoch_seconds: self.started_at_epoch_seconds,
            last_attempt_at_epoch_seconds: self.last_attempt_at_epoch_seconds,
            attempt_count: self.attempt_count,
            created_at_epoch_seconds: self.created_at_epoch_seconds,
            updated_at_epoch_seconds,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrainingPlanGenerationClaimResult {
    Claimed(TrainingPlanGenerationOperation),
    Existing(TrainingPlanGenerationOperation),
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeneratedTrainingPlan {
    pub snapshot: TrainingPlanSnapshot,
    pub active_projected_days: Vec<TrainingPlanProjectedDay>,
    pub was_generated: bool,
}
