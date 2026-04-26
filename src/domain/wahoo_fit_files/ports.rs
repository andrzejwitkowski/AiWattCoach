use std::{future::Future, pin::Pin};

use super::{WahooFitFile, WahooFitFileError};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait WahooFitFileRepository: Clone + Send + Sync + 'static {
    fn find_by_user_id_and_completed_workout_id(
        &self,
        user_id: &str,
        completed_workout_id: &str,
    ) -> BoxFuture<Result<Option<WahooFitFile>, WahooFitFileError>>;

    fn upsert(&self, fit_file: WahooFitFile) -> BoxFuture<Result<WahooFitFile, WahooFitFileError>>;
}
