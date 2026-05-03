mod shared {
    use std::{
        fs,
        path::PathBuf,
        sync::{Arc, Mutex, OnceLock},
        time::Duration,
    };

    use aiwattcoach::{
        build_app_with_frontend_dist,
        config::AppState,
        domain::{
            calendar_coach::CalendarCoachUseCases,
            coach_conversation::{
                BoxFuture, CoachConversation, CoachConversationError, CoachConversationFocus,
                CoachConversationMessage, CoachConversationMessageRole, CoachConversationReply,
                CoachConversationStatus, CoachConversationSurface,
                PersistedConversationUserMessage, SendConversationMessageResult,
            },
            identity::{
                AppUser, GoogleLoginOutcome, GoogleLoginStart, IdentityError, IdentityUseCases,
                Role, WhitelistEntry,
            },
            workout_summary::PublicToolCall,
        },
        Settings,
    };
    use axum::body::to_bytes;
    use mongodb::Client;

    pub(crate) const RESPONSE_LIMIT_BYTES: usize = 4 * 1024;
    static SHARED_FRONTEND_FIXTURE: OnceLock<FrontendFixture> = OnceLock::new();

    pub(crate) async fn get_json<T: serde::de::DeserializeOwned>(
        response: axum::response::Response,
    ) -> T {
        let parts = response.into_parts();
        let body = to_bytes(parts.1, RESPONSE_LIMIT_BYTES)
            .await
            .expect("body to be collected");
        serde_json::from_slice(&body).expect("valid JSON")
    }

    #[derive(Clone, Default)]
    pub(crate) struct TestIdentityServiceWithSession {
        pub(crate) session_id: String,
        pub(crate) user_id: String,
    }

    impl IdentityUseCases for TestIdentityServiceWithSession {
        fn begin_google_login(
            &self,
            _return_to: Option<String>,
        ) -> BoxFuture<Result<GoogleLoginStart, IdentityError>> {
            Box::pin(async {
                Ok(GoogleLoginStart {
                    state: "state-1".to_string(),
                    redirect_url: "https://accounts.google.com/o/oauth2/v2/auth?state=state-1"
                        .to_string(),
                })
            })
        }

        fn join_whitelist(
            &self,
            email: String,
        ) -> BoxFuture<Result<WhitelistEntry, IdentityError>> {
            Box::pin(async move { Ok(WhitelistEntry::new(email, false, 100, 100)) })
        }

        fn handle_google_callback(
            &self,
            _state: &str,
            _code: &str,
        ) -> BoxFuture<Result<GoogleLoginOutcome, IdentityError>> {
            Box::pin(async { Err(IdentityError::External("not used in test".to_string())) })
        }

        fn get_current_user(
            &self,
            session_id: &str,
        ) -> BoxFuture<Result<Option<AppUser>, IdentityError>> {
            let user_id = self.user_id.clone();
            let expected_session_id = if self.session_id.is_empty() {
                "session-1".to_string()
            } else {
                self.session_id.clone()
            };
            let session_id = session_id.to_string();
            Box::pin(async move {
                if session_id != expected_session_id {
                    return Ok(None);
                }

                Ok(Some(AppUser::new(
                    if user_id.is_empty() {
                        "user-1".to_string()
                    } else {
                        user_id
                    },
                    "google-subject-1".to_string(),
                    "athlete@example.com".to_string(),
                    vec![Role::User],
                    Some("Test User".to_string()),
                    None,
                    true,
                )))
            })
        }

        fn logout(&self, _session_id: &str) -> BoxFuture<Result<(), IdentityError>> {
            Box::pin(async { Ok(()) })
        }

        fn require_admin(&self, _session_id: &str) -> BoxFuture<Result<AppUser, IdentityError>> {
            Box::pin(async { Err(IdentityError::Forbidden) })
        }
    }

    #[derive(Clone)]
    pub(crate) struct TestCalendarCoachService {
        state: Arc<Mutex<CoachState>>,
        coach_reply_delay: Option<Duration>,
        availability_configured: bool,
        summary_may_regenerate_before_reply: bool,
        next_tool_call: Option<PublicToolCall>,
    }

    #[derive(Clone)]
    struct CoachState {
        active_conversation_id: String,
        next_conversation_index: usize,
        processed_user_messages: Vec<String>,
        conversations: Vec<StoredConversation>,
    }

    #[derive(Clone)]
    struct StoredConversation {
        conversation: CoachConversation,
        messages: Vec<CoachConversationMessage>,
    }

    impl Default for CoachState {
        fn default() -> Self {
            let conversation = sample_conversation("conversation-1");
            Self {
                active_conversation_id: conversation.conversation_id.clone(),
                next_conversation_index: 2,
                processed_user_messages: Vec::new(),
                conversations: vec![StoredConversation {
                    conversation,
                    messages: Vec::new(),
                }],
            }
        }
    }

    impl TestCalendarCoachService {
        pub(crate) fn with_coach_reply_delay(mut self, delay: Duration) -> Self {
            self.coach_reply_delay = Some(delay);
            self
        }

        pub(crate) fn with_availability_configured(mut self, configured: bool) -> Self {
            self.availability_configured = configured;
            self
        }

        pub(crate) fn with_summary_regeneration_hint(mut self, enabled: bool) -> Self {
            self.summary_may_regenerate_before_reply = enabled;
            self
        }

        pub(crate) fn with_tool_call(mut self, tool_call: PublicToolCall) -> Self {
            self.next_tool_call = Some(tool_call);
            self
        }

        pub(crate) fn processed_user_messages(&self) -> Vec<String> {
            self.state.lock().unwrap().processed_user_messages.clone()
        }

        pub(crate) fn conversation(&self, conversation_id: &str) -> CoachConversation {
            self.state
                .lock()
                .unwrap()
                .conversations
                .iter()
                .find(|stored| stored.conversation.conversation_id == conversation_id)
                .expect("conversation to exist")
                .conversation
                .clone()
        }

        pub(crate) fn messages(&self, conversation_id: &str) -> Vec<CoachConversationMessage> {
            self.state
                .lock()
                .unwrap()
                .conversations
                .iter()
                .find(|stored| stored.conversation.conversation_id == conversation_id)
                .expect("conversation to exist")
                .messages
                .clone()
        }
    }

    impl Default for TestCalendarCoachService {
        fn default() -> Self {
            Self {
                state: Arc::new(Mutex::new(CoachState::default())),
                coach_reply_delay: None,
                availability_configured: true,
                summary_may_regenerate_before_reply: false,
                next_tool_call: None,
            }
        }
    }

    impl CalendarCoachUseCases for TestCalendarCoachService {
        fn get_current_conversation(
            &self,
            _user_id: &str,
        ) -> BoxFuture<
            Result<(CoachConversation, Vec<CoachConversationMessage>), CoachConversationError>,
        > {
            let state = self.state.clone();
            Box::pin(async move {
                let state = state.lock().unwrap();
                let stored = state
                    .conversations
                    .iter()
                    .find(|stored| {
                        stored.conversation.conversation_id == state.active_conversation_id
                    })
                    .expect("active conversation to exist");
                Ok((stored.conversation.clone(), stored.messages.clone()))
            })
        }

        fn start_new_conversation(
            &self,
            _user_id: &str,
        ) -> BoxFuture<
            Result<(CoachConversation, Vec<CoachConversationMessage>), CoachConversationError>,
        > {
            let state = self.state.clone();
            Box::pin(async move {
                let mut state = state.lock().unwrap();
                let previous_active_id = state.active_conversation_id.clone();
                if let Some(previous) = state
                    .conversations
                    .iter_mut()
                    .find(|stored| stored.conversation.conversation_id == previous_active_id)
                {
                    previous.conversation.status = CoachConversationStatus::Archived;
                    previous.conversation.updated_at_epoch_seconds = 1_700_000_100;
                }

                let conversation_id = format!("conversation-{}", state.next_conversation_index);
                state.next_conversation_index += 1;
                let conversation = sample_conversation(&conversation_id);
                state.active_conversation_id = conversation_id;
                state.conversations.push(StoredConversation {
                    conversation: conversation.clone(),
                    messages: Vec::new(),
                });
                Ok((conversation, Vec::new()))
            })
        }

        fn get_conversation(
            &self,
            _user_id: &str,
            conversation_id: &str,
        ) -> BoxFuture<
            Result<(CoachConversation, Vec<CoachConversationMessage>), CoachConversationError>,
        > {
            let state = self.state.clone();
            let conversation_id = conversation_id.to_string();
            Box::pin(async move {
                let state = state.lock().unwrap();
                let stored = state
                    .conversations
                    .iter()
                    .find(|stored| stored.conversation.conversation_id == conversation_id)
                    .ok_or(CoachConversationError::NotFound)?;
                Ok((stored.conversation.clone(), stored.messages.clone()))
            })
        }

        fn send_message(
            &self,
            _user_id: &str,
            conversation_id: &str,
            content: String,
        ) -> BoxFuture<Result<SendConversationMessageResult, CoachConversationError>> {
            let state = self.state.clone();
            let conversation_id = conversation_id.to_string();
            let availability_configured = self.availability_configured;
            Box::pin(async move {
                let content =
                    aiwattcoach::domain::coach_conversation::validate_conversation_message_content(
                        &content,
                    )?;
                if !availability_configured {
                    return Err(CoachConversationError::Validation(
                        "availability must be configured before chatting with coach".to_string(),
                    ));
                }

                let mut state = state.lock().unwrap();
                state.processed_user_messages.push(content.clone());
                let stored = state
                    .conversations
                    .iter_mut()
                    .find(|stored| stored.conversation.conversation_id == conversation_id)
                    .ok_or(CoachConversationError::NotFound)?;

                if stored.conversation.status == CoachConversationStatus::Archived {
                    return Err(CoachConversationError::Archived);
                }

                let user_message = CoachConversationMessage {
                    id: format!("message-user-{}", stored.messages.len() + 1),
                    conversation_id: stored.conversation.conversation_id.clone(),
                    user_id: stored.conversation.user_id.clone(),
                    role: CoachConversationMessageRole::User,
                    content,
                    tool_call: None,
                    created_at_epoch_seconds: 1_700_000_000,
                };
                let coach_message = CoachConversationMessage {
                    id: format!("message-coach-{}", stored.messages.len() + 2),
                    conversation_id: stored.conversation.conversation_id.clone(),
                    user_id: stored.conversation.user_id.clone(),
                    role: CoachConversationMessageRole::Coach,
                    content: format!("Coach reply to: {}", user_message.content),
                    tool_call: None,
                    created_at_epoch_seconds: 1_700_000_001,
                };
                stored.messages.push(user_message.clone());
                stored.messages.push(coach_message.clone());
                stored.conversation.updated_at_epoch_seconds = 1_700_000_100;

                Ok(SendConversationMessageResult {
                    conversation: stored.conversation.clone(),
                    messages: stored.messages.clone(),
                    user_message,
                    coach_message,
                })
            })
        }

        fn append_user_message(
            &self,
            _user_id: &str,
            conversation_id: &str,
            content: String,
        ) -> BoxFuture<Result<PersistedConversationUserMessage, CoachConversationError>> {
            let state = self.state.clone();
            let conversation_id = conversation_id.to_string();
            let availability_configured = self.availability_configured;
            let summary_may_regenerate_before_reply = self.summary_may_regenerate_before_reply;
            Box::pin(async move {
                let content =
                    aiwattcoach::domain::coach_conversation::validate_conversation_message_content(
                        &content,
                    )?;
                if !availability_configured {
                    return Err(CoachConversationError::Validation(
                        "availability must be configured before chatting with coach".to_string(),
                    ));
                }

                let mut state = state.lock().unwrap();
                state.processed_user_messages.push(content.clone());
                let stored = state
                    .conversations
                    .iter_mut()
                    .find(|stored| stored.conversation.conversation_id == conversation_id)
                    .ok_or(CoachConversationError::NotFound)?;

                if stored.conversation.status == CoachConversationStatus::Archived {
                    return Err(CoachConversationError::Archived);
                }

                let user_message = CoachConversationMessage {
                    id: format!("message-user-{}", stored.messages.len() + 1),
                    conversation_id: stored.conversation.conversation_id.clone(),
                    user_id: stored.conversation.user_id.clone(),
                    role: CoachConversationMessageRole::User,
                    content,
                    tool_call: None,
                    created_at_epoch_seconds: 1_700_000_000,
                };
                stored.messages.push(user_message.clone());
                stored.conversation.updated_at_epoch_seconds = 1_700_000_100;

                Ok(PersistedConversationUserMessage {
                    conversation: stored.conversation.clone(),
                    messages: stored.messages.clone(),
                    user_message,
                    athlete_summary_may_regenerate_before_reply:
                        summary_may_regenerate_before_reply,
                })
            })
        }

        fn generate_reply(
            &self,
            _user_id: &str,
            conversation_id: &str,
            user_message_id: String,
        ) -> BoxFuture<Result<CoachConversationReply, CoachConversationError>> {
            let state = self.state.clone();
            let conversation_id = conversation_id.to_string();
            let coach_reply_delay = self.coach_reply_delay;
            let availability_configured = self.availability_configured;
            let next_tool_call = self.next_tool_call.clone();
            Box::pin(async move {
                if let Some(delay) = coach_reply_delay {
                    tokio::time::sleep(delay).await;
                }
                if !availability_configured {
                    return Err(CoachConversationError::Validation(
                        "availability must be configured before chatting with coach".to_string(),
                    ));
                }

                let mut state = state.lock().unwrap();
                let stored = state
                    .conversations
                    .iter_mut()
                    .find(|stored| stored.conversation.conversation_id == conversation_id)
                    .ok_or(CoachConversationError::NotFound)?;

                if stored.conversation.status == CoachConversationStatus::Archived {
                    return Err(CoachConversationError::Archived);
                }

                let user_message = stored
                    .messages
                    .iter()
                    .find(|message| {
                        message.id == user_message_id
                            && message.role == CoachConversationMessageRole::User
                    })
                    .cloned()
                    .ok_or_else(|| {
                        CoachConversationError::Validation(
                            "user message must be persisted before generating coach reply"
                                .to_string(),
                        )
                    })?;

                if let Some(tool_call) = next_tool_call {
                    stored.messages.push(CoachConversationMessage {
                        id: tool_call.id.clone(),
                        conversation_id: stored.conversation.conversation_id.clone(),
                        user_id: stored.conversation.user_id.clone(),
                        role: CoachConversationMessageRole::Tool,
                        content: format!("Tool call: {}", tool_call.name),
                        tool_call: Some(tool_call),
                        created_at_epoch_seconds: 1_700_000_001,
                    });
                }

                let coach_message = CoachConversationMessage {
                    id: format!("message-coach-{}", stored.messages.len() + 1),
                    conversation_id: stored.conversation.conversation_id.clone(),
                    user_id: stored.conversation.user_id.clone(),
                    role: CoachConversationMessageRole::Coach,
                    content: format!("Coach reply to: {}", user_message.content),
                    tool_call: None,
                    created_at_epoch_seconds: 1_700_000_001,
                };
                stored.messages.push(coach_message.clone());
                stored.conversation.updated_at_epoch_seconds = 1_700_000_100;

                Ok(CoachConversationReply {
                    conversation: stored.conversation.clone(),
                    messages: stored.messages.clone(),
                    coach_message,
                    athlete_summary_was_regenerated: false,
                })
            })
        }
    }

    pub(crate) async fn calendar_coach_test_app(
        identity_service: impl IdentityUseCases + 'static,
        calendar_coach_service: impl CalendarCoachUseCases + 'static,
    ) -> axum::Router {
        let settings = Settings::test_defaults();
        let fixture = shared_frontend_fixture();

        let app_state = AppState::new(
            settings.app_name,
            settings.mongo.database,
            test_mongo_client(&settings.mongo.uri).await,
        )
        .with_identity_service(
            Arc::new(identity_service),
            "aiwattcoach_session",
            "lax",
            false,
            24,
        )
        .with_calendar_coach_service(Arc::new(calendar_coach_service));

        build_app_with_frontend_dist(app_state, fixture.dist_dir())
    }

    fn sample_conversation(conversation_id: &str) -> CoachConversation {
        CoachConversation {
            conversation_id: conversation_id.to_string(),
            user_id: "user-1".to_string(),
            surface: CoachConversationSurface::Calendar,
            status: CoachConversationStatus::Active,
            focus: CoachConversationFocus::Overview,
            hidden_transcript: Vec::new(),
            created_at_epoch_seconds: 1_700_000_000,
            updated_at_epoch_seconds: 1_700_000_000,
        }
    }

    struct FrontendFixture {
        root: PathBuf,
    }

    fn shared_frontend_fixture() -> &'static FrontendFixture {
        SHARED_FRONTEND_FIXTURE.get_or_init(frontend_fixture)
    }

    fn frontend_fixture() -> FrontendFixture {
        let root = std::env::temp_dir().join(format!(
            "aiwattcoach-calendar-coach-spa-fixture-{}",
            std::process::id()
        ));
        let dist_dir = root.join("dist");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&dist_dir).unwrap();
        fs::write(
            dist_dir.join("index.html"),
            "<!doctype html><html><body><div id=\"root\">fixture</div></body></html>",
        )
        .unwrap();

        FrontendFixture { root }
    }

    impl FrontendFixture {
        fn dist_dir(&self) -> PathBuf {
            self.root.join("dist")
        }
    }

    impl Drop for FrontendFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    async fn test_mongo_client(uri: &str) -> Client {
        Client::with_uri_str(uri)
            .await
            .expect("test mongo client should be created")
    }
}

