use std::{future::Future, pin::Pin};

#[cfg(test)]
use std::sync::{Arc, Mutex};

use super::{PlannedWorkoutToken, PlannedWorkoutTokenError};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait PlannedWorkoutTokenRepository: Clone + Send + Sync + 'static {
    fn find_by_planned_workout_id(
        &self,
        user_id: &str,
        planned_workout_id: &str,
    ) -> BoxFuture<Result<Option<PlannedWorkoutToken>, PlannedWorkoutTokenError>>;

    fn find_by_match_token(
        &self,
        user_id: &str,
        match_token: &str,
    ) -> BoxFuture<Result<Option<PlannedWorkoutToken>, PlannedWorkoutTokenError>>;

    fn upsert(
        &self,
        token: PlannedWorkoutToken,
    ) -> BoxFuture<Result<PlannedWorkoutToken, PlannedWorkoutTokenError>>;
}

#[derive(Clone, Default)]
pub struct NoopPlannedWorkoutTokenRepository {
    #[cfg(test)]
    stored: Arc<Mutex<Vec<PlannedWorkoutToken>>>,
}

impl PlannedWorkoutTokenRepository for NoopPlannedWorkoutTokenRepository {
    fn find_by_planned_workout_id(
        &self,
        user_id: &str,
        planned_workout_id: &str,
    ) -> BoxFuture<Result<Option<PlannedWorkoutToken>, PlannedWorkoutTokenError>> {
        #[cfg(test)]
        {
            let stored = self.stored.clone();
            let user_id = user_id.to_string();
            let planned_workout_id = planned_workout_id.to_string();
            Box::pin(async move {
                Ok(stored
                    .lock()
                    .expect("planned workout token repo mutex poisoned")
                    .iter()
                    .find(|token| {
                        token.user_id == user_id && token.planned_workout_id == planned_workout_id
                    })
                    .cloned())
            })
        }

        #[cfg(not(test))]
        {
            let _ = (user_id, planned_workout_id);
            Box::pin(async { Ok(None) })
        }
    }

    fn find_by_match_token(
        &self,
        user_id: &str,
        match_token: &str,
    ) -> BoxFuture<Result<Option<PlannedWorkoutToken>, PlannedWorkoutTokenError>> {
        #[cfg(test)]
        {
            let stored = self.stored.clone();
            let user_id = user_id.to_string();
            let match_token = match_token.to_string();
            Box::pin(async move {
                Ok(stored
                    .lock()
                    .expect("planned workout token repo mutex poisoned")
                    .iter()
                    .find(|token| token.user_id == user_id && token.match_token == match_token)
                    .cloned())
            })
        }

        #[cfg(not(test))]
        {
            let _ = (user_id, match_token);
            Box::pin(async { Ok(None) })
        }
    }

    fn upsert(
        &self,
        token: PlannedWorkoutToken,
    ) -> BoxFuture<Result<PlannedWorkoutToken, PlannedWorkoutTokenError>> {
        #[cfg(test)]
        {
            let stored = self.stored.clone();
            Box::pin(async move {
                let mut stored = stored
                    .lock()
                    .expect("planned workout token repo mutex poisoned");
                stored.retain(|existing| {
                    !(existing.user_id == token.user_id
                        && existing.planned_workout_id == token.planned_workout_id)
                });
                stored.push(token.clone());
                Ok(token)
            })
        }

        #[cfg(not(test))]
        {
            Box::pin(async move { Ok(token) })
        }
    }
}
