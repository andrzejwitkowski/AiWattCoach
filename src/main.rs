use std::{error::Error, net::SocketAddr, time::Duration};

use aiwattcoach::{
    adapters::mongo::client::{create_client, verify_connection},
    build_app,
    config::Settings,
    AppState,
};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let settings = Settings::from_env()?;
    let Settings {
        app_name,
        server,
        mongo,
    } = settings;
    let address: SocketAddr = server.address().parse()?;
    let mongo_client = create_client(&mongo.uri).await?;
    verify_connection(&mongo_client, &mongo.database, Duration::from_secs(5)).await?;

    let app = build_app(AppState::new(app_name, mongo.database, mongo_client));
    let listener = TcpListener::bind(address).await?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            let _ = signal.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
