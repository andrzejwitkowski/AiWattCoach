use super::{
    build_activity_upload_operation_key, find_best_activity_match, normalize_external_id,
    parse_workout_doc,
    ports::{ActivityFileIdentityExtractorPort, BoxFuture},
    Activity, ActivityRepositoryPort, ActivityUploadOperation,
    ActivityUploadOperationRepositoryPort, ActivityUploadOperationStatus, CreateEvent, DateRange,
    EnrichedEvent, Event, IntervalsApiPort, IntervalsError, IntervalsSettingsPort, UpdateActivity,
    UpdateEvent, UploadActivity, UploadedActivities,
};
use tracing::warn;

#[derive(Clone, Debug, PartialEq)]
pub enum IntervalsConnectionError {
    Unauthenticated,
    InvalidConfiguration,
    Unavailable,
}

impl std::fmt::Display for IntervalsConnectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unauthenticated => write!(f, "Invalid API key or athlete ID"),
            Self::InvalidConfiguration => write!(f, "Invalid configuration"),
            Self::Unavailable => write!(f, "Intervals.icu is currently unavailable"),
        }
    }
}

impl std::error::Error for IntervalsConnectionError {}

pub trait IntervalsConnectionTester: Send + Sync + 'static {
    fn test_connection(
        &self,
        api_key: &str,
        athlete_id: &str,
    ) -> BoxFuture<Result<(), IntervalsConnectionError>>;
}

