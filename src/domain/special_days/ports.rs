use std::{future::Future, pin::Pin};

use super::SpecialDay;

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait SpecialDayRepository: Clone + Send + Sync + 'static {
    fn upsert(
        &self,
        special_day: SpecialDay,
    ) -> BoxFuture<Result<SpecialDay, std::convert::Infallible>>;
}

#[cfg(test)]
#[derive(Clone, Default)]
pub struct NoopSpecialDayRepository;

#[cfg(test)]
impl SpecialDayRepository for NoopSpecialDayRepository {
    fn upsert(
        &self,
        special_day: SpecialDay,
    ) -> BoxFuture<Result<SpecialDay, std::convert::Infallible>> {
        Box::pin(async move { Ok(special_day) })
    }
}
