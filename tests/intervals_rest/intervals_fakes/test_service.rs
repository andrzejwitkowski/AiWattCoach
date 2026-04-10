use std::sync::{Arc, Mutex};

use aiwattcoach::domain::intervals::{
    find_best_activity_match, parse_workout_doc, Activity, CreateEvent, DateRange, EnrichedEvent,
    Event, IntervalsError, IntervalsUseCases, UpdateActivity, UpdateEvent, UploadActivity,
    UploadedActivities,
};

use crate::test_support::BoxFuture;

#[derive(Clone, Default)]
pub(crate) struct TestIntervalsService {
    events: Arc<Mutex<Vec<Event>>>,
    activities: Arc<Mutex<Vec<Activity>>>,
    detailed_activities: Arc<Mutex<Vec<Activity>>>,
    fit_bytes: Arc<Vec<u8>>,
    error: Option<IntervalsError>,
    uploaded_activities: Option<UploadedActivities>,
}

impl TestIntervalsService {
    pub(crate) fn with_events(events: Vec<Event>) -> Self {
        Self {
            events: Arc::new(Mutex::new(events)),
            activities: Arc::new(Mutex::new(Vec::new())),
            detailed_activities: Arc::new(Mutex::new(Vec::new())),
            fit_bytes: Arc::new(vec![0, 1, 2]),
            error: None,
            uploaded_activities: None,
        }
    }

    pub(crate) fn with_events_and_activities(
        events: Vec<Event>,
        activities: Vec<Activity>,
    ) -> Self {
        Self {
            events: Arc::new(Mutex::new(events)),
            activities: Arc::new(Mutex::new(activities)),
            detailed_activities: Arc::new(Mutex::new(Vec::new())),
            fit_bytes: Arc::new(vec![0, 1, 2]),
            error: None,
            uploaded_activities: None,
        }
    }

    pub(crate) fn with_events_listed_and_detailed_activities(
        events: Vec<Event>,
        listed_activities: Vec<Activity>,
        detailed_activities: Vec<Activity>,
    ) -> Self {
        Self {
            events: Arc::new(Mutex::new(events)),
            activities: Arc::new(Mutex::new(listed_activities)),
            detailed_activities: Arc::new(Mutex::new(detailed_activities)),
            fit_bytes: Arc::new(vec![0, 1, 2]),
            error: None,
            uploaded_activities: None,
        }
    }

    pub(crate) fn with_activities(activities: Vec<Activity>) -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            activities: Arc::new(Mutex::new(activities)),
            detailed_activities: Arc::new(Mutex::new(Vec::new())),
            fit_bytes: Arc::new(vec![0, 1, 2]),
            error: None,
            uploaded_activities: None,
        }
    }

    pub(crate) fn with_error(error: IntervalsError) -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            activities: Arc::new(Mutex::new(Vec::new())),
            detailed_activities: Arc::new(Mutex::new(Vec::new())),
            fit_bytes: Arc::new(vec![0, 1, 2]),
            error: Some(error),
            uploaded_activities: None,
        }
    }

    pub(crate) fn with_fit_bytes(bytes: Vec<u8>) -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            activities: Arc::new(Mutex::new(Vec::new())),
            detailed_activities: Arc::new(Mutex::new(Vec::new())),
            fit_bytes: Arc::new(bytes),
            error: None,
            uploaded_activities: None,
        }
    }

    pub(crate) fn with_uploaded_activities(uploaded_activities: UploadedActivities) -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            activities: Arc::new(Mutex::new(uploaded_activities.activities.clone())),
            detailed_activities: Arc::new(Mutex::new(Vec::new())),
            fit_bytes: Arc::new(vec![0, 1, 2]),
            error: None,
            uploaded_activities: Some(uploaded_activities),
        }
    }
}

impl IntervalsUseCases for TestIntervalsService {
    fn list_events(
        &self,
        _user_id: &str,
        _range: &DateRange,
    ) -> BoxFuture<Result<Vec<Event>, IntervalsError>> {
        let error = self.error.clone();
        let events = self.events.lock().unwrap().clone();
        Box::pin(async move {
            if let Some(error) = error {
                return Err(error);
            }
            Ok(events)
        })
    }