pub trait IntervalsUseCases: Send + Sync {
    fn list_events(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Event>, IntervalsError>>;

    fn get_event(&self, user_id: &str, event_id: i64) -> BoxFuture<Result<Event, IntervalsError>>;

    fn get_enriched_event(
        &self,
        user_id: &str,
        event_id: i64,
    ) -> BoxFuture<Result<EnrichedEvent, IntervalsError>> {
        let _ = (user_id, event_id);
        Box::pin(async {
            Err(IntervalsError::Internal(
                "enriched event lookup not implemented".to_string(),
            ))
        })
    }

    fn create_event(
        &self,
        user_id: &str,
        event: CreateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>>;

    fn update_event(
        &self,
        user_id: &str,
        event_id: i64,
        event: UpdateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>>;

    fn delete_event(&self, user_id: &str, event_id: i64) -> BoxFuture<Result<(), IntervalsError>>;

    fn download_fit(
        &self,
        user_id: &str,
        event_id: i64,
    ) -> BoxFuture<Result<Vec<u8>, IntervalsError>>;

    fn list_activities(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let _ = (user_id, range);
        Box::pin(async {
            Err(IntervalsError::Internal(
                "activity listing not implemented".to_string(),
            ))
        })
    }

    fn get_activity(
        &self,
        user_id: &str,
        activity_id: &str,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        let _ = (user_id, activity_id);
        Box::pin(async {
            Err(IntervalsError::Internal(
                "activity lookup not implemented".to_string(),
            ))
        })
    }

    fn upload_activity(
        &self,
        user_id: &str,
        upload: UploadActivity,
    ) -> BoxFuture<Result<UploadedActivities, IntervalsError>> {
        let _ = (user_id, upload);
        Box::pin(async {
            Err(IntervalsError::Internal(
                "activity upload not implemented".to_string(),
            ))
        })
    }

    fn update_activity(
        &self,
        user_id: &str,
        activity_id: &str,
        activity: UpdateActivity,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        let _ = (user_id, activity_id, activity);
        Box::pin(async {
            Err(IntervalsError::Internal(
                "activity update not implemented".to_string(),
            ))
        })
    }

    fn delete_activity(
        &self,
        user_id: &str,
        activity_id: &str,
    ) -> BoxFuture<Result<(), IntervalsError>> {
        let _ = (user_id, activity_id);
        Box::pin(async {
            Err(IntervalsError::Internal(
                "activity delete not implemented".to_string(),
            ))
        })
    }
}

#[derive(Clone)]
pub struct IntervalsService<Api, Settings, Activities, UploadOperations, Extractor>
where
    Api: IntervalsApiPort,
    Settings: IntervalsSettingsPort,
    Activities: ActivityRepositoryPort,
    UploadOperations: ActivityUploadOperationRepositoryPort,
    Extractor: ActivityFileIdentityExtractorPort,
{
    api: Api,
    settings: Settings,
    activities: Activities,
    upload_operations: UploadOperations,
    identity_extractor: Extractor,
}

impl<Api, Settings, Activities, UploadOperations, Extractor>
    IntervalsService<Api, Settings, Activities, UploadOperations, Extractor>
where
    Api: IntervalsApiPort,
    Settings: IntervalsSettingsPort,
    Activities: ActivityRepositoryPort,
    UploadOperations: ActivityUploadOperationRepositoryPort,
    Extractor: ActivityFileIdentityExtractorPort,
{
    pub fn new(
        api: Api,
        settings: Settings,
        activities: Activities,
        upload_operations: UploadOperations,
        identity_extractor: Extractor,
    ) -> Self {
        Self {
            api,
            settings,
            activities,
            upload_operations,
            identity_extractor,
        }
    }

    async fn recover_uploaded_operation(
        &self,
        user_id: &str,
        operation: &ActivityUploadOperation,
    ) -> Result<Option<UploadedActivities>, IntervalsError> {
        match operation.status {
            ActivityUploadOperationStatus::Pending => Ok(None),
            ActivityUploadOperationStatus::Uploaded | ActivityUploadOperationStatus::Completed => {
                let mut activities = Vec::new();

                for activity_id in &operation.uploaded_activity_ids {
                    if let Some(activity) = self
                        .activities
                        .find_by_user_id_and_activity_id(user_id, activity_id)
                        .await?
                    {
                        activities.push(activity);
                        continue;
                    }

                    let credentials = self.settings.get_credentials(user_id).await?;
                    let activity = self.api.get_activity(&credentials, activity_id).await?;
                    let stored = self.activities.upsert(user_id, activity).await?;
                    activities.push(stored);
                }

                if activities.len() != operation.uploaded_activity_ids.len() {
                    return Ok(None);
                }

                let completed = if operation.status == ActivityUploadOperationStatus::Completed {
                    operation.clone()
                } else {
                    self.upload_operations
                        .upsert(
                            user_id,
                            operation.mark_completed(
                                activities
                                    .iter()
                                    .map(|activity| activity.id.clone())
                                    .collect(),
                            ),
                        )
                        .await?
                };

                Ok(Some(UploadedActivities {
                    created: false,
                    activity_ids: completed.uploaded_activity_ids,
                    activities,
                }))
            }
        }
    }
}

fn shortlist_activity_candidates(activities: &[Activity], limit: usize) -> Vec<&Activity> {
    let mut candidates = activities.iter().collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        activity_shortlist_score(right)
            .cmp(&activity_shortlist_score(left))
            .then_with(|| right.moving_time_seconds.cmp(&left.moving_time_seconds))
    });
    candidates.truncate(limit);
    candidates
}

fn activity_shortlist_score(activity: &Activity) -> i32 {
    i32::from(!activity.details.intervals.is_empty())
        + i32::from(!activity.details.streams.is_empty())
        + i32::from(activity.metrics.average_power_watts.is_some())
        + i32::from(activity.metrics.normalized_power_watts.is_some())
        + i32::from(
            activity
                .stream_types
                .iter()
                .any(|stream| stream.eq_ignore_ascii_case("watts")),
        )
}

impl<Api, Settings, Activities, UploadOperations, Extractor> IntervalsUseCases
    for IntervalsService<Api, Settings, Activities, UploadOperations, Extractor>
where
    Api: IntervalsApiPort,
    Settings: IntervalsSettingsPort,
    Activities: ActivityRepositoryPort,
    UploadOperations: ActivityUploadOperationRepositoryPort,
    Extractor: ActivityFileIdentityExtractorPort,
{
    fn list_events(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Event>, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move {
            let credentials = service.settings.get_credentials(&user_id).await?;
            service.api.list_events(&credentials, &range).await
        })
    }

