use std::{
    env,
    ffi::OsString,
    sync::{Mutex, OnceLock},
};

use aiwattcoach::telemetry::setup_telemetry;

fn telemetry_env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[tokio::test(flavor = "multi_thread")]
async fn setup_telemetry_accepts_otlp_env_configuration() {
    let _guard = telemetry_env_lock()
        .lock()
        .expect("telemetry env lock should not be poisoned");
    let original_endpoint = env::var_os("OTEL_EXPORTER_OTLP_ENDPOINT");
    let original_service_name = env::var_os("OTEL_SERVICE_NAME");

    env::set_var("OTEL_EXPORTER_OTLP_ENDPOINT", "http://127.0.0.1:4317");
    env::set_var("OTEL_SERVICE_NAME", "telemetry-smoke-test");

    let setup_result = setup_telemetry("fallback-service");

    restore_env_var("OTEL_EXPORTER_OTLP_ENDPOINT", original_endpoint);
    restore_env_var("OTEL_SERVICE_NAME", original_service_name);

    let mut telemetry = setup_result
        .expect("telemetry setup should accept OTLP endpoint and service name env vars");

    telemetry
        .shutdown()
        .expect("telemetry shutdown should succeed during smoke test");
}

fn restore_env_var(key: &str, value: Option<OsString>) {
    match value {
        Some(value) => env::set_var(key, value),
        None => env::remove_var(key),
    }
}
