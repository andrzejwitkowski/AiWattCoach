use std::{
    future::Future,
    io::Write,
    sync::{Arc, Mutex, OnceLock},
};

static TEST_TRACING_INIT: OnceLock<()> = OnceLock::new();
static TRACE_CAPTURE_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
static ACTIVE_LOG_BUFFER: OnceLock<Mutex<Option<SharedLogBuffer>>> = OnceLock::new();

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

#[derive(Clone, Default)]
struct GlobalLogRouter;

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for GlobalLogRouter {
    type Writer = GlobalLogWriter;

    fn make_writer(&'a self) -> Self::Writer {
        GlobalLogWriter
    }
}

struct GlobalLogWriter;

impl Write for GlobalLogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let active_buffer = ACTIVE_LOG_BUFFER.get_or_init(|| Mutex::new(None));
        let mut guard = active_buffer
            .lock()
            .expect("active log buffer mutex poisoned");

        if let Some(buffer) = guard.as_mut() {
            buffer.write(buf)
        } else {
            Ok(buf.len())
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct ActiveLogBufferGuard;

impl ActiveLogBufferGuard {
    fn install(buffer: SharedLogBuffer) -> Self {
        let active_buffer = ACTIVE_LOG_BUFFER.get_or_init(|| Mutex::new(None));
        *active_buffer
            .lock()
            .expect("active log buffer mutex poisoned") = Some(buffer);
        Self
    }
}

impl Drop for ActiveLogBufferGuard {
    fn drop(&mut self) {
        let active_buffer = ACTIVE_LOG_BUFFER.get_or_init(|| Mutex::new(None));
        *active_buffer
            .lock()
            .expect("active log buffer mutex poisoned") = None;
    }
}

pub async fn capture_tracing_logs<F, Fut, T>(run: F) -> (T, String)
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = T>,
{
    let _capture_guard = TRACE_CAPTURE_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    init_test_tracing_subscriber();
    let logs = SharedLogBuffer::default();
    let _active_buffer = ActiveLogBufferGuard::install(logs.clone());
    let output = run().await;
    // Drop the active buffer guard first so concurrent work finishes writing
    drop(_active_buffer);
    let captured = logs.contents();

    (output, captured)
}

fn init_test_tracing_subscriber() {
    TEST_TRACING_INIT.get_or_init(|| {
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_ansi(false)
            .without_time()
            .with_target(false)
            .with_current_span(true)
            .with_span_list(true)
            .with_writer(GlobalLogRouter)
            .finish();

        tracing::subscriber::set_global_default(subscriber)
            .expect("test tracing subscriber should install once");
    });
}
