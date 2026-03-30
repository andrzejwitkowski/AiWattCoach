use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use super::{
    normalize_external_id, Activity, ActivityDeduplicationIdentity, ActivityFallbackIdentity,
    ActivityUploadOperation, ActivityUploadOperationClaimResult, ActualWorkoutMatch, CreateEvent,
    DateRange, Event, IntervalsCredentials, IntervalsError, ParsedWorkoutDoc, UpdateActivity,
    UpdateEvent, UploadActivity, UploadedActivities,
};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

#[derive(Clone, Default)]
pub struct NoopActivityRepository {
    stored: Arc<Mutex<HashMap<String, Vec<Activity>>>>,
}

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

    fn list_activities(
        &self,
        credentials: &IntervalsCredentials,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let _ = (credentials, range);
        Box::pin(async {
            Err(IntervalsError::Internal(
                "activity listing not implemented".to_string(),
            ))
        })
    }

    fn get_activity(
        &self,
        credentials: &IntervalsCredentials,
        activity_id: &str,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        let _ = (credentials, activity_id);
        Box::pin(async {
            Err(IntervalsError::Internal(
                "activity lookup not implemented".to_string(),
            ))
        })
    }

    fn upload_activity(
        &self,
        credentials: &IntervalsCredentials,
        upload: UploadActivity,
    ) -> BoxFuture<Result<UploadedActivities, IntervalsError>> {
        let _ = (credentials, upload);
        Box::pin(async {
            Err(IntervalsError::Internal(
                "activity upload not implemented".to_string(),
            ))
        })
    }

    fn update_activity(
        &self,
        credentials: &IntervalsCredentials,
        activity_id: &str,
        activity: UpdateActivity,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        let _ = (credentials, activity_id, activity);
        Box::pin(async {
            Err(IntervalsError::Internal(
                "activity update not implemented".to_string(),
            ))
        })
    }

    fn delete_activity(
        &self,
        credentials: &IntervalsCredentials,
        activity_id: &str,
    ) -> BoxFuture<Result<(), IntervalsError>> {
        let _ = (credentials, activity_id);
        Box::pin(async {
            Err(IntervalsError::Internal(
                "activity delete not implemented".to_string(),
            ))
        })
    }
}

pub trait ActivityRepositoryPort: Clone + Send + Sync + 'static {
    fn upsert(
        &self,
        user_id: &str,
        activity: Activity,
    ) -> BoxFuture<Result<Activity, IntervalsError>>;

    fn upsert_many(
        &self,
        user_id: &str,
        activities: Vec<Activity>,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>>;

    fn find_by_user_id_and_range(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>>;

    fn find_by_user_id_and_activity_id(
        &self,
        user_id: &str,
        activity_id: &str,
    ) -> BoxFuture<Result<Option<Activity>, IntervalsError>>;

    fn find_by_user_id_and_external_id(
        &self,
        user_id: &str,
        external_id: &str,
    ) -> BoxFuture<Result<Option<Activity>, IntervalsError>>;

    fn find_by_user_id_and_fallback_identity(
        &self,
        user_id: &str,
        identity: &str,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>>;

    fn delete(&self, user_id: &str, activity_id: &str) -> BoxFuture<Result<(), IntervalsError>>;
}

pub trait ActivityFileIdentityExtractorPort: Clone + Send + Sync + 'static {
    fn extract_identity(
        &self,
        upload: &UploadActivity,
    ) -> BoxFuture<Result<Option<ActivityFallbackIdentity>, IntervalsError>>;
}

pub trait ActivityUploadOperationRepositoryPort: Clone + Send + Sync + 'static {
    fn claim_pending(
        &self,
        user_id: &str,
        operation: ActivityUploadOperation,
    ) -> BoxFuture<Result<ActivityUploadOperationClaimResult, IntervalsError>>;

    fn find_by_user_id_and_operation_key(
        &self,
        user_id: &str,
        operation_key: &str,
    ) -> BoxFuture<Result<Option<ActivityUploadOperation>, IntervalsError>>;

    fn upsert(
        &self,
        user_id: &str,
        operation: ActivityUploadOperation,
    ) -> BoxFuture<Result<ActivityUploadOperation, IntervalsError>>;
}

