use std::{future::Future, pin::Pin};

use super::{CreateEvent, DateRange, Event, IntervalsCredentials, IntervalsError, UpdateEvent};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait IntervalsApiPort: Clone + Send + Sync + 'static {
    fn list_events(
        &self,
        credentials: &IntervalsCredentials,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Event>, IntervalsError>>;

    fn get_event(
        &self,
        credentials: &IntervalsCredentials,
        event_id: i64,
    ) -> BoxFuture<Result<Event, IntervalsError>>;

    fn create_event(
        &self,
        credentials: &IntervalsCredentials,
        event: CreateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>>;

    fn update_event(
        &self,
        credentials: &IntervalsCredentials,
        event_id: i64,
        event: UpdateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>>;

    fn delete_event(
        &self,
        credentials: &IntervalsCredentials,
        event_id: i64,
    ) -> BoxFuture<Result<(), IntervalsError>>;

    fn download_fit(
        &self,
        credentials: &IntervalsCredentials,
        event_id: i64,
    ) -> BoxFuture<Result<Vec<u8>, IntervalsError>>;
}

pub trait IntervalsSettingsPort: Clone + Send + Sync + 'static {
    fn get_credentials(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<IntervalsCredentials, IntervalsError>>;
}
