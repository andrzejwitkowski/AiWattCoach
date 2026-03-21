use std::{future::Future, pin::Pin};

use super::{AppUser, AuthSession, GoogleIdentity, IdentityError, LoginState};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait UserRepository: Clone + Send + Sync + 'static {
    fn find_by_id(&self, user_id: &str) -> BoxFuture<Result<Option<AppUser>, IdentityError>>;
    fn find_by_google_subject(
        &self,
        google_subject: &str,
    ) -> BoxFuture<Result<Option<AppUser>, IdentityError>>;
    fn find_by_normalized_email(
        &self,
        normalized_email: &str,
    ) -> BoxFuture<Result<Option<AppUser>, IdentityError>>;
    fn save(&self, user: AppUser) -> BoxFuture<Result<AppUser, IdentityError>>;
}

pub trait SessionRepository: Clone + Send + Sync + 'static {
    fn find_by_id(&self, session_id: &str)
        -> BoxFuture<Result<Option<AuthSession>, IdentityError>>;
    fn save(&self, session: AuthSession) -> BoxFuture<Result<AuthSession, IdentityError>>;
    fn delete(&self, session_id: &str) -> BoxFuture<Result<(), IdentityError>>;
}

pub trait LoginStateRepository: Clone + Send + Sync + 'static {
    fn create(&self, login_state: LoginState) -> BoxFuture<Result<LoginState, IdentityError>>;
    fn find_by_id(&self, state_id: &str) -> BoxFuture<Result<Option<LoginState>, IdentityError>>;
    fn delete(&self, state_id: &str) -> BoxFuture<Result<(), IdentityError>>;
}

pub trait GoogleOAuthPort: Clone + Send + Sync + 'static {
    fn build_authorize_url(&self, state: &str) -> Result<String, IdentityError>;
    fn exchange_code_for_identity(
        &self,
        code: &str,
    ) -> BoxFuture<Result<GoogleIdentity, IdentityError>>;
}

pub trait Clock: Clone + Send + Sync + 'static {
    fn now_epoch_seconds(&self) -> i64;
}

pub trait IdGenerator: Clone + Send + Sync + 'static {
    fn new_id(&self, prefix: &str) -> String;
}
