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

fn lock_telemetry_env() -> std::sync::MutexGuard<'static, ()> {
    telemetry_env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[tokio::test(flavor = "current_thread")]
async fn setup_telemetry_accepts_service_name_override_without_otlp_endpoint() {
    let _guard = lock_telemetry_env();
    let original_endpoint = env::var_os("OTEL_EXPORTER_OTLP_ENDPOINT");
    let original_service_name = env::var_os("OTEL_SERVICE_NAME");

    env::remove_var("OTEL_EXPORTER_OTLP_ENDPOINT");
    env::set_var("OTEL_SERVICE_NAME", "telemetry-smoke-test");

    let setup_result = setup_telemetry("fallback-service");

    restore_env_var("OTEL_EXPORTER_OTLP_ENDPOINT", original_endpoint);
    restore_env_var("OTEL_SERVICE_NAME", original_service_name);

    let mut telemetry = setup_result
        .expect("telemetry setup should accept OTEL_SERVICE_NAME without OTLP exporters enabled");

    telemetry
        .shutdown()
        .expect("telemetry shutdown should succeed during smoke test");
}

#[tokio::test(flavor = "current_thread")]
async fn setup_telemetry_rejects_malformed_otlp_endpoint() {
    let _guard = lock_telemetry_env();
    let original_endpoint = env::var_os("OTEL_EXPORTER_OTLP_ENDPOINT");
    let original_service_name = env::var_os("OTEL_SERVICE_NAME");

    env::set_var("OTEL_EXPORTER_OTLP_ENDPOINT", "not a valid otlp endpoint");
    env::set_var("OTEL_SERVICE_NAME", "telemetry-smoke-test");

    let setup_result = setup_telemetry("fallback-service");

    restore_env_var("OTEL_EXPORTER_OTLP_ENDPOINT", original_endpoint);
    restore_env_var("OTEL_SERVICE_NAME", original_service_name);

    assert!(
        setup_result.is_err(),
        "malformed OTLP endpoint should fail setup so env wiring is exercised"
    );
}

fn restore_env_var(key: &str, value: Option<OsString>) {
    match value {
        Some(value) => env::set_var(key, value),
        None => env::remove_var(key),
    }
}
