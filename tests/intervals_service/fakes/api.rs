use std::sync::{Arc, Mutex};

use aiwattcoach::domain::intervals::{
    Activity, CreateEvent, DateRange, Event, IntervalsApiPort, IntervalsCredentials,
    IntervalsError, UpdateActivity, UpdateEvent, UploadActivity, UploadedActivities,
};

use crate::common::BoxFuture;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ApiCall {
    Create(CreateEvent),
    Update {
        event_id: i64,
        event: UpdateEvent,
    },
    Delete(i64),
    UploadActivity(UploadActivity),
    UpdateActivity {
        activity_id: String,
        activity: UpdateActivity,
    },
}

#[derive(Clone)]
pub(crate) struct FakeIntervalsApi {
    list_events_result: Result<Vec<Event>, IntervalsError>,
    get_event_result: Result<Event, IntervalsError>,
    create_event_result: Result<Event, IntervalsError>,
    update_event_result: Result<Event, IntervalsError>,
    delete_event_result: Result<(), IntervalsError>,
    fit_result: Result<Vec<u8>, IntervalsError>,
    list_activities_result: Result<Vec<Activity>, IntervalsError>,
    get_activity_result: Result<Activity, IntervalsError>,
    upload_activity_result: Result<UploadedActivities, IntervalsError>,
    update_activity_result: Result<Activity, IntervalsError>,
    delete_activity_result: Result<(), IntervalsError>,
    pub(crate) call_log: Arc<Mutex<Vec<ApiCall>>>,
    sequence: Option<Arc<Mutex<Vec<String>>>>,
}

impl Default for FakeIntervalsApi {
    fn default() -> Self {
        Self {
            list_events_result: Ok(Vec::new()),
            get_event_result: Err(IntervalsError::NotFound),
            create_event_result: Err(IntervalsError::NotFound),
            update_event_result: Err(IntervalsError::NotFound),
            delete_event_result: Ok(()),
            fit_result: Ok(Vec::new()),
            list_activities_result: Ok(Vec::new()),
            get_activity_result: Err(IntervalsError::NotFound),
            upload_activity_result: Err(IntervalsError::NotFound),
            update_activity_result: Err(IntervalsError::NotFound),
            delete_activity_result: Ok(()),
            call_log: Arc::new(Mutex::new(Vec::new())),
            sequence: None,
        }
    }
}

impl FakeIntervalsApi {
    pub(crate) fn with_events(events: Vec<Event>) -> Self {
        Self {
            list_events_result: Ok(events),
            ..Self::default()
        }
    }

    pub(crate) fn with_get_event(event: Event) -> Self {
        Self {
            get_event_result: Ok(event),
            ..Self::default()
        }
    }

    pub(crate) fn with_created_event(event: Event) -> Self {
        Self {
            create_event_result: Ok(event),
            ..Self::default()
        }
    }

    pub(crate) fn with_updated_event(event: Event) -> Self {
        Self {
            update_event_result: Ok(event),
            ..Self::default()
        }
    }

    pub(crate) fn with_fit_bytes(bytes: Vec<u8>) -> Self {
        Self {
            fit_result: Ok(bytes),
            ..Self::default()
        }
    }

    pub(crate) fn with_activities(activities: Vec<Activity>) -> Self {
        Self {
            list_activities_result: Ok(activities),
            ..Self::default()
        }
    }

    pub(crate) fn with_get_activity(activity: Activity) -> Self {
        Self {
            get_activity_result: Ok(activity),
            ..Self::default()
        }
    }

    pub(crate) fn with_get_event_and_activities_error(event: Event, error: IntervalsError) -> Self {
        Self {
            get_event_result: Ok(event),
            list_activities_result: Err(error),
            ..Self::default()
        }
    }

    pub(crate) fn with_uploaded_activities(result: UploadedActivities) -> Self {
        Self {
            upload_activity_result: Ok(result),
            ..Self::default()
        }
    }

    pub(crate) fn with_updated_activity(activity: Activity) -> Self {
        Self {
            update_activity_result: Ok(activity),
            ..Self::default()
        }
    }

