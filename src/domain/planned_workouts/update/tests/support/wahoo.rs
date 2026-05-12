use std::sync::{Arc, Mutex};

use crate::domain::wahoo::{
    BoxFuture as WahooBoxFuture, WahooAuthExchange, WahooAuthStart, WahooCreatePlan,
    WahooCreateWorkout, WahooError, WahooPlan, WahooToken, WahooUpdatePlan, WahooUpdateWorkout,
    WahooUseCases, WahooUser, WahooWorkout, WahooWorkoutList, WahooWorkoutSummary,
};

#[derive(Clone)]
pub struct RecordingWahooService {
    fail_message: Option<String>,
    shared_log: Arc<Mutex<Vec<String>>>,
    updated_plans: Arc<Mutex<Vec<(i64, WahooUpdatePlan)>>>,
    updated_workouts: Arc<Mutex<Vec<(i64, WahooUpdateWorkout)>>>,
}

impl RecordingWahooService {
    pub fn successful(shared_log: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            fail_message: None,
            shared_log,
            updated_plans: Arc::new(Mutex::new(Vec::new())),
            updated_workouts: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn failing(message: &str, shared_log: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            fail_message: Some(message.to_string()),
            shared_log,
            updated_plans: Arc::new(Mutex::new(Vec::new())),
            updated_workouts: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn updated_plans(&self) -> Vec<(i64, WahooUpdatePlan)> {
        self.updated_plans
            .lock()
            .expect("wahoo mutex poisoned")
            .clone()
    }

    pub fn updated_workouts(&self) -> Vec<(i64, WahooUpdateWorkout)> {
        self.updated_workouts
            .lock()
            .expect("wahoo mutex poisoned")
            .clone()
    }
}

impl WahooUseCases for RecordingWahooService {
    fn begin_connect(
        &self,
        _user_id: &str,
        _return_to: Option<String>,
    ) -> WahooBoxFuture<Result<WahooAuthStart, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn finish_connect(
        &self,
        _user_id: &str,
        _state: &str,
        _code: &str,
    ) -> WahooBoxFuture<Result<WahooAuthExchange, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn ensure_token(&self, _user_id: &str) -> WahooBoxFuture<Result<WahooToken, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn get_authenticated_user(
        &self,
        _user_id: &str,
    ) -> WahooBoxFuture<Result<WahooUser, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn list_workouts(
        &self,
        _user_id: &str,
        _page: usize,
        _per_page: usize,
    ) -> WahooBoxFuture<Result<WahooWorkoutList, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn get_workout(
        &self,
        _user_id: &str,
        _workout_id: i64,
    ) -> WahooBoxFuture<Result<WahooWorkout, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn get_workout_summary(
        &self,
        _user_id: &str,
        _workout_id: i64,
    ) -> WahooBoxFuture<Result<Option<WahooWorkoutSummary>, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn find_plan_by_external_id(
        &self,
        _user_id: &str,
        _external_id: &str,
    ) -> WahooBoxFuture<Result<Option<WahooPlan>, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn create_plan(
        &self,
        _user_id: &str,
        _request: WahooCreatePlan,
    ) -> WahooBoxFuture<Result<WahooPlan, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn update_plan(
        &self,
        _user_id: &str,
        plan_id: i64,
        request: WahooUpdatePlan,
    ) -> WahooBoxFuture<Result<WahooPlan, WahooError>> {
        let shared_log = self.shared_log.clone();
        let updated_plans = self.updated_plans.clone();
        let fail_message = self.fail_message.clone();
        Box::pin(async move {
            shared_log
                .lock()
                .expect("shared log mutex poisoned")
                .push("wahoo.update_plan".to_string());
            updated_plans
                .lock()
                .expect("wahoo mutex poisoned")
                .push((plan_id, request.clone()));
            if let Some(message) = fail_message {
                return Err(WahooError::External(message));
            }
            Ok(WahooPlan {
                id: plan_id,
                external_id: "training-plan:user-1:w1:2026-05-10".to_string(),
                provider_updated_at: Some(request.provider_updated_at),
                filename: request.filename,
                name: Some("Warmup".to_string()),
                description: None,
                created_at: None,
                updated_at: None,
            })
        })
    }

    fn create_workout(
        &self,
        _user_id: &str,
        _request: WahooCreateWorkout,
    ) -> WahooBoxFuture<Result<WahooWorkout, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }

    fn update_workout(
        &self,
        _user_id: &str,
        workout_id: i64,
        request: WahooUpdateWorkout,
    ) -> WahooBoxFuture<Result<WahooWorkout, WahooError>> {
        let shared_log = self.shared_log.clone();
        let updated_workouts = self.updated_workouts.clone();
        let fail_message = self.fail_message.clone();
        Box::pin(async move {
            shared_log
                .lock()
                .expect("shared log mutex poisoned")
                .push("wahoo.update_workout".to_string());
            updated_workouts
                .lock()
                .expect("wahoo mutex poisoned")
                .push((workout_id, request.clone()));
            if let Some(message) = fail_message {
                return Err(WahooError::External(message));
            }
            Ok(WahooWorkout {
                id: workout_id,
                starts: request
                    .starts
                    .clone()
                    .unwrap_or_else(|| "2026-05-10T00:00:00.000Z".to_string()),
                minutes: request.minutes,
                name: request.name,
                plan_id: request.plan_id,
                plan_ids: request.plan_id.into_iter().collect(),
                route_id: None,
                workout_token: request.workout_token,
                workout_type_id: request.workout_type_id,
                workout_summary: None,
                created_at: None,
                updated_at: None,
            })
        })
    }

    fn download_workout_file(
        &self,
        _file_url: &str,
    ) -> WahooBoxFuture<Result<Vec<u8>, WahooError>> {
        Box::pin(async { Err(WahooError::NotConnected) })
    }
}
