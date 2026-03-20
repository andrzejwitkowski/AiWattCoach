use std::{error::Error, net::SocketAddr};

use aiwattcoach::config::{build_app, create_mongo_client, AppState, Settings};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let settings = Settings::from_env()?;
    let mongo_client = create_mongo_client(&settings).await?;
    let app = build_app(AppState::new(settings.clone(), mongo_client));

    let address: SocketAddr = settings.server.address().parse()?;
    let listener = TcpListener::bind(address).await?;

    axum::serve(listener, app).await?;

    Ok(())
}
