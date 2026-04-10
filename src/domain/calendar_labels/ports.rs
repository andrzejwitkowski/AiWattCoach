use std::{future::Future, pin::Pin};

use crate::domain::intervals::DateRange;

use super::{CalendarLabel, CalendarLabelError, CalendarLabelsResponse};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait CalendarLabelSource: Send + Sync + 'static {
    fn list_labels(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<CalendarLabel>, CalendarLabelError>>;
}

pub trait CalendarLabelsUseCases: Send + Sync {
    fn list_labels(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<CalendarLabelsResponse, CalendarLabelError>>;
}
