use super::*;

impl<Api, Settings, Activities, UploadOperations, Extractor, PocRepo, Time>
    IntervalsService<Api, Settings, Activities, UploadOperations, Extractor, PocRepo, Time>
where
    Api: IntervalsApiPort,
    Settings: IntervalsSettingsPort,
    Activities: ActivityRepositoryPort,
    UploadOperations: ActivityUploadOperationRepositoryPort,
    Extractor: ActivityFileIdentityExtractorPort,
    PocRepo: PestParserPocRepositoryPort,
    Time: Clock,
{
    pub(super) async fn list_events_impl(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> Result<Vec<Event>, IntervalsError> {
        let credentials = self.settings.get_credentials(user_id).await?;
        let events = self.api.list_events(&credentials, range).await?;
        for event in &events {
            self.observe_workout_text(
                user_id,
                PestParserPocSource {
                    direction: PestParserPocDirection::Inbound,
                    operation: PestParserPocOperation::ListEvents,
                },
                Some(event.id.to_string()),
                event.structured_workout_text(),
            )
            .await;
        }
        Ok(events)
    }

    pub(super) async fn get_event_impl(
        &self,
        user_id: &str,
        event_id: i64,
    ) -> Result<Event, IntervalsError> {
        let credentials = self.settings.get_credentials(user_id).await?;
        let event = self.api.get_event(&credentials, event_id).await?;
        self.observe_workout_text(
            user_id,
            PestParserPocSource {
                direction: PestParserPocDirection::Inbound,
                operation: PestParserPocOperation::GetEvent,
            },
            Some(event.id.to_string()),
            event.structured_workout_text(),
        )
        .await;
        Ok(event)
    }

    pub(super) async fn create_event_impl(
        &self,
        user_id: &str,
        event: CreateEvent,
    ) -> Result<Event, IntervalsError> {
        let credentials = self.settings.get_credentials(user_id).await?;
        self.observe_workout_text(
            user_id,
            PestParserPocSource {
                direction: PestParserPocDirection::Outbound,
                operation: PestParserPocOperation::CreateEvent,
            },
            None,
            event
                .workout_doc
                .as_deref()
                .or(event.description.as_deref()),
        )
        .await;
        self.api.create_event(&credentials, event).await
    }

    pub(super) async fn update_event_impl(
        &self,
        user_id: &str,
        event_id: i64,
        event: UpdateEvent,
    ) -> Result<Event, IntervalsError> {
        let credentials = self.settings.get_credentials(user_id).await?;
        self.observe_workout_text(
            user_id,
            PestParserPocSource {
                direction: PestParserPocDirection::Outbound,
                operation: PestParserPocOperation::UpdateEvent,
            },
            Some(event_id.to_string()),
            event
                .workout_doc
                .as_deref()
                .or(event.description.as_deref()),
        )
        .await;
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
