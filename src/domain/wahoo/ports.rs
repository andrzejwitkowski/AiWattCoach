use std::{future::Future, pin::Pin};

use super::{
    WahooConnectState, WahooError, WahooToken, WahooWorkout, WahooWorkoutList, WahooWorkoutSummary,
};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait WahooConnectStateRepository: Clone + Send + Sync + 'static {
    fn create(&self, state: WahooConnectState) -> BoxFuture<Result<WahooConnectState, WahooError>>;

    fn consume(
        &self,
        state_id: &str,
        user_id: &str,
    ) -> BoxFuture<Result<Option<WahooConnectState>, WahooError>>;
}

pub trait WahooOAuthPort: Clone + Send + Sync + 'static {
    fn build_authorize_url(&self, state: &str) -> Result<String, WahooError>;

    fn exchange_code(&self, code: &str) -> BoxFuture<Result<WahooToken, WahooError>>;

    fn refresh_token(&self, refresh_token: &str) -> BoxFuture<Result<WahooToken, WahooError>>;
}

pub trait WahooApiPort: Clone + Send + Sync + 'static {
    fn list_workouts(
        &self,
        access_token: &str,
        page: usize,
        per_page: usize,
    ) -> BoxFuture<Result<WahooWorkoutList, WahooError>>;

    fn get_workout(
        &self,
        access_token: &str,
        workout_id: i64,
    ) -> BoxFuture<Result<WahooWorkout, WahooError>>;

    fn get_workout_summary(
        &self,
        access_token: &str,
        workout_id: i64,
    ) -> BoxFuture<Result<Option<WahooWorkoutSummary>, WahooError>>;

    fn download_workout_file(&self, file_url: &str) -> BoxFuture<Result<Vec<u8>, WahooError>>;
}
