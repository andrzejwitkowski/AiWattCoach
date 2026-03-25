use std::{error::Error, future::Future, net::SocketAddr, sync::Arc, time::Duration};

use aiwattcoach::{
    adapters::{
        google_oauth::client::GoogleOAuthClient,
        intervals_icu::{client::IntervalsIcuClient, settings_adapter::SettingsIntervalsProvider},
        mongo::{
            activities::MongoActivityRepository,
            client::{create_client, ensure_database_exists, verify_connection},
            login_state::MongoLoginStateRepository,
            sessions::MongoSessionRepository,
            settings::MongoUserSettingsRepository,
            users::MongoUserRepository,
        },
        support::{SystemClock, UuidIdGenerator},
    },
    build_app,
    config::Settings,
    domain::identity::{
        validate_session_ttl_against_current_time, Clock, IdentityService, IdentityServiceConfig,
    },
    domain::intervals::IntervalsService,
    domain::settings::UserSettingsService,
    telemetry::init_telemetry,
    AppState,
};
use tokio::net::TcpListener;
use tokio::sync::Notify;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let settings = Settings::from_env()?;
    let Settings {
        app_name,
        server,
        mongo,
        auth,
        client_log_ingestion_enabled,
    } = settings;
    let mut telemetry = init_telemetry(&app_name)?;
    let address: SocketAddr = server.address().parse()?;
    let mongo_client = create_client(&mongo.uri).await?;
    ensure_database_exists(&mongo_client, &mongo.database).await?;
    verify_connection(&mongo_client, &mongo.database, Duration::from_secs(5)).await?;

    let mongo_database = mongo.database.clone();
    let user_repository = MongoUserRepository::new(mongo_client.clone(), &mongo_database);
    let session_repository = MongoSessionRepository::new(mongo_client.clone(), &mongo_database);
    let login_state_repository =
        MongoLoginStateRepository::new(mongo_client.clone(), &mongo_database);
    user_repository.ensure_indexes().await?;
    session_repository.ensure_indexes().await?;
    login_state_repository.ensure_indexes().await?;
    let google_oauth_client = GoogleOAuthClient::new(
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .build()?,
        auth.google.client_id,
        auth.google.client_secret,
        auth.google.redirect_url,
    );
    validate_session_ttl_against_current_time(
        SystemClock.now_epoch_seconds(),
        auth.session.ttl_hours,
    )?;
    let identity_service = IdentityService::new(
        user_repository,
        session_repository,
        login_state_repository,
        google_oauth_client,
        SystemClock,
        UuidIdGenerator,
        IdentityServiceConfig::new(auth.admin_emails, auth.session.ttl_hours),
    );

    let settings_repository =
        MongoUserSettingsRepository::new(mongo_client.clone(), &mongo_database);
    settings_repository.ensure_indexes().await?;
    let settings_service = Arc::new(UserSettingsService::new(settings_repository, SystemClock));
    let activity_repository = MongoActivityRepository::new(mongo_client.clone(), &mongo_database);
    activity_repository.ensure_indexes().await?;
    let intervals_api_client = IntervalsIcuClient::with_timeouts(10, 30)?;
    let intervals_settings_provider = SettingsIntervalsProvider::new(settings_service.clone());
    let intervals_service = Arc::new(IntervalsService::new(
        intervals_api_client,
        intervals_settings_provider,
        activity_repository,
    ));

    let intervals_connection_tester = IntervalsIcuClient::with_timeouts(5, 15)?;

    let app = build_app(
        AppState::new(app_name, mongo_database, mongo_client)
            .with_client_log_ingestion(client_log_ingestion_enabled)
            .with_identity_service(
                Arc::new(identity_service),
                auth.session.cookie_name,
                auth.session.same_site,
                auth.session.secure,
                auth.session.ttl_hours,
            )
            .with_settings_service(settings_service)
            .with_intervals_service(intervals_service)
            .with_intervals_connection_tester(Arc::new(intervals_connection_tester)),
    );
    let listener = TcpListener::bind(address).await?;

    let serve_result = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await;
    let telemetry_shutdown_result = telemetry.shutdown();

    serve_result?;
    telemetry_shutdown_result?;

    Ok(())
}

async fn shutdown_signal() {
    let shutdown = Arc::new(Notify::new());
    let ctrl_c = wait_for_ctrl_c(tokio::signal::ctrl_c(), shutdown.clone());

    #[cfg(unix)]
    let terminate = wait_for_sigterm(
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()),
        shutdown,
    );

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

async fn wait_for_ctrl_c<F>(ctrl_c: F, shutdown: Arc<Notify>)
where
    F: Future<Output = std::io::Result<()>>,
{
    match ctrl_c.await {
        Ok(()) => shutdown.notify_waiters(),
        Err(error) => {
            tracing::error!(%error, "Failed to listen for Ctrl+C");
            shutdown.notified().await;
        }
    }
}

#[cfg(unix)]
async fn wait_for_sigterm(
    signal: std::io::Result<tokio::signal::unix::Signal>,
    shutdown: Arc<Notify>,
) {
    match signal {
        Ok(mut signal) => {
            signal.recv().await;
            shutdown.notify_waiters();
        }
        Err(error) => {
            tracing::error!(%error, "Failed to listen for SIGTERM");
            shutdown.notified().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Error as IoError, Write},
        sync::{Arc, Mutex},
    };

    use super::wait_for_ctrl_c;
    #[cfg(unix)]
    use super::wait_for_sigterm;
    use tokio::sync::Notify;
    use tokio::time::{timeout, Duration};

    #[derive(Clone, Default)]
    struct SharedLogBuffer(Arc<Mutex<Vec<u8>>>);

    impl SharedLogBuffer {
        fn contents(&self) -> String {
            String::from_utf8(self.0.lock().expect("log buffer mutex poisoned").clone())
                .expect("log buffer contained invalid utf-8")
        }
    }

    impl Write for SharedLogBuffer {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .expect("log buffer mutex poisoned")
                .extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SharedLogBuffer {
        type Writer = SharedLogBuffer;

        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn ctrl_c_registration_error_logs_and_does_not_finish_shutdown_future() {
        let logs = SharedLogBuffer::default();
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .without_time()
            .with_target(false)
            .with_writer(logs.clone())
            .finish();
        let _default = tracing::subscriber::set_default(subscriber);

        let result = timeout(
            Duration::from_millis(50),
            wait_for_ctrl_c(
                async { Err(IoError::other("boom")) },
                Arc::new(Notify::new()),
            ),
        )
        .await;

        assert!(result.is_err());
        let output = logs.contents();
        assert!(output.contains("Failed to listen for Ctrl+C"));
        assert!(output.contains("boom"));
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "current_thread")]
    async fn sigterm_registration_error_logs_and_does_not_finish_shutdown_future() {
        let logs = SharedLogBuffer::default();
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .without_time()
            .with_target(false)
            .with_writer(logs.clone())
            .finish();
        let _default = tracing::subscriber::set_default(subscriber);

        let result = timeout(
            Duration::from_millis(50),
            wait_for_sigterm(Err(IoError::other("boom")), Arc::new(Notify::new())),
        )
        .await;

        assert!(result.is_err());
        let output = logs.contents();
        assert!(output.contains("Failed to listen for SIGTERM"));
        assert!(output.contains("boom"));
    }
}
