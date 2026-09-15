use serde::{Deserialize, Serialize};

use crate::domain::{
    ai_workflow::{AttemptRecord, ValidationIssue, WorkflowPhase, WorkflowStatus},
    intervals::PlannedWorkout,
    llm_tools::LlmToolLoopState,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrainingPlanError {
    Unavailable(String),
    Repository(String),
    Validation(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrainingPlanConversationRole {
    Coach,
    User,
}

impl TrainingPlanConversationRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Coach => "coach",
            Self::User => "user",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrainingPlanConversationMessage {
    pub role: TrainingPlanConversationRole,
    pub content: String,
    pub created_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TrainingPlanPlanningContext {
    pub rpe: Option<u8>,
    pub messages: Vec<TrainingPlanConversationMessage>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrainingPlanPhaseOutput {
    pub raw_response: String,
    pub description: Option<String>,
    pub tool_loop_state: LlmToolLoopState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanQualityEvaluation {
    pub attempt: u32,
    pub score: u8,
    pub critique: String,
    #[serde(default)]
    pub raise_to_next: String,
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
    pub rest_day_reason: Option<String>,
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
    pub rest_day_reason: Option<String>,
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
    pub raw_plan_description: Option<String>,
    pub initial_plan_tool_loop_state: Option<LlmToolLoopState>,
    pub raw_correction_response: Option<String>,
    pub raw_correction_description: Option<String>,
    pub correction_tool_loop_state: Option<LlmToolLoopState>,
    pub validation_issues: Vec<ValidationIssue>,
    pub quality_evaluations: Vec<PlanQualityEvaluation>,
    pub best_quality_evaluation: Option<PlanQualityEvaluation>,
    pub best_quality_plan_response: Option<String>,
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
            raw_plan_description: None,
            initial_plan_tool_loop_state: None,
            raw_correction_response: None,
            raw_correction_description: None,
            correction_tool_loop_state: None,
            validation_issues: Vec::new(),
            quality_evaluations: Vec::new(),
            best_quality_evaluation: None,
            best_quality_plan_response: None,
            attempts: Vec::new(),
            failure: None,
            started_at_epoch_seconds: now_epoch_seconds,
            last_attempt_at_epoch_seconds: now_epoch_seconds,
            attempt_count: 1,
            created_at_epoch_seconds: now_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
        }
    }

    fn transition(
        &self,
        status: WorkflowStatus,
        failure: Option<TrainingPlanFailureState>,
        updated_at_epoch_seconds: i64,
    ) -> Self {
        let mut updated = self.clone();
        updated.status = status;
        updated.failure = failure;
        updated.updated_at_epoch_seconds = updated_at_epoch_seconds;
        updated
    }

    pub fn reclaim(&self, now_epoch_seconds: i64) -> Self {
        let mut updated = self.clone_pending_update(now_epoch_seconds);
        updated.last_attempt_at_epoch_seconds = now_epoch_seconds;
        updated.attempt_count = self.attempt_count.saturating_add(1);
        updated
    }

    fn clone_pending_update(&self, updated_at_epoch_seconds: i64) -> Self {
        self.transition(WorkflowStatus::Pending, None, updated_at_epoch_seconds)
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
        tool_loop_state: LlmToolLoopState,
        recorded_at_epoch_seconds: i64,
    ) -> Self {
        self.with_raw_plan_payload(
            raw_plan_response,
            None,
            tool_loop_state,
            recorded_at_epoch_seconds,
        )
    }

    pub fn with_raw_plan_payload(
        &self,
        raw_plan_response: String,
        raw_plan_description: Option<String>,
        tool_loop_state: LlmToolLoopState,
        recorded_at_epoch_seconds: i64,
    ) -> Self {
        let mut attempts = self.attempts.clone();
        let initial_generation_attempt_number = attempts
            .iter()
            .filter(|attempt| attempt.phase == WorkflowPhase::InitialGeneration)
            .count() as u32
            + 1;
        attempts.push(AttemptRecord {
            phase: WorkflowPhase::InitialGeneration,
            attempt_number: initial_generation_attempt_number,
            recorded_at_epoch_seconds,
        });

        let mut updated = self.clone_pending_update(recorded_at_epoch_seconds);
        updated.raw_plan_response = Some(raw_plan_response);
        updated.raw_plan_description = raw_plan_description;
        updated.initial_plan_tool_loop_state = Some(tool_loop_state);
        // A new initial draft supersedes any prior correction transcript for this operation.
        updated.raw_correction_response = None;
        updated.raw_correction_description = None;
        updated.correction_tool_loop_state = None;
        updated.attempts = attempts;
        updated
    }

    pub fn with_initial_plan_tool_loop_state(
        &self,
        tool_loop_state: LlmToolLoopState,
        recorded_at_epoch_seconds: i64,
    ) -> Self {
        let mut updated = self.clone_pending_update(recorded_at_epoch_seconds);
        updated.initial_plan_tool_loop_state = Some(tool_loop_state);
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
        tool_loop_state: LlmToolLoopState,
        recorded_at_epoch_seconds: i64,
    ) -> Self {
        self.with_correction_payload(
            raw_correction_response,
            None,
            tool_loop_state,
            recorded_at_epoch_seconds,
        )
    }

    pub fn with_correction_payload(
        &self,
        raw_correction_response: String,
        raw_correction_description: Option<String>,
        tool_loop_state: LlmToolLoopState,
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
        updated.raw_correction_description = raw_correction_description;
        updated.correction_tool_loop_state = Some(tool_loop_state);
        updated.attempts = attempts;
        updated
    }

    pub fn with_correction_tool_loop_state(
        &self,
        tool_loop_state: LlmToolLoopState,
        recorded_at_epoch_seconds: i64,
    ) -> Self {
        let mut updated = self.clone_pending_update(recorded_at_epoch_seconds);
        updated.correction_tool_loop_state = Some(tool_loop_state);
        updated
    }

    pub fn with_quality_evaluation(
        &self,
        evaluation: PlanQualityEvaluation,
        best_plan_response: Option<String>,
        recorded_at_epoch_seconds: i64,
    ) -> Self {
        let mut attempts = self.attempts.clone();
        attempts.push(AttemptRecord {
            phase: WorkflowPhase::QualityEvaluation,
            attempt_number: evaluation.attempt,
            recorded_at_epoch_seconds,
        });
        let mut quality_evaluations = self.quality_evaluations.clone();
        quality_evaluations.push(evaluation.clone());

        let mut updated = self.clone_pending_update(recorded_at_epoch_seconds);
        updated.quality_evaluations = quality_evaluations;
        updated.attempts = attempts;
        if let Some(best_plan_response) = best_plan_response {
            updated.best_quality_evaluation = Some(evaluation);
            updated.best_quality_plan_response = Some(best_plan_response);
        }
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
        self.transition(WorkflowStatus::Completed, None, updated_at_epoch_seconds)
    }

    pub fn mark_failed(
        &self,
        phase: WorkflowPhase,
        message: String,
        validation_issues: Vec<ValidationIssue>,
        updated_at_epoch_seconds: i64,
    ) -> Self {
        let mut updated = self.transition(
            WorkflowStatus::Failed,
            Some(TrainingPlanFailureState { phase, message }),
            updated_at_epoch_seconds,
        );
        updated.validation_issues = validation_issues;
        updated
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
    pub quality_evaluations: Vec<PlanQualityEvaluation>,
    pub shipped_quality: Option<PlanQualityEvaluation>,
    pub quality_progress_messages: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrainingPlanReplacementResult {
    pub snapshot: TrainingPlanSnapshot,
    pub projected_days: Vec<TrainingPlanProjectedDay>,
    pub superseded_date_range: Option<(String, String)>,
}

#[cfg(test)]
mod operation_transition_tests {
    use super::{PlanQualityEvaluation, TrainingPlanGenerationOperation};
    use crate::domain::ai_workflow::{ValidationIssue, WorkflowPhase, WorkflowStatus};

    fn sample_operation() -> TrainingPlanGenerationOperation {
        let mut op = TrainingPlanGenerationOperation::pending(
            "training-plan:u:w:1".to_string(),
            "u".to_string(),
            "w".to_string(),
            100,
            200,
        );
        op.workout_recap_text = Some("recap".to_string());
        op.raw_plan_response = Some("plan".to_string());
        op.quality_evaluations = vec![PlanQualityEvaluation {
            attempt: 1,
            score: 6,
            critique: "tempo heavy".to_string(),
            raise_to_next: "Cut midweek tempo.".to_string(),
        }];
        op.best_quality_evaluation = Some(PlanQualityEvaluation {
            attempt: 1,
            score: 6,
            critique: "tempo heavy".to_string(),
            raise_to_next: "Cut midweek tempo.".to_string(),
        });
        op.best_quality_plan_response = Some("best-plan".to_string());
        op.attempt_count = 3;
        op
    }

    #[test]
    fn reclaim_keeps_payload_clears_failure_bumps_attempt() {
        let mut op = sample_operation();
        op.status = WorkflowStatus::Failed;
        op.failure = Some(super::TrainingPlanFailureState {
            phase: WorkflowPhase::Correction,
            message: "boom".to_string(),
        });

        let reclaimed = op.reclaim(500);
        assert_eq!(reclaimed.status, WorkflowStatus::Pending);
        assert!(reclaimed.failure.is_none());
        assert_eq!(reclaimed.attempt_count, 4);
        assert_eq!(reclaimed.last_attempt_at_epoch_seconds, 500);
        assert_eq!(reclaimed.updated_at_epoch_seconds, 500);
        assert_eq!(reclaimed.workout_recap_text.as_deref(), Some("recap"));
        assert_eq!(reclaimed.raw_plan_response.as_deref(), Some("plan"));
        assert_eq!(
            reclaimed.best_quality_plan_response.as_deref(),
            Some("best-plan")
        );
        assert_eq!(reclaimed.quality_evaluations.len(), 1);
        assert_eq!(reclaimed.created_at_epoch_seconds, 200);
    }

    #[test]
    fn mark_completed_preserves_payload_and_clears_failure() {
        let mut op = sample_operation();
        op.failure = Some(super::TrainingPlanFailureState {
            phase: WorkflowPhase::QualityEvaluation,
            message: "x".to_string(),
        });
        let completed = op.mark_completed(600);
        assert_eq!(completed.status, WorkflowStatus::Completed);
        assert!(completed.failure.is_none());
        assert_eq!(completed.updated_at_epoch_seconds, 600);
        assert_eq!(completed.attempt_count, 3);
        assert_eq!(
            completed.best_quality_plan_response.as_deref(),
            Some("best-plan")
        );
    }

    #[test]
    fn mark_failed_sets_failure_and_validation_issues() {
        let op = sample_operation();
        let failed = op.mark_failed(
            WorkflowPhase::Correction,
            "nope".to_string(),
            vec![ValidationIssue {
                scope: "2026-04-10".to_string(),
                message: "bad".to_string(),
            }],
            700,
        );
        assert_eq!(failed.status, WorkflowStatus::Failed);
        assert_eq!(
            failed.failure.as_ref().map(|f| f.message.as_str()),
            Some("nope")
        );
        assert_eq!(failed.validation_issues.len(), 1);
        assert_eq!(failed.updated_at_epoch_seconds, 700);
        assert_eq!(failed.raw_plan_response.as_deref(), Some("plan"));
    }

    #[test]
    fn with_raw_plan_payload_clears_stale_correction_state() {
        use crate::domain::llm_tools::LlmToolLoopState;

        let mut op = sample_operation();
        op.correction_tool_loop_state = Some(LlmToolLoopState::default());
        op.raw_correction_response = Some("old-correction".to_string());
        op.raw_correction_description = Some("old-desc".to_string());

        let updated = op.with_raw_plan_payload(
            "new-plan".to_string(),
            Some("new-desc".to_string()),
            LlmToolLoopState::default(),
            700,
        );
        assert_eq!(updated.raw_plan_response.as_deref(), Some("new-plan"));
        assert!(updated.initial_plan_tool_loop_state.is_some());
        assert!(updated.correction_tool_loop_state.is_none());
        assert!(updated.raw_correction_response.is_none());
        assert!(updated.raw_correction_description.is_none());
    }
}
