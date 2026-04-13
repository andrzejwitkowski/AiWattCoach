use std::{future::Future, pin::Pin};

#[cfg(test)]
use std::sync::{Arc, Mutex};

use super::{SpecialDay, SpecialDayError};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait SpecialDayRepository: Clone + Send + Sync + 'static {
    fn list_by_user_id(&self, user_id: &str)
        -> BoxFuture<Result<Vec<SpecialDay>, SpecialDayError>>;

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> BoxFuture<Result<Vec<SpecialDay>, SpecialDayError>>;

    fn upsert(&self, special_day: SpecialDay) -> BoxFuture<Result<SpecialDay, SpecialDayError>>;
}

#[cfg(test)]
#[derive(Clone, Default)]
pub struct NoopSpecialDayRepository {
    stored: Arc<Mutex<Vec<SpecialDay>>>,
}

#[cfg(test)]
impl SpecialDayRepository for NoopSpecialDayRepository {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Vec<SpecialDay>, SpecialDayError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let stored = stored.lock().expect("special day repo mutex poisoned");
            let mut days = stored
                .iter()
                .filter(|day| day.user_id == user_id)
                .cloned()
                .collect::<Vec<_>>();
            days.sort_by(|left, right| {
                left.date
                    .cmp(&right.date)
                    .then_with(|| left.special_day_id.cmp(&right.special_day_id))
            });
            Ok(days)
        })
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> BoxFuture<Result<Vec<SpecialDay>, SpecialDayError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            let stored = stored.lock().expect("special day repo mutex poisoned");
            let mut days = stored
                .iter()
                .filter(|day| day.user_id == user_id)
                .filter(|day| day.date >= oldest && day.date <= newest)
                .cloned()
                .collect::<Vec<_>>();
            days.sort_by(|left, right| {
                left.date
                    .cmp(&right.date)
                    .then_with(|| left.special_day_id.cmp(&right.special_day_id))
            });
            Ok(days)
        })
    }

    fn upsert(&self, special_day: SpecialDay) -> BoxFuture<Result<SpecialDay, SpecialDayError>> {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().expect("special day repo mutex poisoned");
            stored.retain(|existing| {
                !(existing.user_id == special_day.user_id
                    && existing.special_day_id == special_day.special_day_id)
            });
            stored.push(special_day.clone());
            Ok(special_day)
        })
    }
}
