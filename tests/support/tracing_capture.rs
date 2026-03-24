use std::{
    collections::HashMap,
    future::Future,
    io::Write,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
};

static TEST_TRACING_INIT: OnceLock<()> = OnceLock::new();
static TRACE_CAPTURE_ID: AtomicU64 = AtomicU64::new(1);
static ACTIVE_LOG_BUFFERS: OnceLock<Mutex<HashMap<String, SharedLogBuffer>>> = OnceLock::new();

tokio::task_local! {
    static CURRENT_CAPTURE_ID: String;
}

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
        let Some(capture_id) = CURRENT_CAPTURE_ID.try_with(Clone::clone).ok() else {
            return Ok(buf.len());
        };

        let active_buffers = ACTIVE_LOG_BUFFERS.get_or_init(|| Mutex::new(HashMap::new()));
        let mut guard = active_buffers
            .lock()
            .expect("active log buffers mutex poisoned");

        if let Some(buffer) = guard.get_mut(&capture_id) {
            buffer.write(buf)
        } else {
            Ok(buf.len())
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct ActiveLogBufferGuard {
    capture_id: String,
}

impl ActiveLogBufferGuard {
    fn install(capture_id: String, buffer: SharedLogBuffer) -> Self {
        let active_buffers = ACTIVE_LOG_BUFFERS.get_or_init(|| Mutex::new(HashMap::new()));
        active_buffers
            .lock()
            .expect("active log buffers mutex poisoned")
            .insert(capture_id.clone(), buffer);
        Self { capture_id }
    }
}

impl Drop for ActiveLogBufferGuard {
    fn drop(&mut self) {
        let active_buffers = ACTIVE_LOG_BUFFERS.get_or_init(|| Mutex::new(HashMap::new()));
        active_buffers
            .lock()
            .expect("active log buffers mutex poisoned")
            .remove(&self.capture_id);
    }
}

pub async fn capture_tracing_logs<F, Fut, T>(run: F) -> (T, String)
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = T>,
{
    init_test_tracing_subscriber();
    let logs = SharedLogBuffer::default();
    let capture_id = format!(
        "capture-{}",
        TRACE_CAPTURE_ID.fetch_add(1, Ordering::Relaxed)
    );
    let _active_buffer = ActiveLogBufferGuard::install(capture_id.clone(), logs.clone());
    let output = CURRENT_CAPTURE_ID.scope(capture_id, run()).await;
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
