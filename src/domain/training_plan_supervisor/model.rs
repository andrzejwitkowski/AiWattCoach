use crate::domain::training_plan::TrainingPlanError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrainingPlanSupervisorStatus {
    Pending,
    Accepted,
    Replaced,
    Failed,
}

impl TrainingPlanSupervisorStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Replaced => "replaced",
            Self::Failed => "failed",
        }
    }
}

impl TryFrom<&str> for TrainingPlanSupervisorStatus {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "pending" => Ok(Self::Pending),
            "accepted" => Ok(Self::Accepted),
            "replaced" => Ok(Self::Replaced),
            "failed" => Ok(Self::Failed),
            other => Err(format!("unknown training plan supervisor status: {other}")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrainingPlanSupervisorDecision {
    Accept,
    Replace,
    Fail,
}

impl TrainingPlanSupervisorDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accept => "accept",
            Self::Replace => "replace",
            Self::Fail => "fail",
        }
    }

    pub fn terminal_status(self) -> TrainingPlanSupervisorStatus {
        match self {
            Self::Accept => TrainingPlanSupervisorStatus::Accepted,
            Self::Replace => TrainingPlanSupervisorStatus::Replaced,
            Self::Fail => TrainingPlanSupervisorStatus::Failed,
        }
    }
}

impl TryFrom<&str> for TrainingPlanSupervisorDecision {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "accept" => Ok(Self::Accept),
            "replace" => Ok(Self::Replace),
            "fail" => Ok(Self::Fail),
            other => Err(format!(
                "unknown training plan supervisor decision: {other}"
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrainingPlanSupervisorReview {
    pub decision: TrainingPlanSupervisorDecision,
    pub reason: String,
    pub plan: Option<String>,
}

impl TrainingPlanSupervisorReview {
    pub fn validate(&self) -> Result<(), TrainingPlanError> {
        if self.reason.trim().is_empty() {
            return Err(TrainingPlanError::Validation(
                "training plan supervisor review reason must not be empty".to_string(),
            ));
        }

        if matches!(self.decision, TrainingPlanSupervisorDecision::Replace)
            && self
                .plan
                .as_ref()
                .map(|plan| plan.trim())
                .unwrap_or("")
                .is_empty()
        {
            return Err(TrainingPlanError::Validation(
                "training plan supervisor replacement review must include a replacement plan"
                    .to_string(),
            ));
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrainingPlanSupervisorReplacementApplyResult {
    pub applied_dates: Vec<String>,
    pub skipped_dates: Vec<String>,
    pub skipped_synced_dates: Vec<String>,
    pub applied_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeminiSupervisorWebhookOutcome {
    Ignored,
    Accepted(Box<TrainingPlanSupervisorOperation>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrainingPlanSupervisorOperation {
    pub worker_operation_key: String,
    pub user_id: String,
    pub worker_saved_at_epoch_seconds: i64,
    pub model: String,
    pub batch_name: Option<String>,
    pub batch_submitted_at_epoch_seconds: Option<i64>,
    pub status: TrainingPlanSupervisorStatus,
    pub review: Option<TrainingPlanSupervisorReview>,
    pub replacement_apply_result: Option<TrainingPlanSupervisorReplacementApplyResult>,
    pub created_at_epoch_seconds: i64,
    pub updated_at_epoch_seconds: i64,
}

impl TrainingPlanSupervisorOperation {
    pub fn pending(
        worker_operation_key: String,
        user_id: String,
        worker_saved_at_epoch_seconds: i64,
        model: String,
        now_epoch_seconds: i64,
    ) -> Self {
        Self {
            worker_operation_key,
            user_id,
            worker_saved_at_epoch_seconds,
            model,
            batch_name: None,
            batch_submitted_at_epoch_seconds: None,
            status: TrainingPlanSupervisorStatus::Pending,
            review: None,
            replacement_apply_result: None,
            created_at_epoch_seconds: now_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
        }
    }

    pub fn with_batch_submission(&self, batch_name: String, now_epoch_seconds: i64) -> Self {
        Self {
            worker_operation_key: self.worker_operation_key.clone(),
            user_id: self.user_id.clone(),
            worker_saved_at_epoch_seconds: self.worker_saved_at_epoch_seconds,
            model: self.model.clone(),
            batch_name: Some(batch_name),
            batch_submitted_at_epoch_seconds: Some(now_epoch_seconds),
            status: self.status,
            review: self.review.clone(),
            replacement_apply_result: self.replacement_apply_result.clone(),
            created_at_epoch_seconds: self.created_at_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
        }
    }

    pub fn complete_review(
        &self,
        review: TrainingPlanSupervisorReview,
        now_epoch_seconds: i64,
    ) -> Result<Self, TrainingPlanError> {
        review.validate()?;
        let status = review.decision.terminal_status();

        if self.status == status && self.review.as_ref() == Some(&review) {
            return Ok(self.clone());
        }

        if self.status != TrainingPlanSupervisorStatus::Pending {
            return Err(TrainingPlanError::Validation(format!(
                "training plan supervisor review already completed with status {}",
                self.status.as_str()
            )));
        }

        Ok(Self {
            worker_operation_key: self.worker_operation_key.clone(),
            user_id: self.user_id.clone(),
            worker_saved_at_epoch_seconds: self.worker_saved_at_epoch_seconds,
            model: self.model.clone(),
            batch_name: self.batch_name.clone(),
            batch_submitted_at_epoch_seconds: self.batch_submitted_at_epoch_seconds,
            status,
            review: Some(review),
            replacement_apply_result: self.replacement_apply_result.clone(),
            created_at_epoch_seconds: self.created_at_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
        })
    }

    pub fn with_replacement_apply_result(
        &self,
        replacement_apply_result: TrainingPlanSupervisorReplacementApplyResult,
        now_epoch_seconds: i64,
    ) -> Self {
        Self {
            worker_operation_key: self.worker_operation_key.clone(),
            user_id: self.user_id.clone(),
            worker_saved_at_epoch_seconds: self.worker_saved_at_epoch_seconds,
            model: self.model.clone(),
            batch_name: self.batch_name.clone(),
            batch_submitted_at_epoch_seconds: self.batch_submitted_at_epoch_seconds,
            status: self.status,
            review: self.review.clone(),
            replacement_apply_result: Some(replacement_apply_result),
            created_at_epoch_seconds: self.created_at_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
        }
    }
}
