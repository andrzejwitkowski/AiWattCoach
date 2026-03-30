use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use aiwattcoach::domain::intervals::{
    parse_workout_doc, Activity, CreateEvent, DateRange, EnrichedEvent, Event, IntervalsError,
    IntervalsUseCases, UpdateActivity, UpdateEvent, UploadActivity, UploadedActivities,
};

use crate::{fixtures::sample_activity, test_support::BoxFuture};

#[derive(Clone, Default)]
pub(crate) struct ScopedIntervalsService {
    events_by_user: Arc<Mutex<HashMap<String, Vec<Event>>>>,
    activities_by_user: Arc<Mutex<HashMap<String, Vec<Activity>>>>,
}

impl ScopedIntervalsService {
    pub(crate) fn with_user_events<const N: usize>(entries: [(&str, Vec<Event>); N]) -> Self {
        let events_by_user = entries
            .into_iter()
            .map(|(user_id, events)| (user_id.to_string(), events))
            .collect();

        Self {
            events_by_user: Arc::new(Mutex::new(events_by_user)),
            activities_by_user: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl IntervalsUseCases for ScopedIntervalsService {
    fn list_events(
        &self,
        user_id: &str,
        _range: &DateRange,
    ) -> BoxFuture<Result<Vec<Event>, IntervalsError>> {
        let user_id = user_id.to_string();
        let store = self.events_by_user.clone();
        Box::pin(async move {
            Ok(store
                .lock()
                .unwrap()
                .get(&user_id)
                .cloned()
                .unwrap_or_default())
        })
    }

    fn get_event(&self, user_id: &str, event_id: i64) -> BoxFuture<Result<Event, IntervalsError>> {
        let user_id = user_id.to_string();
        let store = self.events_by_user.clone();
        Box::pin(async move {
            store
                .lock()
                .unwrap()
                .get(&user_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .find(|event| event.id == event_id)
                .ok_or(IntervalsError::NotFound)
        })
    }

    fn get_enriched_event(
        &self,
        user_id: &str,
        event_id: i64,
    ) -> BoxFuture<Result<EnrichedEvent, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let event = service.get_event(&user_id, event_id).await?;
            let parsed_workout = parse_workout_doc(event.workout_doc.as_deref(), None);
            Ok(EnrichedEvent {
                event,
                parsed_workout,
                actual_workout: None,
            })
        })
    }

    fn create_event(
        &self,
        user_id: &str,
        event: CreateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        let user_id = user_id.to_string();
        let store = self.events_by_user.clone();
        Box::pin(async move {
            let mut store = store.lock().unwrap();
            let events = store.entry(user_id).or_default();
            let next_id = events.iter().map(|existing| existing.id).max().unwrap_or(0) + 1;
            let event = Event {
                id: next_id,
                start_date_local: event.start_date_local,
                name: event.name,
                category: event.category,
                description: event.description,
                indoor: event.indoor,
                color: event.color,
                workout_doc: event.workout_doc,
            };
            events.push(event.clone());
            Ok(event)
        })
    }

    fn update_event(
        &self,
        user_id: &str,
        event_id: i64,
        event: UpdateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        let user_id = user_id.to_string();
        let store = self.events_by_user.clone();
        Box::pin(async move {
            let mut store = store.lock().unwrap();
            let events = store.entry(user_id).or_default();
            let existing = events
                .iter_mut()
                .find(|existing| existing.id == event_id)
                .ok_or(IntervalsError::NotFound)?;

            if let Some(category) = event.category {
                existing.category = category;
            }
            if let Some(start_date_local) = event.start_date_local {
                existing.start_date_local = start_date_local;
            }
            if let Some(name) = event.name {
                existing.name = Some(name);
            }
            if let Some(description) = event.description {
                existing.description = Some(description);
            }
            if let Some(indoor) = event.indoor {
                existing.indoor = indoor;
            }
            if let Some(color) = event.color {
                existing.color = Some(color);
            }
            if let Some(workout_doc) = event.workout_doc {
                existing.workout_doc = Some(workout_doc);
            }

            Ok(existing.clone())
        })
    }

    fn delete_event(&self, user_id: &str, event_id: i64) -> BoxFuture<Result<(), IntervalsError>> {
        let user_id = user_id.to_string();
        let store = self.events_by_user.clone();
        Box::pin(async move {
            let mut store = store.lock().unwrap();
            let events = store.entry(user_id).or_default();
            let before = events.len();
            events.retain(|event| event.id != event_id);
            if events.len() == before {
                return Err(IntervalsError::NotFound);
            }
            Ok(())
        })
    }

    fn download_fit(
        &self,
        user_id: &str,
        event_id: i64,
    ) -> BoxFuture<Result<Vec<u8>, IntervalsError>> {
        let user_id = user_id.to_string();
        let store = self.events_by_user.clone();
        Box::pin(async move {
            let has_event = store
                .lock()
                .unwrap()
                .get(&user_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .any(|event| event.id == event_id);

            if !has_event {
                return Err(IntervalsError::NotFound);
            }

            Ok(vec![1, 2, 3])
        })
    }

    fn list_activities(
        &self,
        user_id: &str,
        _range: &DateRange,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let user_id = user_id.to_string();
        let store = self.activities_by_user.clone();
        Box::pin(async move {
            Ok(store
                .lock()
                .unwrap()
                .get(&user_id)
                .cloned()
                .unwrap_or_default())
        })
    }

    fn get_activity(
        &self,
        user_id: &str,
        activity_id: &str,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        let user_id = user_id.to_string();
        let activity_id = activity_id.to_string();
        let store = self.activities_by_user.clone();
        Box::pin(async move {
            store
                .lock()
                .unwrap()
                .get(&user_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .find(|activity| activity.id == activity_id)
                .ok_or(IntervalsError::NotFound)
        })
    }

    fn upload_activity(
        &self,
        user_id: &str,
        upload: UploadActivity,
    ) -> BoxFuture<Result<UploadedActivities, IntervalsError>> {
        let user_id = user_id.to_string();
        let store = self.activities_by_user.clone();
        Box::pin(async move {
            let mut store = store.lock().unwrap();
            let activities = store.entry(user_id).or_default();
            let id = format!("i{}", activities.len() + 1);
            let activity =
                sample_activity(&id, upload.name.as_deref().unwrap_or("Uploaded Activity"));
            activities.push(activity.clone());
            Ok(UploadedActivities {
                created: true,
                activity_ids: vec![id],
                activities: vec![activity],
            })
        })
    }

    fn update_activity(
        &self,
        user_id: &str,
        activity_id: &str,
        update: UpdateActivity,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        let user_id = user_id.to_string();
        let activity_id = activity_id.to_string();
        let store = self.activities_by_user.clone();
        Box::pin(async move {
            let mut store = store.lock().unwrap();
            let activities = store.entry(user_id).or_default();
            let existing = activities
                .iter_mut()
                .find(|activity| activity.id == activity_id)
                .ok_or(IntervalsError::NotFound)?;
            if let Some(name) = update.name {
                existing.name = Some(name);
            }
            if let Some(description) = update.description {
                existing.description = Some(description);
            }
            if let Some(activity_type) = update.activity_type {
                existing.activity_type = Some(activity_type);
            }
            if let Some(trainer) = update.trainer {
                existing.trainer = trainer;
            }
            if let Some(commute) = update.commute {
                existing.commute = commute;
            }
            if let Some(race) = update.race {
                existing.race = race;
            }
            Ok(existing.clone())
        })
    }

    fn delete_activity(
        &self,
        user_id: &str,
        activity_id: &str,
    ) -> BoxFuture<Result<(), IntervalsError>> {
        let user_id = user_id.to_string();
        let activity_id = activity_id.to_string();
        let store = self.activities_by_user.clone();
        Box::pin(async move {
            let mut store = store.lock().unwrap();
            let activities = store.entry(user_id).or_default();
            let before = activities.len();
            activities.retain(|activity| activity.id != activity_id);
            if activities.len() == before {
                return Err(IntervalsError::NotFound);
            }
            Ok(())
        })
    }
}
