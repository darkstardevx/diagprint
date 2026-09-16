use diagprint::{Diagnostic, DiagnosticSink, Reporter, Severity, SinkResult};
use diagprint_async::{AsyncDiagnosticSink, AsyncSinkError, BackpressurePolicy, SubmitOutcome};
use std::sync::{
    Arc, Barrier, Condvar, Mutex,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Clone, Default)]
struct RecordingState {
    messages: Arc<Mutex<Vec<String>>>,

    flushes: Arc<AtomicUsize>,
}

struct RecordingSink {
    state: RecordingState,
}

impl DiagnosticSink for RecordingSink {
    fn emit(&self, diagnostic: &Diagnostic) -> SinkResult<()> {
        self.state
            .messages
            .lock()
            .unwrap()
            .push(diagnostic.message.clone());

        Ok(())
    }

    fn flush(&self) -> SinkResult<()> {
        self.state.flushes.fetch_add(1, Ordering::SeqCst);

        Ok(())
    }
}

struct GateSink {
    calls: AtomicUsize,

    entered: Arc<Barrier>,

    release: Arc<(Mutex<bool>, Condvar)>,
}

impl DiagnosticSink for GateSink {
    fn emit(&self, _: &Diagnostic) -> SinkResult<()> {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            self.entered.wait();

            let (lock, condition) = &*self.release;

            let mut released = lock.lock().unwrap();

            while !*released {
                released = condition.wait(released).unwrap();
            }
        }

        Ok(())
    }
}

fn reporter() -> Reporter {
    Reporter::builder()
        .application("async-test")
        .build()
        .unwrap()
}

fn release_gate(release: &Arc<(Mutex<bool>, Condvar)>) {
    let (lock, condition) = &**release;

    *lock.lock().unwrap() = true;

    condition.notify_all();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queued_diagnostics_are_delivered_and_flushed() {
    let reporter = reporter();

    let state = RecordingState::default();

    let sink = RecordingSink {
        state: state.clone(),
    };

    let async_sink = AsyncDiagnosticSink::spawn(sink, 8, BackpressurePolicy::Block).unwrap();

    assert_eq!(
        async_sink.emit(reporter.info("one"),).await.unwrap(),
        SubmitOutcome::Enqueued
    );

    assert_eq!(
        async_sink.emit(reporter.warning("two"),).await.unwrap(),
        SubmitOutcome::Enqueued
    );

    async_sink.flush().await.unwrap();

    assert_eq!(state.messages.lock().unwrap().as_slice(), ["one", "two"]);

    assert_eq!(state.flushes.load(Ordering::SeqCst,), 1);

    async_sink.shutdown().await.unwrap();

    assert_eq!(state.flushes.load(Ordering::SeqCst,), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reject_policy_reports_full_queue() {
    let reporter = reporter();

    let entered = Arc::new(Barrier::new(2));

    let release = Arc::new((Mutex::new(false), Condvar::new()));

    let async_sink = AsyncDiagnosticSink::spawn(
        GateSink {
            calls: AtomicUsize::new(0),

            entered: entered.clone(),

            release: release.clone(),
        },
        1,
        BackpressurePolicy::Reject,
    )
    .unwrap();

    async_sink.emit(reporter.info("one")).await.unwrap();

    tokio::task::spawn_blocking(move || {
        entered.wait();
    })
    .await
    .unwrap();

    async_sink.emit(reporter.info("two")).await.unwrap();

    let error = async_sink
        .emit(reporter.warning("three"))
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        AsyncSinkError::QueueFull {
            severity: Severity::Warning,
        }
    ));

    release_gate(&release);

    async_sink.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn drop_newest_can_protect_errors() {
    let reporter = reporter();

    let entered = Arc::new(Barrier::new(2));

    let release = Arc::new((Mutex::new(false), Condvar::new()));

    let async_sink = AsyncDiagnosticSink::spawn(
        GateSink {
            calls: AtomicUsize::new(0),

            entered: entered.clone(),

            release: release.clone(),
        },
        1,
        BackpressurePolicy::DropNewest {
            up_to: Severity::Warning,
        },
    )
    .unwrap();

    async_sink.emit(reporter.info("one")).await.unwrap();

    tokio::task::spawn_blocking(move || {
        entered.wait();
    })
    .await
    .unwrap();

    async_sink.emit(reporter.info("two")).await.unwrap();

    assert_eq!(
        async_sink
            .emit(reporter.warning("droppable",),)
            .await
            .unwrap(),
        SubmitOutcome::Dropped
    );

    let error = async_sink
        .emit(reporter.error("protected"))
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        AsyncSinkError::QueueFull {
            severity: Severity::Error,
        }
    ));

    release_gate(&release);

    async_sink.shutdown().await.unwrap();
}
