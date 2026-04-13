use std::{future::Future, pin::Pin};

#[cfg(test)]
use std::sync::{Arc, Mutex};

use super::{CalendarEntryView, CalendarEntryViewError};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait CalendarEntryViewRepository: Clone + Send + Sync + 'static {
    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> BoxFuture<Result<Vec<CalendarEntryView>, CalendarEntryViewError>>;

    fn upsert(
        &self,
        entry: CalendarEntryView,
    ) -> BoxFuture<Result<CalendarEntryView, CalendarEntryViewError>>;

    fn replace_all_for_user(
        &self,
        user_id: &str,
        entries: Vec<CalendarEntryView>,
    ) -> BoxFuture<Result<Vec<CalendarEntryView>, CalendarEntryViewError>>;

    fn replace_range_for_user(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
        entries: Vec<CalendarEntryView>,
    ) -> BoxFuture<Result<Vec<CalendarEntryView>, CalendarEntryViewError>>;
}

#[cfg(test)]
#[derive(Clone, Default)]
pub struct InMemoryCalendarEntryViewRepository {
    stored: Arc<Mutex<Vec<CalendarEntryView>>>,
}

#[cfg(test)]
impl CalendarEntryViewRepository for InMemoryCalendarEntryViewRepository {
    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> BoxFuture<Result<Vec<CalendarEntryView>, CalendarEntryViewError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            let stored = stored.lock().expect("calendar view repo mutex poisoned");
            let mut entries = stored
                .iter()
                .filter(|entry| entry.user_id == user_id)
                .filter(|entry| entry.date >= oldest && entry.date <= newest)
                .cloned()
                .collect::<Vec<_>>();
            entries.sort_by(|left, right| {
                left.date
                    .cmp(&right.date)
                    .then_with(|| left.entry_kind.as_str().cmp(right.entry_kind.as_str()))
                    .then_with(|| left.entry_id.cmp(&right.entry_id))
            });
            Ok(entries)
        })
    }

    fn upsert(
        &self,
        entry: CalendarEntryView,
    ) -> BoxFuture<Result<CalendarEntryView, CalendarEntryViewError>> {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().expect("calendar view repo mutex poisoned");
            stored.retain(|existing| {
                !(existing.user_id == entry.user_id && existing.entry_id == entry.entry_id)
            });
            stored.push(entry.clone());
            Ok(entry)
        })
    }

    fn replace_all_for_user(
        &self,
        user_id: &str,
        entries: Vec<CalendarEntryView>,
    ) -> BoxFuture<Result<Vec<CalendarEntryView>, CalendarEntryViewError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut stored = stored.lock().expect("calendar view repo mutex poisoned");
            stored.retain(|existing| existing.user_id != user_id);
            stored.extend(entries.clone());
            Ok(entries)
        })
    }

    fn replace_range_for_user(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
        entries: Vec<CalendarEntryView>,
    ) -> BoxFuture<Result<Vec<CalendarEntryView>, CalendarEntryViewError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        let incoming_entry_ids = entries
            .iter()
            .map(|entry| entry.entry_id.clone())
            .collect::<Vec<_>>();
        Box::pin(async move {
            let mut stored = stored.lock().expect("calendar view repo mutex poisoned");
            stored.retain(|existing| {
                if existing.user_id != user_id {
                    return true;
                }
                let in_target_range = existing.date >= oldest && existing.date <= newest;
                let superseded_by_entry_id = incoming_entry_ids.contains(&existing.entry_id);
                !in_target_range && !superseded_by_entry_id
            });
            stored.extend(entries.clone());
            Ok(entries)
        })
    }
}
