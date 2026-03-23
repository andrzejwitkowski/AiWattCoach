use super::{
    ports::BoxFuture, CreateEvent, DateRange, Event, IntervalsApiPort, IntervalsError,
    IntervalsSettingsPort, UpdateEvent,
};

pub trait IntervalsUseCases: Send + Sync {
    fn list_events(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Event>, IntervalsError>>;

    fn get_event(&self, user_id: &str, event_id: i64) -> BoxFuture<Result<Event, IntervalsError>>;

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
}

#[derive(Clone)]
pub struct IntervalsService<Api, Settings>
where
    Api: IntervalsApiPort,
    Settings: IntervalsSettingsPort,
{
    api: Api,
    settings: Settings,
}

impl<Api, Settings> IntervalsService<Api, Settings>
where
    Api: IntervalsApiPort,
    Settings: IntervalsSettingsPort,
{
    pub fn new(api: Api, settings: Settings) -> Self {
        Self { api, settings }
    }
}

impl<Api, Settings> IntervalsUseCases for IntervalsService<Api, Settings>
where
    Api: IntervalsApiPort,
    Settings: IntervalsSettingsPort,
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
}