    fn get_event(&self, _user_id: &str, event_id: i64) -> BoxFuture<Result<Event, IntervalsError>> {
        let error = self.error.clone();
        let events = self.events.lock().unwrap().clone();
        Box::pin(async move {
            if let Some(error) = error {
                return Err(error);
            }
            events
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
            let date_key = event.start_date_local.clone();
            let listed_activities = service
                .list_activities(
                    &user_id,
                    &DateRange {
                        oldest: date_key.clone(),
                        newest: date_key,
                    },
                )
                .await?;
            let effective_ftp_watts = listed_activities
                .iter()
                .find_map(|activity| activity.metrics.ftp_watts);
            let parsed_workout =
                parse_workout_doc(event.structured_workout_text(), effective_ftp_watts);

            let mut best_match =
                find_best_activity_match(&parsed_workout, &listed_activities, effective_ftp_watts);

            for listed_activity in listed_activities.iter().take(3) {
                let detailed_activity =
                    match service.get_activity(&user_id, &listed_activity.id).await {
                        Ok(activity) => activity,
                        Err(_) => continue,
                    };

                let candidate = match find_best_activity_match(
                    &parsed_workout,
                    std::slice::from_ref(&detailed_activity),
                    effective_ftp_watts,
                ) {
                    Some(candidate) => candidate,
                    None => continue,
                };

                if best_match.as_ref().is_none_or(|current| {
                    candidate.compliance_score > current.compliance_score
                        || (candidate.compliance_score == current.compliance_score
                            && candidate.power_values.len() > current.power_values.len())
                }) {
                    best_match = Some(candidate);
                }
            }

            Ok(EnrichedEvent {
                event,
                parsed_workout,
                actual_workout: best_match,
            })
        })
    }

    fn create_event(
        &self,
        _user_id: &str,
        event: CreateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        let error = self.error.clone();
        let store = self.events.clone();
        Box::pin(async move {
            if let Some(error) = error {
                return Err(error);
            }
            let event = Event {
                id: 1000,
                start_date_local: event.start_date_local,
                event_type: event.event_type,
                name: event.name,
                category: event.category,
                description: event.description,
                indoor: event.indoor,
                color: event.color,
                workout_doc: event.workout_doc,
            };
            store.lock().unwrap().push(event.clone());
            Ok(event)
        })
    }

    fn update_event(
        &self,
        _user_id: &str,
        event_id: i64,
        event: UpdateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        let error = self.error.clone();
        let store = self.events.clone();
        Box::pin(async move {
            if let Some(error) = error {
                return Err(error);
            }
            let mut events = store.lock().unwrap();
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
            if let Some(event_type) = event.event_type {
                existing.event_type = Some(event_type);
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

    fn delete_event(&self, _user_id: &str, event_id: i64) -> BoxFuture<Result<(), IntervalsError>> {
        let error = self.error.clone();
        let store = self.events.clone();
        Box::pin(async move {
            if let Some(error) = error {
                return Err(error);
            }
            let mut events = store.lock().unwrap();
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
        _user_id: &str,
        _event_id: i64,
    ) -> BoxFuture<Result<Vec<u8>, IntervalsError>> {
        let error = self.error.clone();
        let fit_bytes = self.fit_bytes.as_ref().clone();
        Box::pin(async move {
            if let Some(error) = error {
                return Err(error);
            }
            Ok(fit_bytes)
        })
    }

    fn list_activities(
        &self,
        _user_id: &str,
        _range: &DateRange,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let error = self.error.clone();
        let activities = self.activities.lock().unwrap().clone();
        Box::pin(async move {
            if let Some(error) = error {
                return Err(error);
            }
            Ok(activities)
        })
    }

    fn get_activity(
        &self,
        _user_id: &str,
        activity_id: &str,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        let error = self.error.clone();
        let activities = self.activities.lock().unwrap().clone();
        let detailed_activities = self.detailed_activities.lock().unwrap().clone();
        let activity_id = activity_id.to_string();
        Box::pin(async move {
            if let Some(error) = error {
                return Err(error);
            }
            if let Some(activity) = detailed_activities
                .into_iter()
                .find(|activity| activity.id == activity_id)
            {
                return Ok(activity);
            }
            activities
                .into_iter()
                .find(|activity| activity.id == activity_id)
                .ok_or(IntervalsError::NotFound)
        })
    }

    fn upload_activity(
        &self,
        _user_id: &str,
        _upload: UploadActivity,
    ) -> BoxFuture<Result<UploadedActivities, IntervalsError>> {
        let error = self.error.clone();
        let uploaded = self.uploaded_activities.clone();
        Box::pin(async move {
            if let Some(error) = error {
                return Err(error);
            }
            uploaded.ok_or(IntervalsError::NotFound)
        })
    }

    fn update_activity(
        &self,
        _user_id: &str,
        activity_id: &str,
        update: UpdateActivity,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        let error = self.error.clone();
        let store = self.activities.clone();
        let activity_id = activity_id.to_string();
        Box::pin(async move {
            if let Some(error) = error {
                return Err(error);
            }
            let mut activities = store.lock().unwrap();
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
        _user_id: &str,
        activity_id: &str,
    ) -> BoxFuture<Result<(), IntervalsError>> {
        let error = self.error.clone();
        let store = self.activities.clone();
        let activity_id = activity_id.to_string();
        Box::pin(async move {
            if let Some(error) = error {
                return Err(error);
            }
            let mut activities = store.lock().unwrap();
            let before = activities.len();
            activities.retain(|activity| activity.id != activity_id);
            if activities.len() == before {
                return Err(IntervalsError::NotFound);
            }
            Ok(())
        })
    }
}
