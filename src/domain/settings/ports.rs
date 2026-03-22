use std::{future::Future, pin::Pin};

use super::{SettingsError, UserSettings};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait UserSettingsRepository: Clone + Send + Sync + 'static {
    fn find_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Option<UserSettings>, SettingsError>>;
    fn save(&self, settings: UserSettings) -> BoxFuture<Result<UserSettings, SettingsError>>;
}
