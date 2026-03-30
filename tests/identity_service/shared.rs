use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use aiwattcoach::domain::identity::{
    AppUser, AuthSession, BoxFuture, Clock, GoogleIdentity, GoogleOAuthPort, IdGenerator,
    IdentityError, IdentityService, IdentityServiceConfig, LoginState, LoginStateRepository,
    SessionRepository, UserRepository,
};

#[derive(Clone)]
pub(crate) struct TestGoogleOAuthAdapter;

impl GoogleOAuthPort for TestGoogleOAuthAdapter {
    fn build_authorize_url(&self, state: &str) -> Result<String, IdentityError> {
        Ok(format!(
            "https://accounts.google.com/o/oauth2/v2/auth?state={state}"
        ))
    }

    fn exchange_code_for_identity(
        &self,
        code: &str,
    ) -> BoxFuture<Result<GoogleIdentity, IdentityError>> {
        let code = code.to_string();
        Box::pin(async move {
            if code == "unverified" {
                return GoogleIdentity::new(
                    "google-subject-1",
                    "athlete@example.com",
                    false,
                    Some("Athlete".to_string()),
                    None,
                );
            }

            GoogleIdentity::new(
                "google-subject-1",
                "admin@example.com",
                true,
                Some("Admin Athlete".to_string()),
                Some("https://example.com/avatar.png".to_string()),
            )
        })
    }
}

#[derive(Clone)]
pub(crate) struct TestClock;

impl Clock for TestClock {
    fn now_epoch_seconds(&self) -> i64 {
        100
    }
}

#[derive(Clone)]
pub(crate) struct TestIdGenerator;

impl IdGenerator for TestIdGenerator {
    fn new_id(&self, prefix: &str) -> String {
        format!("{prefix}-1")
    }
}

#[derive(Clone, Default)]
pub(crate) struct InMemoryUsers {
    by_google_subject: Arc<Mutex<BTreeMap<String, AppUser>>>,
}

impl UserRepository for InMemoryUsers {
    fn find_by_id(&self, user_id: &str) -> BoxFuture<Result<Option<AppUser>, IdentityError>> {
        let id = user_id.to_string();
        let data = self.by_google_subject.clone();
        Box::pin(async move {
            Ok(data
                .lock()
                .unwrap()
                .values()
                .find(|user| user.id == id)
                .cloned())
        })
    }

    fn find_by_google_subject(
        &self,
        google_subject: &str,
    ) -> BoxFuture<Result<Option<AppUser>, IdentityError>> {
        let key = google_subject.to_string();
        let data = self.by_google_subject.clone();
        Box::pin(async move { Ok(data.lock().unwrap().get(&key).cloned()) })
    }

    fn find_by_normalized_email(
        &self,
        normalized_email: &str,
    ) -> BoxFuture<Result<Option<AppUser>, IdentityError>> {
        let email = normalized_email.to_string();
        let data = self.by_google_subject.clone();
        Box::pin(async move {
            Ok(data
                .lock()
                .unwrap()
                .values()
                .find(|user| user.email_normalized == email)
                .cloned())
        })
    }

    fn save(&self, user: AppUser) -> BoxFuture<Result<AppUser, IdentityError>> {
        let data = self.by_google_subject.clone();
        Box::pin(async move {
            data.lock()
                .unwrap()
                .insert(user.google_subject.clone(), user.clone());
            Ok(user)
        })
    }

    fn upsert_google_user(
        &self,
        new_user_id: String,
        google_identity: GoogleIdentity,
        roles: Vec<aiwattcoach::domain::identity::Role>,
    ) -> BoxFuture<Result<AppUser, IdentityError>> {
        let data = self.by_google_subject.clone();
        Box::pin(async move {
            let mut users = data.lock().unwrap();
            let existing = users
                .values()
                .find(|user| {
                    user.google_subject == google_identity.subject
                        || user.email_normalized == google_identity.email_normalized
                })
                .cloned();

            let user = AppUser::new(
                existing.map(|user| user.id).unwrap_or(new_user_id),
                google_identity.subject.clone(),
                google_identity.email.clone(),
                roles,
                google_identity.display_name.clone(),
                google_identity.avatar_url.clone(),
                google_identity.email_verified,
            );

            if let Some(previous_subject) = users
                .iter()
                .find(|(_, existing_user)| existing_user.id == user.id)
                .map(|(subject, _)| subject.clone())
            {
                users.remove(&previous_subject);
            }

            users.insert(user.google_subject.clone(), user.clone());
            Ok(user)
        })
    }

