use super::*;

impl<Api, Settings, Activities, UploadOperations, Extractor>
    IntervalsService<Api, Settings, Activities, UploadOperations, Extractor>
where
    Api: IntervalsApiPort,
    Settings: IntervalsSettingsPort,
    Activities: ActivityRepositoryPort,
    UploadOperations: ActivityUploadOperationRepositoryPort,
    Extractor: ActivityFileIdentityExtractorPort,
{
    pub(super) async fn list_activities_impl(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> Result<Vec<Activity>, IntervalsError> {
        let credentials = self.settings.get_credentials(user_id).await?;
        let activities = self.api.list_activities(&credentials, range).await?;
        if let Err(error) = self
            .activities
            .upsert_many(user_id, activities.clone())
            .await
        {
            warn!(
                ?error,
                %user_id,
                "activity list refresh succeeded but local persistence failed"
            );
        }
        Ok(activities)
    }

    pub(super) async fn get_activity_impl(
        &self,
        user_id: &str,
        activity_id: &str,
    ) -> Result<Activity, IntervalsError> {
        let credentials = self.settings.get_credentials(user_id).await?;
        let activity = self.api.get_activity(&credentials, activity_id).await?;
        self.activities.upsert(user_id, activity.clone()).await?;
        Ok(activity)
    }

    pub(super) async fn update_activity_impl(
        &self,
        user_id: &str,
        activity_id: &str,
        activity: UpdateActivity,
    ) -> Result<Activity, IntervalsError> {
        let credentials = self.settings.get_credentials(user_id).await?;
        let updated = self
            .api
            .update_activity(&credentials, activity_id, activity)
            .await?;
        if let Err(error) = self.activities.upsert(user_id, updated.clone()).await {
            warn!(
                ?error,
                %user_id,
                activity_id,
                "activity update succeeded upstream but local persistence failed"
            );
        }
        Ok(updated)
    }

    pub(super) async fn delete_activity_impl(
        &self,
        user_id: &str,
        activity_id: &str,
    ) -> Result<(), IntervalsError> {
        let credentials = self.settings.get_credentials(user_id).await?;
        self.api.delete_activity(&credentials, activity_id).await?;
        if let Err(error) = self.activities.delete(user_id, activity_id).await {
            warn!(
                ?error,
                %user_id,
                activity_id,
                "activity delete succeeded upstream but local deletion failed"
            );
        }
        Ok(())
    }
}
