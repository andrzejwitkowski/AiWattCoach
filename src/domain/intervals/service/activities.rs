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
        } else if let Err(error) = self
            .refresh
            .refresh_range_for_user(user_id, &range.oldest, &range.newest)
            .await
        {
            warn!(
                ?error,
                %user_id,
                oldest = %range.oldest,
                newest = %range.newest,
                "activity list refresh succeeded but calendar view refresh failed"
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
        let activity = self.activities.upsert(user_id, activity).await?;
        let activity_date = activity_date(&activity.start_date_local).to_string();
        if let Err(error) = self
            .refresh
            .refresh_range_for_user(user_id, &activity_date, &activity_date)
            .await
        {
            warn!(
                ?error,
                %user_id,
                activity_id,
                date = %activity_date,
                "activity get succeeded but calendar view refresh failed"
            );
        }
        Ok(activity)
    }

    pub(super) async fn update_activity_impl(
        &self,
        user_id: &str,
        activity_id: &str,
        activity: UpdateActivity,
    ) -> Result<Activity, IntervalsError> {
        let existing = self
            .activities
            .find_by_user_id_and_activity_id(user_id, activity_id)
            .await?;
        let old_date = existing
            .as_ref()
            .map(|activity| activity_date(&activity.start_date_local).to_string());
        let credentials = self.settings.get_credentials(user_id).await?;
        let updated = self
            .api
            .update_activity(&credentials, activity_id, activity)
            .await?;
        let updated_date = activity_date(&updated.start_date_local).to_string();
        if let Err(error) = self.activities.upsert(user_id, updated.clone()).await {
            warn!(
                ?error,
                %user_id,
                activity_id,
                "activity update succeeded upstream but local persistence failed"
            );
        } else if let Err(error) = self
            .refresh
            .refresh_range_for_user(
                user_id,
                old_date
                    .as_deref()
                    .map(|date| date.min(updated_date.as_str()))
                    .unwrap_or(updated_date.as_str()),
                old_date
                    .as_deref()
                    .map(|date| date.max(updated_date.as_str()))
                    .unwrap_or(updated_date.as_str()),
            )
            .await
        {
            warn!(
                ?error,
                %user_id,
                activity_id,
                old_date = old_date.as_deref().unwrap_or("<missing>"),
                updated_date = %updated_date,
                "activity update succeeded but calendar view refresh failed"
            );
        }
        Ok(updated)
    }

    pub(super) async fn delete_activity_impl(
        &self,
        user_id: &str,
        activity_id: &str,
    ) -> Result<(), IntervalsError> {
        let existing = self
            .activities
            .find_by_user_id_and_activity_id(user_id, activity_id)
            .await?;
        let existing_date = existing
            .as_ref()
            .map(|activity| activity_date(&activity.start_date_local).to_string());
        let credentials = self.settings.get_credentials(user_id).await?;
        self.api.delete_activity(&credentials, activity_id).await?;
        if let Err(error) = self.activities.delete(user_id, activity_id).await {
            warn!(
                ?error,
                %user_id,
                activity_id,
                "activity delete succeeded upstream but local deletion failed"
            );
        } else if let Some(existing_date) = existing_date.as_ref() {
            if let Err(error) = self
                .refresh
                .refresh_range_for_user(user_id, existing_date, existing_date)
                .await
            {
                warn!(
                    ?error,
                    %user_id,
                    activity_id,
                    date = %existing_date,
                    "activity delete succeeded but calendar view refresh failed"
                );
            }
        }
        Ok(())
    }
}
