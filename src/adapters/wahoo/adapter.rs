macro_rules! delegate {
    ($adapter:expr, $call:ident ( $($arg:expr),* )) => {
        match $adapter {
            WahooOAuthAdapter::Live(client) => client.$call($($arg),*),
            WahooOAuthAdapter::Dev(client) => client.$call($($arg),*),
        }
    };
}

use super::{client::WahooOAuthClient, dev_client::DevWahooOAuthClient};
use crate::domain::wahoo::{
    BoxFuture, WahooApiPort, WahooError, WahooOAuthPort, WahooToken, WahooWorkout,
    WahooWorkoutList, WahooWorkoutSummary,
};

#[derive(Clone)]
pub enum WahooOAuthAdapter {
    Live(WahooOAuthClient),
    Dev(DevWahooOAuthClient),
}

impl WahooOAuthPort for WahooOAuthAdapter {
    fn build_authorize_url(&self, state: &str) -> Result<String, WahooError> {
        delegate!(self, build_authorize_url(state))
    }

    fn exchange_code(&self, code: &str) -> BoxFuture<Result<WahooToken, WahooError>> {
        delegate!(self, exchange_code(code))
    }

    fn refresh_token(&self, refresh_token: &str) -> BoxFuture<Result<WahooToken, WahooError>> {
        delegate!(self, refresh_token(refresh_token))
    }
}

impl WahooApiPort for WahooOAuthAdapter {
    fn list_workouts(
        &self,
        access_token: &str,
        page: usize,
        per_page: usize,
    ) -> BoxFuture<Result<WahooWorkoutList, WahooError>> {
        delegate!(self, list_workouts(access_token, page, per_page))
    }

    fn get_workout(
        &self,
        access_token: &str,
        workout_id: i64,
    ) -> BoxFuture<Result<WahooWorkout, WahooError>> {
        delegate!(self, get_workout(access_token, workout_id))
    }

    fn get_workout_summary(
        &self,
        access_token: &str,
        workout_id: i64,
    ) -> BoxFuture<Result<Option<WahooWorkoutSummary>, WahooError>> {
        delegate!(self, get_workout_summary(access_token, workout_id))
    }

    fn download_workout_file(&self, file_url: &str) -> BoxFuture<Result<Vec<u8>, WahooError>> {
        delegate!(self, download_workout_file(file_url))
    }
}
