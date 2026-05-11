use std::sync::{Arc, Mutex};

use crate::domain::intervals::{
    BoxFuture as IntervalsBoxFuture, CreateEvent, DateRange, Event, IntervalsError,
    IntervalsUseCases, UpdateEvent,
};

#[derive(Clone, Default)]
pub struct RecordingIntervalsService {
    pub(super) existing_event: Option<Event>,
    pub(super) updated_events: Arc<Mutex<Vec<(i64, UpdateEvent)>>>,
    pub(super) operation_log: Arc<Mutex<Vec<String>>>,
    pub(super) fail_update: bool,
    pub(super) shared_log: Option<Arc<Mutex<Vec<String>>>>,
}

impl RecordingIntervalsService {
    pub fn with_existing_event_and_shared_log(
        existing_event: Event,
        shared_log: Arc<Mutex<Vec<String>>>,
    ) -> Self {
        Self {
            existing_event: Some(existing_event),
            updated_events: Arc::new(Mutex::new(Vec::new())),
            operation_log: Arc::new(Mutex::new(Vec::new())),
            fail_update: false,
            shared_log: Some(shared_log),
        }
    }

    pub fn with_failed_update_and_shared_log(
        existing_event: Event,
        shared_log: Arc<Mutex<Vec<String>>>,
    ) -> Self {
        Self {
            existing_event: Some(existing_event),
            updated_events: Arc::new(Mutex::new(Vec::new())),
            operation_log: Arc::new(Mutex::new(Vec::new())),
            fail_update: true,
            shared_log: Some(shared_log),
        }
    }

    pub fn updated_events(&self) -> Vec<(i64, UpdateEvent)> {
        self.updated_events
            .lock()
            .expect("intervals mutex poisoned")
            .clone()
    }
}

impl IntervalsUseCases for RecordingIntervalsService {
    fn list_events(
        &self,
        _user_id: &str,
        _range: &DateRange,
    ) -> IntervalsBoxFuture<Result<Vec<Event>, IntervalsError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn get_event(
        &self,
        _user_id: &str,
        event_id: i64,
    ) -> IntervalsBoxFuture<Result<Event, IntervalsError>> {
        let existing_event = self.existing_event.clone();
        let operation_log = self.operation_log.clone();
        let shared_log = self.shared_log.clone();
        Box::pin(async move {
            let entry = "intervals.get_event".to_string();
            operation_log
                .lock()
                .expect("intervals mutex poisoned")
                .push(entry.clone());
            if let Some(shared_log) = shared_log {
                shared_log
                    .lock()
                    .expect("shared log mutex poisoned")
                    .push(entry);
            }
            match existing_event {
                Some(event) if event.id == event_id => Ok(event),
                _ => Err(IntervalsError::NotFound),
            }
        })
    }

    fn create_event(
        &self,
        _user_id: &str,
        _event: CreateEvent,
    ) -> IntervalsBoxFuture<Result<Event, IntervalsError>> {
        Box::pin(async { Err(IntervalsError::Internal("unused in test".to_string())) })
    }

    fn update_event(
        &self,
        _user_id: &str,
        event_id: i64,
        event: UpdateEvent,
    ) -> IntervalsBoxFuture<Result<Event, IntervalsError>> {
        let existing_event = self.existing_event.clone();
        let updated_events = self.updated_events.clone();
        let operation_log = self.operation_log.clone();
        let fail_update = self.fail_update;
        let shared_log = self.shared_log.clone();
        Box::pin(async move {
            let entry = "intervals.update_event".to_string();
            operation_log
                .lock()
                .expect("intervals mutex poisoned")
                .push(entry.clone());
            if let Some(shared_log) = shared_log {
                shared_log
                    .lock()
                    .expect("shared log mutex poisoned")
                    .push(entry);
            }
            updated_events
                .lock()
                .expect("intervals mutex poisoned")
                .push((event_id, event.clone()));
            if fail_update {
                return Err(IntervalsError::ConnectionError("boom".to_string()));
            }

            let existing_event = existing_event.ok_or_else(|| {
                IntervalsError::Internal("missing existing event fixture".to_string())
            })?;
            Ok(Event {
                id: event_id,
                start_date_local: event
                    .start_date_local
                    .unwrap_or(existing_event.start_date_local),
                event_type: event.event_type.or(existing_event.event_type),
                name: event.name.or(existing_event.name),
                category: event.category.unwrap_or(existing_event.category),
                description: event.description.or(existing_event.description),
                indoor: event.indoor.unwrap_or(existing_event.indoor),
                color: event.color.or(existing_event.color),
                workout_doc: event.workout_doc.or(existing_event.workout_doc),
            })
        })
    }

    fn delete_event(
        &self,
        _user_id: &str,
        _event_id: i64,
    ) -> IntervalsBoxFuture<Result<(), IntervalsError>> {
        Box::pin(async { Ok(()) })
    }

    fn download_fit(
        &self,
        _user_id: &str,
        _event_id: i64,
    ) -> IntervalsBoxFuture<Result<Vec<u8>, IntervalsError>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}
