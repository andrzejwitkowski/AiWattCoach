use std::{error::Error, future::Future, net::SocketAddr, sync::Arc, time::Duration};

use aiwattcoach::{
    adapters::{
        google_oauth::client::GoogleOAuthClient,
        mongo::{
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
    domain::settings::UserSettingsService,
    AppState,
};
use tokio::net::TcpListener;
use tokio::sync::Notify;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let settings = Settings::from_env()?;
    let Settings {
        app_name,
        server,
        mongo,
        auth,
    } = settings;
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
    let settings_service = UserSettingsService::new(settings_repository, SystemClock);

    let app = build_app(
        AppState::new(app_name, mongo_database, mongo_client)
            .with_identity_service(
                Arc::new(identity_service),
                auth.session.cookie_name,
                auth.session.same_site,
                auth.session.secure,
                auth.session.ttl_hours,
            )
            .with_settings_service(Arc::new(settings_service)),
    );
    let listener = TcpListener::bind(address).await?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

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
            eprintln!("Failed to listen for Ctrl+C: {error}");
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
            eprintln!("Failed to listen for SIGTERM: {error}");
            shutdown.notified().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Error as IoError;

    use super::wait_for_ctrl_c;
    #[cfg(unix)]
    use super::wait_for_sigterm;
    use std::sync::Arc;
    use tokio::sync::Notify;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn ctrl_c_registration_error_does_not_finish_shutdown_future() {
        let result = timeout(
            Duration::from_millis(50),
            wait_for_ctrl_c(
                async { Err(IoError::other("boom")) },
                Arc::new(Notify::new()),
            ),
        )
        .await;

        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn sigterm_registration_error_does_not_finish_shutdown_future() {
        let result = timeout(
            Duration::from_millis(50),
            wait_for_sigterm(Err(IoError::other("boom")), Arc::new(Notify::new())),
        )
        .await;

        assert!(result.is_err());
    }
}
