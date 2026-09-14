use std::sync::{Arc, Mutex};

use aiwattcoach::domain::{
    llm::{LlmProvider, LlmProviderConfig},
    training_plan::{
        plan_quality_attempt_message, plan_quality_finished_accepted_message,
        plan_quality_finished_best_message, PlanQualityEvaluation,
        PlanQualityEvaluatorLlmConfigPort, PlanQualityProgressPort, TrainingPlanError,
    },
};

use super::support::*;

#[derive(Clone)]
struct FixedPlanQualityConfig {
    max_loops: u32,
    pass_score: u8,
}

impl PlanQualityEvaluatorLlmConfigPort for FixedPlanQualityConfig {
    fn get_plan_quality_evaluator_config(
        &self,
        _user_id: &str,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<Result<LlmProviderConfig, TrainingPlanError>>
    {
        Box::pin(async {
            Ok(LlmProviderConfig {
                provider: LlmProvider::OpenRouter,
                model: "test-evaluator".to_string(),
                api_key: "test-key".to_string(),
                base_url: None,
            })
        })
    }

    fn get_plan_quality_max_loops(
        &self,
        _user_id: &str,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<Result<u32, TrainingPlanError>> {
        let max_loops = self.max_loops;
        Box::pin(async move { Ok(max_loops) })
    }

    fn get_plan_quality_pass_score(
        &self,
        _user_id: &str,
    ) -> aiwattcoach::domain::training_plan::BoxFuture<Result<u8, TrainingPlanError>> {
        let pass_score = self.pass_score;
        Box::pin(async move { Ok(pass_score) })
    }
}

#[derive(Clone, Default)]
struct RecordingPlanQualityProgress {
    messages: Arc<Mutex<Vec<String>>>,
}

impl PlanQualityProgressPort for RecordingPlanQualityProgress {
    fn on_quality_progress(&self, _user_id: &str, _workout_id: &str, message: String) {
        self.messages.lock().unwrap().push(message);
    }
}

impl RecordingPlanQualityProgress {
    fn messages(&self) -> Vec<String> {
        self.messages.lock().unwrap().clone()
    }
}

#[tokio::test]
async fn quality_loop_replans_until_score_passes_and_records_progress() {
    let call_log = new_call_log();
    let second_plan = valid_plan_window("2026-04-20");
    let built = build_service(
        call_log.clone(),
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(FIRST_DAY)), Ok(second_plan.clone())],
        vec![],
        FIRST_DAY,
    );
    built.generator.set_quality_evaluations(vec![
        Ok(PlanQualityEvaluation {
            attempt: 0,
            score: 5,
            critique: "Too much tempo.".to_string(),
        }),
        Ok(PlanQualityEvaluation {
            attempt: 0,
            score: 8,
            critique: "Polarized and race-aware.".to_string(),
        }),
    ]);
    let progress = RecordingPlanQualityProgress::default();
    let service = built
        .service
        .with_plan_quality_evaluator_config(Arc::new(FixedPlanQualityConfig {
            max_loops: 5,
            pass_score: 7,
        }))
        .with_plan_quality_progress(Arc::new(progress.clone()));

    let result = service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    assert_eq!(built.generator.initial_plan_call_count(), 2);
    assert!(built.generator.quality_feedbacks()[1]
        .as_ref()
        .is_some_and(|feedback| feedback.contains("Too much tempo.")));
    assert_eq!(
        result.quality_progress_messages,
        vec![
            plan_quality_attempt_message(1, 5, 5, "Too much tempo."),
            plan_quality_attempt_message(2, 5, 8, "Polarized and race-aware."),
            plan_quality_finished_accepted_message(8),
        ]
    );
    assert_eq!(progress.messages(), result.quality_progress_messages);
    assert_eq!(result.shipped_quality.as_ref().map(|e| e.score), Some(8));
    assert_eq!(result.quality_evaluations.len(), 2);
    assert_eq!(result.snapshot.start_date, "2026-04-20");
}

#[tokio::test]
async fn quality_loop_exhaustion_ships_highest_scoring_draft() {
    let call_log = new_call_log();
    let second_plan = valid_plan_window("2026-04-20");
    let built = build_service(
        call_log.clone(),
        vec![Ok(workout_recap())],
        vec![Ok(valid_plan_window(FIRST_DAY)), Ok(second_plan.clone())],
        vec![],
        FIRST_DAY,
    );
    built.generator.set_quality_evaluations(vec![
        Ok(PlanQualityEvaluation {
            attempt: 0,
            score: 4,
            critique: "Almost all sweet spot.".to_string(),
        }),
        Ok(PlanQualityEvaluation {
            attempt: 0,
            score: 6,
            critique: "Still too much middle intensity.".to_string(),
        }),
    ]);
    let progress = RecordingPlanQualityProgress::default();
    let service = built
        .service
        .with_plan_quality_evaluator_config(Arc::new(FixedPlanQualityConfig {
            max_loops: 2,
            pass_score: 7,
        }))
        .with_plan_quality_progress(Arc::new(progress.clone()));

    let result = service
        .generate_for_saved_workout(USER_ID, WORKOUT_ID, date_epoch(FIRST_DAY))
        .await
        .unwrap();

    assert_eq!(built.generator.initial_plan_call_count(), 2);
    assert_eq!(result.shipped_quality.as_ref().map(|e| e.score), Some(6));
    assert_eq!(result.snapshot.start_date, "2026-04-20");
    assert_eq!(
        result.quality_progress_messages,
        vec![
            plan_quality_attempt_message(1, 2, 4, "Almost all sweet spot."),
            plan_quality_attempt_message(2, 2, 6, "Still too much middle intensity."),
            plan_quality_finished_best_message(6, 2),
        ]
    );
    assert_eq!(progress.messages(), result.quality_progress_messages);
}