    fn get_event(&self, user_id: &str, event_id: i64) -> BoxFuture<Result<Event, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let credentials = service.settings.get_credentials(&user_id).await?;
            service.api.get_event(&credentials, event_id).await
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
            let configured_ftp_watts = service.settings.get_cycling_ftp_watts(&user_id).await?;
            let credentials = service.settings.get_credentials(&user_id).await?;
            let event = service.api.get_event(&credentials, event_id).await?;
            let date_key = event
                .start_date_local
                .split('T')
                .next()
                .unwrap_or(&event.start_date_local)
                .to_string();
            let listed_activities = service
                .api
                .list_activities(
                    &credentials,
                    &DateRange {
                        oldest: date_key.clone(),
                        newest: date_key,
                    },
                )
                .await?;
            let effective_ftp_watts = configured_ftp_watts.or_else(|| {
                listed_activities
                    .iter()
                    .find_map(|activity| activity.metrics.ftp_watts)
            });
            let parsed_workout =
                parse_workout_doc(event.workout_doc.as_deref(), effective_ftp_watts);

            let mut best_match =
                find_best_activity_match(&parsed_workout, &listed_activities, effective_ftp_watts);

            let detailed_candidates = shortlist_activity_candidates(&listed_activities, 3);

            for listed_activity in detailed_candidates {
                let detailed_activity = match service
                    .api
                    .get_activity(&credentials, &listed_activity.id)
                    .await
                {
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
        user_id: &str,
        event: CreateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let credentials = service.settings.get_credentials(&user_id).await?;
            service.api.create_event(&credentials, event).await
        })
    }

    fn update_event(
        &self,
        user_id: &str,
        event_id: i64,
        event: UpdateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let credentials = service.settings.get_credentials(&user_id).await?;
            service
                .api
                .update_event(&credentials, event_id, event)
                .await
        })
    }

    fn delete_event(&self, user_id: &str, event_id: i64) -> BoxFuture<Result<(), IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let credentials = service.settings.get_credentials(&user_id).await?;
            service.api.delete_event(&credentials, event_id).await
        })
    }

    fn download_fit(
        &self,
        user_id: &str,
        event_id: i64,
    ) -> BoxFuture<Result<Vec<u8>, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let credentials = service.settings.get_credentials(&user_id).await?;
            service.api.download_fit(&credentials, event_id).await
        })
    }

    fn list_activities(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move {
            let credentials = service.settings.get_credentials(&user_id).await?;
            let activities = service.api.list_activities(&credentials, &range).await?;
            if let Err(error) = service
                .activities
                .upsert_many(&user_id, activities.clone())
                .await
            {
                warn!(
                    ?error,
                    %user_id,
                    "activity list refresh succeeded but local persistence failed"
                );
            }
            Ok(activities)
        })
    }

    fn get_activity(
        &self,
        user_id: &str,
        activity_id: &str,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let activity_id = activity_id.to_string();
        Box::pin(async move {
            let credentials = service.settings.get_credentials(&user_id).await?;
            let activity = service.api.get_activity(&credentials, &activity_id).await?;
            service
                .activities
                .upsert(&user_id, activity.clone())
                .await?;
            Ok(activity)
        })
    }

    fn upload_activity(
        &self,
        user_id: &str,
        upload: UploadActivity,
    ) -> BoxFuture<Result<UploadedActivities, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let upload = UploadActivity {
                external_id: normalize_external_id(upload.external_id.as_deref()),
                ..upload
            };
            let normalized_external_id = normalize_external_id(upload.external_id.as_deref());
            let fallback_identity = service.identity_extractor.extract_identity(&upload).await?;
            let fallback_fingerprint = fallback_identity
                .as_ref()
                .map(|identity| identity.as_fingerprint());
            let operation_key = build_activity_upload_operation_key(
                normalized_external_id.as_deref(),
                fallback_fingerprint.as_deref(),
                &upload.file_bytes,
            );

            if let Some(external_id) = normalized_external_id.as_deref() {
                if let Some(existing) = service
                    .activities
                    .find_by_user_id_and_external_id(&user_id, external_id)
                    .await?
                {
                    return Ok(UploadedActivities {
                        created: false,
                        activity_ids: vec![existing.id.clone()],
                        activities: vec![existing],
                    });
                }
            }

            if let Some(fallback_fingerprint) = fallback_fingerprint.as_deref() {
                let matches = service
                    .activities
                    .find_by_user_id_and_fallback_identity(&user_id, fallback_fingerprint)
                    .await?;

                if matches.len() == 1 {
                    let existing = matches.into_iter().next().expect("single match expected");
                    let existing_external_id =
                        normalize_external_id(existing.external_id.as_deref());
                    if normalized_external_id.is_none()
                        || existing_external_id.is_none()
                        || existing_external_id == normalized_external_id
                    {
                        return Ok(UploadedActivities {
                            created: false,
                            activity_ids: vec![existing.id.clone()],
                            activities: vec![existing],
                        });
                    }
                } else if matches.len() > 1 {
                    warn!(
                        %user_id,
                        fallback_identity = %fallback_fingerprint,
                        matches = matches.len(),
                        "activity upload dedupe fallback matched multiple cached activities"
                    );
                }
            }

            let pending_operation = match service
                .upload_operations
                .find_by_user_id_and_operation_key(&user_id, &operation_key)
                .await?
            {
                Some(existing_operation) => {
                    if let Some(existing_result) = service
                        .recover_uploaded_operation(&user_id, &existing_operation)
                        .await?
                    {
                        return Ok(existing_result);
                    }

                    return Err(IntervalsError::Internal(
                        "Activity upload is already pending recovery".to_string(),
                    ));
                }
                None => {
                    service
                        .upload_operations
                        .upsert(
                            &user_id,
                            ActivityUploadOperation::pending(
                                operation_key.clone(),
                                normalized_external_id.clone(),
                                fallback_fingerprint.clone(),
                            ),
                        )
                        .await?
                }
            };

            let credentials = service.settings.get_credentials(&user_id).await?;
            let uploaded = service.api.upload_activity(&credentials, upload).await?;
            let uploaded_operation = service
                .upload_operations
                .upsert(
                    &user_id,
                    pending_operation.mark_uploaded(uploaded.activity_ids.clone()),
                )
                .await?;
            let stored_activities = service
                .activities
                .upsert_many(&user_id, uploaded.activities.clone())
                .await?;
            service
                .upload_operations
                .upsert(
                    &user_id,
                    uploaded_operation.mark_completed(
                        stored_activities
                            .iter()
                            .map(|activity| activity.id.clone())
                            .collect(),
                    ),
                )
                .await?;
            Ok(UploadedActivities {
                created: uploaded.created,
                activity_ids: stored_activities
                    .iter()
                    .map(|activity| activity.id.clone())
                    .collect(),
                activities: stored_activities,
            })
        })
    }

    fn update_activity(
        &self,
        user_id: &str,
        activity_id: &str,
        activity: UpdateActivity,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let activity_id = activity_id.to_string();
        Box::pin(async move {
            let credentials = service.settings.get_credentials(&user_id).await?;
            let updated = service
                .api
                .update_activity(&credentials, &activity_id, activity)
                .await?;
            service.activities.upsert(&user_id, updated.clone()).await?;
            Ok(updated)
        })
    }

    fn delete_activity(
        &self,
        user_id: &str,
        activity_id: &str,
    ) -> BoxFuture<Result<(), IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let activity_id = activity_id.to_string();
        Box::pin(async move {
            let credentials = service.settings.get_credentials(&user_id).await?;
            service
                .api
                .delete_activity(&credentials, &activity_id)
                .await?;
            service.activities.delete(&user_id, &activity_id).await
        })
    }
}
