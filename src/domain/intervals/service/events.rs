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
    pub(super) async fn list_events_impl(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> Result<Vec<Event>, IntervalsError> {
        let credentials = self.settings.get_credentials(user_id).await?;
        self.api.list_events(&credentials, range).await
    }

    pub(super) async fn get_event_impl(
        &self,
        user_id: &str,
        event_id: i64,
    ) -> Result<Event, IntervalsError> {
        let credentials = self.settings.get_credentials(user_id).await?;
        self.api.get_event(&credentials, event_id).await
    }

    pub(super) async fn create_event_impl(
        &self,
        user_id: &str,
        event: CreateEvent,
    ) -> Result<Event, IntervalsError> {
        let credentials = self.settings.get_credentials(user_id).await?;
        self.api.create_event(&credentials, event).await
    }

    pub(super) async fn update_event_impl(
        &self,
        user_id: &str,
        event_id: i64,
        event: UpdateEvent,
    ) -> Result<Event, IntervalsError> {
        let credentials = self.settings.get_credentials(user_id).await?;
        self.api.update_event(&credentials, event_id, event).await
    }

    pub(super) async fn delete_event_impl(
        &self,
        user_id: &str,
        event_id: i64,
    ) -> Result<(), IntervalsError> {
        let credentials = self.settings.get_credentials(user_id).await?;
        self.api.delete_event(&credentials, event_id).await
    }

    pub(super) async fn download_fit_impl(
        &self,
        user_id: &str,
        event_id: i64,
    ) -> Result<Vec<u8>, IntervalsError> {
        let credentials = self.settings.get_credentials(user_id).await?;
        self.api.download_fit(&credentials, event_id).await
    }
}
