use super::*;
use crate::domain::intervals::ports::activity_date;

impl<Api, Settings, Activities, UploadOperations, Extractor, PocRepo, Time, Refresh>
    IntervalsService<Api, Settings, Activities, UploadOperations, Extractor, PocRepo, Time, Refresh>
where
    Api: IntervalsApiPort,
    Settings: IntervalsSettingsPort,
    Activities: ActivityRepositoryPort,
    UploadOperations: ActivityUploadOperationRepositoryPort,
    Extractor: ActivityFileIdentityExtractorPort,
    PocRepo: PestParserPocRepositoryPort,
    Time: Clock,
    Refresh: crate::domain::calendar_view::CalendarEntryViewRefreshPort,
{
    pub(super) async fn recover_uploaded_operation(
        &self,
        user_id: &str,
        operation: &ActivityUploadOperation,
    ) -> Result<Option<UploadedActivities>, IntervalsError> {
        match operation.status {
            ActivityUploadOperationStatus::Pending => Ok(None),
            ActivityUploadOperationStatus::Failed => Ok(None),
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

                for activity in &activities {
                    let activity_date = activity_date(&activity.start_date_local).to_string();
                    if let Err(error) = self
                        .refresh
                        .refresh_range_for_user(user_id, &activity_date, &activity_date)
                        .await
                    {
                        warn!(
                            ?error,
                            %user_id,
                            activity_id = %activity.id,
                            date = %activity_date,
                            "activity upload recovery succeeded but calendar view refresh failed"
                        );
                    }
                }

                Ok(Some(UploadedActivities {
                    created: false,
                    activity_ids: completed.uploaded_activity_ids,
                    activities,
                }))
            }
        }
    }

    pub(super) async fn upload_activity_impl(
        &self,
        user_id: &str,
        upload: UploadActivity,
    ) -> Result<UploadedActivities, IntervalsError> {
        let upload = UploadActivity {
            external_id: normalize_external_id(upload.external_id.as_deref()),
            ..upload
        };
        let normalized_external_id = upload.external_id.clone();
        let fallback_identity = self.identity_extractor.extract_identity(&upload).await?;
        let fallback_fingerprint = fallback_identity
            .as_ref()
            .map(|identity| identity.as_fingerprint());
        let operation_key = build_activity_upload_operation_key(
            normalized_external_id.as_deref(),
            fallback_fingerprint.as_deref(),
            &upload.file_bytes,
        );

        if let Some(external_id) = normalized_external_id.as_deref() {
            if let Some(existing) = self
                .activities
                .find_by_user_id_and_external_id(user_id, external_id)
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
            let matches = self
                .activities
                .find_by_user_id_and_fallback_identity(user_id, fallback_fingerprint)
                .await?;

            if matches.len() == 1 {
                let existing = matches.into_iter().next().expect("single match expected");
                let existing_external_id = normalize_external_id(existing.external_id.as_deref());
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

        let pending_operation = match self
            .upload_operations
            .claim_pending(
                user_id,
                ActivityUploadOperation::pending(
                    operation_key.clone(),
                    normalized_external_id.clone(),
                    fallback_fingerprint.clone(),
                ),
            )
            .await?
        {
            ActivityUploadOperationClaimResult::Claimed(pending_operation) => pending_operation,
            ActivityUploadOperationClaimResult::Existing(existing_operation) => {
                if let Some(existing_result) = self
                    .recover_uploaded_operation(user_id, &existing_operation)
                    .await?
                {
                    return Ok(existing_result);
                }

                return Err(IntervalsError::Internal(
                    "Activity upload is already pending recovery".to_string(),
                ));
            }
        };

        let credentials = self.settings.get_credentials(user_id).await?;
        let uploaded = match self.api.upload_activity(&credentials, upload).await {
            Ok(uploaded) => uploaded,
            Err(error) => {
                self.upload_operations
                    .upsert(user_id, pending_operation.mark_failed())
                    .await?;
                return Err(error);
            }
        };
        let uploaded_operation = self
            .upload_operations
            .upsert(
                user_id,
                pending_operation.mark_uploaded(uploaded.activity_ids.clone()),
            )
            .await?;
        let stored_activities = match self
            .activities
            .upsert_many(user_id, uploaded.activities.clone())
            .await
        {
            Ok(stored_activities) => stored_activities,
            Err(error) => {
                self.upload_operations
                    .upsert(
                        user_id,
                        uploaded_operation.mark_uploaded(uploaded.activity_ids.clone()),
                    )
                    .await?;
                return Err(error);
            }
        };
        self.upload_operations
            .upsert(
                user_id,
                uploaded_operation.mark_completed(
                    stored_activities
                        .iter()
                        .map(|activity| activity.id.clone())
                        .collect(),
                ),
            )
            .await?;
        for activity in &stored_activities {
            let activity_date = activity_date(&activity.start_date_local).to_string();
            if let Err(error) = self
                .refresh
                .refresh_range_for_user(user_id, &activity_date, &activity_date)
                .await
            {
                warn!(
                    ?error,
                    %user_id,
                    activity_id = %activity.id,
                    date = %activity_date,
                    "activity upload succeeded but calendar view refresh failed"
                );
            }
        }
        Ok(UploadedActivities {
            created: uploaded.created,
            activity_ids: stored_activities
                .iter()
                .map(|activity| activity.id.clone())
                .collect(),
            activities: stored_activities,
        })
    }
}
