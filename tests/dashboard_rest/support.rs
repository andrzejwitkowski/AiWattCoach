use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    sync::{Arc, Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use aiwattcoach::{
    build_app_with_frontend_dist,
    config::AppState,
    domain::{
        identity::IdentityUseCases,
        training_load::{
            FtpSource, TrainingLoadDailySnapshot, TrainingLoadDailySnapshotRepository,
            TrainingLoadDashboardReadService,
        },
    },
    Settings,
};
use mongodb::Client;

static FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);
static KEPT_FRONTEND_FIXTURES: OnceLock<Mutex<Vec<FrontendFixture>>> = OnceLock::new();

pub(crate) async fn dashboard_test_app(
    identity_service: impl IdentityUseCases + 'static,
    snapshots: impl TrainingLoadDailySnapshotRepository + 'static,
) -> axum::Router {
    let settings = Settings::test_defaults();
    let app_name = settings.app_name.clone();
    let mongo_database = settings.mongo.database.clone();
    let mongo_uri = settings.mongo.uri.clone();
    let session_name = settings.auth.session.cookie_name.clone();
    let session_same_site = settings.auth.session.same_site.clone();
    let session_secure = settings.auth.session.secure;
    let session_ttl_hours = settings.auth.session.ttl_hours;
    let fixture = frontend_fixture();
    let dist_dir = fixture.dist_dir();
    keep_frontend_fixture(fixture);
    let dashboard_service = Arc::new(TrainingLoadDashboardReadService::new(snapshots));

    build_app_with_frontend_dist(
        AppState::new(
            app_name,
            mongo_database,
            test_mongo_client(&mongo_uri).await,
        )
        .with_identity_service(
            Arc::new(identity_service),
            session_name,
            session_same_site,
            session_secure,
            session_ttl_hours,
        )
        .with_training_load_dashboard_service(dashboard_service),
        dist_dir,
    )
}

pub(crate) fn sample_snapshot(
    user_id: &str,
    date: &str,
    daily_tss: Option<i32>,
    ctl: Option<f64>,
    atl: Option<f64>,
    tsb: Option<f64>,
) -> TrainingLoadDailySnapshot {
    TrainingLoadDailySnapshot {
        user_id: user_id.to_string(),
        date: date.to_string(),
        daily_tss,
        rolling_tss_7d: None,
        rolling_tss_28d: None,
        ctl,
        atl,
        tsb,
        average_if_28d: None,
        average_ef_28d: None,
        ftp_effective_watts: Some(340),
        ftp_source: Some(FtpSource::Settings),
        recomputed_at_epoch_seconds: 100,
        created_at_epoch_seconds: 100,
        updated_at_epoch_seconds: 100,
    }
}

struct FrontendFixture {
    root: PathBuf,
}

fn frontend_fixture() -> FrontendFixture {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let counter = FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "aiwattcoach-dashboard-spa-fixture-{}-{unique}-{counter}",
        std::process::id()
    ));
    let dist_dir = root.join("dist");
    fs::create_dir_all(&dist_dir).unwrap();
    fs::write(
        dist_dir.join("index.html"),
        "<!doctype html><html><body><div id=\"root\">fixture</div></body></html>",
    )
    .unwrap();

    FrontendFixture { root }
}

impl Drop for FrontendFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl FrontendFixture {
    fn dist_dir(&self) -> PathBuf {
        self.root.join("dist")
    }
}

fn keep_frontend_fixture(fixture: FrontendFixture) {
    KEPT_FRONTEND_FIXTURES
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap()
        .push(fixture);
}

async fn test_mongo_client(uri: &str) -> Client {
    Client::with_uri_str(uri)
        .await
        .expect("test mongo client should be created")
}