use std::{net::SocketAddr, time::Duration};

use aiwattcoach::domain::workout_summary::PublicToolCall;
use futures::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::task::JoinHandle;
use tokio::{net::TcpListener, time::timeout};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, protocol::Message},
};
use tower::ServiceExt;

use shared::{
    calendar_coach_test_app, get_json, TestCalendarCoachService, TestIdentityServiceWithSession,
};

struct SpawnedApp {
    address: SocketAddr,
    task: JoinHandle<()>,
}

impl SpawnedApp {
    async fn start(app: axum::Router) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        Self { address, task }
    }
}

impl Drop for SpawnedApp {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[tokio::test]
async fn get_current_conversation_requires_authentication() {
    let app = calendar_coach_test_app(
        TestIdentityServiceWithSession::default(),
        TestCalendarCoachService::default(),
    )
    .await;

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/calendar/coach/current")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn get_current_conversation_returns_active_conversation() {
    let app = calendar_coach_test_app(
        TestIdentityServiceWithSession::default(),
        TestCalendarCoachService::default(),
    )
    .await;

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/calendar/coach/current")
                .header(axum::http::header::COOKIE, "aiwattcoach_session=session-1")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body: Value = get_json(response).await;
    assert_eq!(body["conversation"]["conversationId"], "conversation-1");
    assert_eq!(body["conversation"]["surface"], "calendar");
    assert_eq!(body["conversation"]["focus"], "overview");
    assert_eq!(body["messages"], serde_json::json!([]));
}

#[tokio::test]
async fn start_new_conversation_archives_previous_active_conversation() {
    let service = TestCalendarCoachService::default();
    let app =
        calendar_coach_test_app(TestIdentityServiceWithSession::default(), service.clone()).await;

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method(axum::http::Method::POST)
                .uri("/api/calendar/coach/conversations")
                .header(axum::http::header::COOKIE, "aiwattcoach_session=session-1")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), axum::http::StatusCode::CREATED);
    let body: Value = get_json(response).await;
    assert_eq!(body["conversation"]["conversationId"], "conversation-2");
    assert_eq!(body["messages"], serde_json::json!([]));
    assert_eq!(
        service.conversation("conversation-1").status,
        aiwattcoach::domain::coach_conversation::CoachConversationStatus::Archived
    );
}

