use std::{future::Future, pin::Pin};

use crate::domain::intervals::DateRange;

use super::{CreateRace, Race, RaceError, UpdateRace};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait RaceRepository: Send + Sync + 'static {
    fn list_by_user_id(&self, user_id: &str) -> BoxFuture<Result<Vec<Race>, RaceError>>;

    fn list_by_user_id_and_range(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Race>, RaceError>>;

    fn find_by_user_id_and_race_id(
        &self,
        user_id: &str,
        race_id: &str,
    ) -> BoxFuture<Result<Option<Race>, RaceError>>;

    fn upsert(&self, race: Race) -> BoxFuture<Result<Race, RaceError>>;

    fn delete(&self, user_id: &str, race_id: &str) -> BoxFuture<Result<(), RaceError>>;
}

pub trait RaceUseCases: Send + Sync {
    fn list_races(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Race>, RaceError>>;

    fn get_race(&self, user_id: &str, race_id: &str) -> BoxFuture<Result<Race, RaceError>>;

    fn create_race(&self, user_id: &str, request: CreateRace)
        -> BoxFuture<Result<Race, RaceError>>;

    fn update_race(
        &self,
        user_id: &str,
        race_id: &str,
        request: UpdateRace,
    ) -> BoxFuture<Result<Race, RaceError>>;

    fn delete_race(&self, user_id: &str, race_id: &str) -> BoxFuture<Result<(), RaceError>>;
}