impl ActivityRepositoryPort for NoopActivityRepository {
    fn upsert(
        &self,
        user_id: &str,
        activity: Activity,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut stored = stored.lock().expect("noop activity repo mutex poisoned");
            let activities = stored.entry(user_id).or_default();
            activities.retain(|existing| existing.id != activity.id);
            activities.push(activity.clone());
            Ok(activity)
        })
    }

    fn upsert_many(
        &self,
        user_id: &str,
        activities: Vec<Activity>,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut stored = stored.lock().expect("noop activity repo mutex poisoned");
            let existing = stored.entry(user_id).or_default();
            for activity in &activities {
                existing.retain(|current| current.id != activity.id);
                existing.push(activity.clone());
            }
            Ok(activities)
        })
    }

    fn find_by_user_id_and_range(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let oldest = range.oldest.clone();
        let newest = range.newest.clone();
        Box::pin(async move {
            let stored = stored.lock().expect("noop activity repo mutex poisoned");
            let activities = stored.get(&user_id).cloned().unwrap_or_default();
            Ok(activities
                .into_iter()
                .filter(|activity| activity_date(&activity.start_date_local) >= oldest.as_str())
                .filter(|activity| activity_date(&activity.start_date_local) <= newest.as_str())
                .collect())
        })
    }

    fn find_by_user_id_and_activity_id(
        &self,
        user_id: &str,
        activity_id: &str,
    ) -> BoxFuture<Result<Option<Activity>, IntervalsError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let activity_id = activity_id.to_string();
        Box::pin(async move {
            let stored = stored.lock().expect("noop activity repo mutex poisoned");
            Ok(stored
                .get(&user_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .find(|activity| activity.id == activity_id))
        })
    }

    fn find_by_user_id_and_external_id(
        &self,
        user_id: &str,
        external_id: &str,
    ) -> BoxFuture<Result<Option<Activity>, IntervalsError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let external_id = external_id.to_string();
        Box::pin(async move {
            let stored = stored.lock().expect("noop activity repo mutex poisoned");
            Ok(stored
                .get(&user_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .find(|activity| {
                    normalize_external_id(activity.external_id.as_deref()).as_deref()
                        == Some(external_id.as_str())
                }))
        })
    }

    fn find_by_user_id_and_fallback_identity(
        &self,
        user_id: &str,
        identity: &str,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let identity = identity.to_string();
        Box::pin(async move {
            let stored = stored.lock().expect("noop activity repo mutex poisoned");
            Ok(stored
                .get(&user_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(|activity| {
                    ActivityDeduplicationIdentity::from_activity(activity)
                        .fallback_identity
                        .as_deref()
                        == Some(identity.as_str())
                })
                .collect())
        })
    }

    fn delete(&self, user_id: &str, activity_id: &str) -> BoxFuture<Result<(), IntervalsError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let activity_id = activity_id.to_string();
        Box::pin(async move {
            let mut stored = stored.lock().expect("noop activity repo mutex poisoned");
            if let Some(activities) = stored.get_mut(&user_id) {
                activities.retain(|activity| activity.id != activity_id);
            }
            Ok(())
        })
    }
}

#[derive(Clone, Default)]
pub struct NoopActivityUploadOperationRepository {
    stored: Arc<Mutex<HashMap<String, Vec<ActivityUploadOperation>>>>,
}

impl ActivityUploadOperationRepositoryPort for NoopActivityUploadOperationRepository {
    fn claim_pending(
        &self,
        user_id: &str,
        operation: ActivityUploadOperation,
    ) -> BoxFuture<Result<ActivityUploadOperationClaimResult, IntervalsError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut stored = stored
                .lock()
                .expect("noop upload operation repo mutex poisoned");
            let operations = stored.entry(user_id).or_default();

            if let Some(index) = operations
                .iter()
                .position(|existing| existing.operation_key == operation.operation_key)
            {
                let existing = operations[index].clone();
                if existing.status == super::ActivityUploadOperationStatus::Failed {
                    operations[index] = operation.clone();
                    return Ok(ActivityUploadOperationClaimResult::Claimed(operation));
                }

                return Ok(ActivityUploadOperationClaimResult::Existing(existing));
            }

            operations.push(operation.clone());
            Ok(ActivityUploadOperationClaimResult::Claimed(operation))
        })
    }

    fn find_by_user_id_and_operation_key(
        &self,
        user_id: &str,
        operation_key: &str,
    ) -> BoxFuture<Result<Option<ActivityUploadOperation>, IntervalsError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let operation_key = operation_key.to_string();
        Box::pin(async move {
            let stored = stored
                .lock()
                .expect("noop upload operation repo mutex poisoned");
            Ok(stored
                .get(&user_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .find(|operation| operation.operation_key == operation_key))
        })
    }

    fn upsert(
        &self,
        user_id: &str,
        operation: ActivityUploadOperation,
    ) -> BoxFuture<Result<ActivityUploadOperation, IntervalsError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut stored = stored
                .lock()
                .expect("noop upload operation repo mutex poisoned");
            let operations = stored.entry(user_id).or_default();
            operations.retain(|existing| existing.operation_key != operation.operation_key);
            operations.push(operation.clone());
            Ok(operation)
        })
    }
}

#[derive(Clone, Default)]
pub struct NoopActivityFileIdentityExtractor;

impl ActivityFileIdentityExtractorPort for NoopActivityFileIdentityExtractor {
    fn extract_identity(
        &self,
        _upload: &UploadActivity,
    ) -> BoxFuture<Result<Option<ActivityFallbackIdentity>, IntervalsError>> {
        Box::pin(async { Ok(None) })
    }
}

fn activity_date(start_date_local: &str) -> &str {
    start_date_local.get(..10).unwrap_or(start_date_local)
}

pub trait IntervalsSettingsPort: Clone + Send + Sync + 'static {
    fn get_credentials(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<IntervalsCredentials, IntervalsError>>;

    fn get_cycling_ftp_watts(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Option<i32>, IntervalsError>> {
        let _ = user_id;
        Box::pin(async { Ok(None) })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnrichedEvent {
    pub event: Event,
    pub parsed_workout: ParsedWorkoutDoc,
    pub actual_workout: Option<ActualWorkoutMatch>,
}