#[tokio::test]
async fn send_message_returns_updated_transcript() {
    let service = TestCalendarCoachService::default();
    let app =
        calendar_coach_test_app(TestIdentityServiceWithSession::default(), service.clone()).await;

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method(axum::http::Method::POST)
                .uri("/api/calendar/coach/conversations/conversation-1/messages")
                .header(axum::http::header::COOKIE, "aiwattcoach_session=session-1")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    "{\"content\":\"How should I pace this week?\"}",
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body: Value = get_json(response).await;
    assert_eq!(body["userMessage"]["role"], "user");
    assert_eq!(body["coachMessage"]["role"], "coach");
    assert_eq!(body["messages"].as_array().unwrap().len(), 2);
    assert_eq!(
        service.processed_user_messages(),
        vec!["How should I pace this week?".to_string()]
    );
}

#[tokio::test]
async fn send_message_returns_validation_error_when_availability_missing() {
    let app = calendar_coach_test_app(
        TestIdentityServiceWithSession::default(),
        TestCalendarCoachService::default().with_availability_configured(false),
    )
    .await;

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method(axum::http::Method::POST)
                .uri("/api/calendar/coach/conversations/conversation-1/messages")
                .header(axum::http::header::COOKIE, "aiwattcoach_session=session-1")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(r#"{"content":"Need advice"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
    let body: Value = get_json(response).await;
    assert_eq!(
        body["message"],
        "availability must be configured before chatting with coach"
    );
}

#[tokio::test]
async fn websocket_requires_authentication() {
    let app = calendar_coach_test_app(
        TestIdentityServiceWithSession::default(),
        TestCalendarCoachService::default(),
    )
    .await;

    let server = SpawnedApp::start(app).await;

    let result = connect_async(format!(
        "ws://{}/api/calendar/coach/conversations/conversation-1/ws",
        server.address
    ))
    .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn websocket_sends_typing_then_coach_message() {
    let service = TestCalendarCoachService::default().with_summary_regeneration_hint(true);
    let app =
        calendar_coach_test_app(TestIdentityServiceWithSession::default(), service.clone()).await;

    let server = SpawnedApp::start(app).await;

    let mut request = format!(
        "ws://{}/api/calendar/coach/conversations/conversation-1/ws",
        server.address
    )
    .into_client_request()
    .unwrap();
    request
        .headers_mut()
        .insert("Cookie", "aiwattcoach_session=session-1".parse().unwrap());

    let (mut socket, _) = connect_async(request).await.unwrap();
    socket
        .send(Message::Text(
            r#"{"type":"send_message","content":"Legs felt heavy today"}"#
                .to_string()
                .into(),
        ))
        .await
        .unwrap();

    let mut frames = Vec::new();
    for _ in 0..3 {
        let next = timeout(Duration::from_secs(5), socket.next()).await;
        let Ok(Some(Ok(frame))) = next else {
            break;
        };
        frames.push(frame.into_text().unwrap().to_string());
        if frames
            .iter()
            .any(|frame| frame.contains(r#""type":"coach_message""#))
        {
            break;
        }
    }

    assert!(
        frames
            .iter()
            .any(|frame| frame.contains(r#""type":"coach_typing""#)),
        "expected coach_typing frame, got {frames:?}"
    );
    assert!(
        frames
            .iter()
            .any(|frame| frame.contains(r#""type":"coach_message""#)),
        "expected coach_message frame, got {frames:?}; persisted messages: {:?}",
        service.messages("conversation-1")
    );
    let coach_message_frame = frames
        .iter()
        .find(|frame| frame.contains(r#""type":"coach_message""#))
        .expect("coach_message frame to exist");
    assert!(coach_message_frame.contains(r#""role":"coach""#));
    assert!(coach_message_frame.contains(r#""conversationId":"conversation-1""#));
    assert!(
        frames
            .iter()
            .all(|frame| !frame.contains(r#""type":"system_message""#)),
        "expected no summary system_message frame, got {frames:?}"
    );
}

#[tokio::test]
async fn websocket_streams_tool_message_before_calendar_coach_message() {
    let service = TestCalendarCoachService::default().with_tool_call(PublicToolCall {
        id: "tool-1".to_string(),
        name: "lookupCalendar".to_string(),
        arguments_json: r#"{"week":"2026-W18"}"#.to_string(),
    });
    let app =
        calendar_coach_test_app(TestIdentityServiceWithSession::default(), service.clone()).await;

    let server = SpawnedApp::start(app).await;

    let mut request = format!(
        "ws://{}/api/calendar/coach/conversations/conversation-1/ws",
        server.address
    )
    .into_client_request()
    .unwrap();
    request
        .headers_mut()
        .insert("Cookie", "aiwattcoach_session=session-1".parse().unwrap());

    let (mut socket, _) = connect_async(request).await.unwrap();
    socket
        .send(Message::Text(
            r#"{"type":"send_message","content":"Legs felt heavy today"}"#
                .to_string()
                .into(),
        ))
        .await
        .unwrap();

    let first = timeout(Duration::from_secs(1), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .into_text()
        .unwrap()
        .to_string();
    let second = timeout(Duration::from_secs(3), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .into_text()
        .unwrap()
        .to_string();
    let third = timeout(Duration::from_secs(3), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .into_text()
        .unwrap()
        .to_string();

    assert!(first.contains(r#""type":"coach_typing""#));
    assert!(second.contains(r#""type":"tool_message""#));
    assert!(second.contains(r#""role":"tool""#));
    assert!(second.contains(r#""name":"lookupCalendar""#));
    assert!(third.contains(r#""type":"coach_message""#));
}

#[tokio::test]
async fn websocket_disconnect_does_not_generate_queued_follow_up_replies() {
    let service =
        TestCalendarCoachService::default().with_coach_reply_delay(Duration::from_millis(250));
    let app =
        calendar_coach_test_app(TestIdentityServiceWithSession::default(), service.clone()).await;

    let server = SpawnedApp::start(app).await;

    let mut request = format!(
        "ws://{}/api/calendar/coach/conversations/conversation-1/ws",
        server.address
    )
    .into_client_request()
    .unwrap();
    request
        .headers_mut()
        .insert("Cookie", "aiwattcoach_session=session-1".parse().unwrap());

    let (mut socket, _) = connect_async(request).await.unwrap();
    socket
        .send(Message::Text(
            r#"{"type":"send_message","content":"First"}"#.to_string().into(),
        ))
        .await
        .unwrap();
    socket
        .send(Message::Text(
            r#"{"type":"send_message","content":"Second"}"#.to_string().into(),
        ))
        .await
        .unwrap();

    let first_frame = timeout(Duration::from_secs(5), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .into_text()
        .unwrap()
        .to_string();

    let first_payload: Value = serde_json::from_str(&first_frame).unwrap();
    assert_ne!(
        first_payload.get("type").and_then(Value::as_str),
        Some("error")
    );

    socket.close(None).await.unwrap();

    timeout(Duration::from_secs(2), async {
        loop {
            let messages = service.messages("conversation-1");
            let processed = service.processed_user_messages();

            if messages.len() >= 2 || processed.len() >= 2 {
                break;
            }

            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap();

    let messages = service.messages("conversation-1");
    assert_eq!(service.processed_user_messages(), vec!["First".to_string()]);
    assert_eq!(messages.len(), 2);
    assert_eq!(
        messages[0].role,
        aiwattcoach::domain::coach_conversation::CoachConversationMessageRole::User
    );
    assert_eq!(messages[0].content, "First");
    assert_eq!(
        messages[1].role,
        aiwattcoach::domain::coach_conversation::CoachConversationMessageRole::Coach
    );
    assert_eq!(messages[1].content, "Coach reply to: First");
}