    fn save_google_user_for_identity(
        &self,
        new_user_id: String,
        google_identity: GoogleIdentity,
        roles: Vec<aiwattcoach::domain::identity::Role>,
    ) -> BoxFuture<Result<AppUser, IdentityError>> {
        let repository = self.clone();
        Box::pin(async move {
            let by_subject = repository
                .find_by_google_subject(&google_identity.subject)
                .await?;
            let by_email = repository
                .find_by_normalized_email(&google_identity.email_normalized)
                .await?;

            match (by_subject, by_email) {
                (Some(subject_user), Some(email_user)) if subject_user.id != email_user.id => {
                    Err(IdentityError::Repository(
                        "conflicting google subject/email mapping".to_string(),
                    ))
                }
                (Some(_), _) => {
                    repository
                        .upsert_google_user(new_user_id, google_identity, roles)
                        .await
                }
                (None, Some(email_user)) => {
                    if email_user.google_subject != google_identity.subject {
                        return Err(IdentityError::Repository(
                            "conflicting google subject/email mapping".to_string(),
                        ));
                    }

                    repository
                        .save(AppUser::new(
                            email_user.id,
                            google_identity.subject,
                            google_identity.email,
                            roles,
                            google_identity.display_name,
                            google_identity.avatar_url,
                            google_identity.email_verified,
                        ))
                        .await
                }
                (None, None) => {
                    repository
                        .upsert_google_user(new_user_id, google_identity, roles)
                        .await
                }
            }
        })
    }
}

#[derive(Clone, Default)]
pub(crate) struct InMemorySessions {
    items: Arc<Mutex<BTreeMap<String, AuthSession>>>,
}

impl SessionRepository for InMemorySessions {
    fn find_by_id(
        &self,
        session_id: &str,
    ) -> BoxFuture<Result<Option<AuthSession>, IdentityError>> {
        let id = session_id.to_string();
        let data = self.items.clone();
        Box::pin(async move { Ok(data.lock().unwrap().get(&id).cloned()) })
    }

    fn save(&self, session: AuthSession) -> BoxFuture<Result<AuthSession, IdentityError>> {
        let data = self.items.clone();
        Box::pin(async move {
            data.lock()
                .unwrap()
                .insert(session.id.clone(), session.clone());
            Ok(session)
        })
    }

    fn delete(&self, session_id: &str) -> BoxFuture<Result<(), IdentityError>> {
        let id = session_id.to_string();
        let data = self.items.clone();
        Box::pin(async move {
            data.lock().unwrap().remove(&id);
            Ok(())
        })
    }
}

#[derive(Clone)]
pub(crate) struct InMemoryLoginStates {
    pub(crate) items: Arc<Mutex<Vec<LoginState>>>,
}

impl LoginStateRepository for InMemoryLoginStates {
    fn create(&self, login_state: LoginState) -> BoxFuture<Result<LoginState, IdentityError>> {
        let data = self.items.clone();
        Box::pin(async move {
            data.lock().unwrap().push(login_state.clone());
            Ok(login_state)
        })
    }

    fn find_by_id(&self, state_id: &str) -> BoxFuture<Result<Option<LoginState>, IdentityError>> {
        let id = state_id.to_string();
        let data = self.items.clone();
        Box::pin(async move {
            Ok(data
                .lock()
                .unwrap()
                .iter()
                .find(|state| state.id == id)
                .cloned())
        })
    }

    fn delete(&self, state_id: &str) -> BoxFuture<Result<(), IdentityError>> {
        let id = state_id.to_string();
        let data = self.items.clone();
        Box::pin(async move {
            data.lock().unwrap().retain(|state| state.id != id);
            Ok(())
        })
    }

    fn consume(&self, state_id: &str) -> BoxFuture<Result<Option<LoginState>, IdentityError>> {
        let id = state_id.to_string();
        let data = self.items.clone();
        Box::pin(async move {
            let mut items = data.lock().unwrap();
            let index = items.iter().position(|state| state.id == id);
            Ok(index.map(|position| items.remove(position)))
        })
    }
}

pub(crate) struct TestIdentityService {
    inner: IdentityService<
        InMemoryUsers,
        InMemorySessions,
        InMemoryLoginStates,
        TestGoogleOAuthAdapter,
        TestClock,
        TestIdGenerator,
    >,
    pub(crate) sessions: InMemorySessions,
}

pub(crate) fn test_service(
    login_states: Arc<Mutex<Vec<LoginState>>>,
    admin_emails: Vec<String>,
) -> TestIdentityService {
    let users = InMemoryUsers::default();
    let sessions = InMemorySessions::default();
    let states = InMemoryLoginStates {
        items: login_states,
    };

    let service = IdentityService::new(
        users,
        sessions.clone(),
        states,
        TestGoogleOAuthAdapter,
        TestClock,
        TestIdGenerator,
        IdentityServiceConfig::new(admin_emails, 24),
    );

    TestIdentityService {
        inner: service,
        sessions,
    }
}

impl std::ops::Deref for TestIdentityService {
    type Target = IdentityService<
        InMemoryUsers,
        InMemorySessions,
        InMemoryLoginStates,
        TestGoogleOAuthAdapter,
        TestClock,
        TestIdGenerator,
    >;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