    pub(crate) fn with_sequence(mut self, sequence: Arc<Mutex<Vec<String>>>) -> Self {
        self.sequence = Some(sequence);
        self
    }

    pub(crate) fn with_error(error: IntervalsError) -> Self {
        Self {
            list_events_result: Err(error.clone()),
            get_event_result: Err(error.clone()),
            create_event_result: Err(error.clone()),
            update_event_result: Err(error.clone()),
            delete_event_result: Err(error.clone()),
            fit_result: Err(error.clone()),
            list_activities_result: Err(error.clone()),
            get_activity_result: Err(error.clone()),
            upload_activity_result: Err(error.clone()),
            update_activity_result: Err(error.clone()),
            delete_activity_result: Err(error),
            ..Self::default()
        }
    }
}

impl IntervalsApiPort for FakeIntervalsApi {
    fn list_events(
        &self,
        _credentials: &IntervalsCredentials,
        _range: &DateRange,
    ) -> BoxFuture<Result<Vec<Event>, IntervalsError>> {
        let result = self.list_events_result.clone();
        Box::pin(async move { result })
    }

    fn get_event(
        &self,
        _credentials: &IntervalsCredentials,
        _event_id: i64,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        let result = self.get_event_result.clone();
        Box::pin(async move { result })
    }

    fn create_event(
        &self,
        _credentials: &IntervalsCredentials,
        event: CreateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        self.call_log.lock().unwrap().push(ApiCall::Create(event));
        let result = self.create_event_result.clone();
        Box::pin(async move { result })
    }

    fn update_event(
        &self,
        _credentials: &IntervalsCredentials,
        event_id: i64,
        event: UpdateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        self.call_log
            .lock()
            .unwrap()
            .push(ApiCall::Update { event_id, event });
        let result = self.update_event_result.clone();
        Box::pin(async move { result })
    }

    fn delete_event(
        &self,
        _credentials: &IntervalsCredentials,
        event_id: i64,
    ) -> BoxFuture<Result<(), IntervalsError>> {
        self.call_log
            .lock()
            .unwrap()
            .push(ApiCall::Delete(event_id));
        let result = self.delete_event_result.clone();
        Box::pin(async move { result })
    }

    fn download_fit(
        &self,
        _credentials: &IntervalsCredentials,
        _event_id: i64,
    ) -> BoxFuture<Result<Vec<u8>, IntervalsError>> {
        let result = self.fit_result.clone();
        Box::pin(async move { result })
    }

    fn list_activities(
        &self,
        _credentials: &IntervalsCredentials,
        _range: &DateRange,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let result = self.list_activities_result.clone();
        Box::pin(async move { result })
    }

    fn get_activity(
        &self,
        _credentials: &IntervalsCredentials,
        _activity_id: &str,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        let result = self.get_activity_result.clone();
        Box::pin(async move { result })
    }

    fn upload_activity(
        &self,
        _credentials: &IntervalsCredentials,
        upload: UploadActivity,
    ) -> BoxFuture<Result<UploadedActivities, IntervalsError>> {
        self.call_log
            .lock()
            .unwrap()
            .push(ApiCall::UploadActivity(upload));
        let result = self.upload_activity_result.clone();
        Box::pin(async move { result })
    }

    fn update_activity(
        &self,
        _credentials: &IntervalsCredentials,
        activity_id: &str,
        activity: UpdateActivity,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        self.call_log.lock().unwrap().push(ApiCall::UpdateActivity {
            activity_id: activity_id.to_string(),
            activity,
        });
        let result = self.update_activity_result.clone();
        Box::pin(async move { result })
    }

    fn delete_activity(
        &self,
        _credentials: &IntervalsCredentials,
        activity_id: &str,
    ) -> BoxFuture<Result<(), IntervalsError>> {
        if let Some(sequence) = self.sequence.as_ref() {
            sequence
                .lock()
                .unwrap()
                .push(format!("api_delete:{activity_id}"));
        }
        let result = self.delete_activity_result.clone();
        Box::pin(async move { result })
    }
}
