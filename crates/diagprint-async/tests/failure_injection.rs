use diagprint::{Diagnostic, DiagnosticSink, Reporter, SinkError, SinkErrorKind, SinkResult};
use diagprint_async::{AsyncDiagnosticSink, AsyncSinkError, BackpressurePolicy, SubmitOutcome};

const EMIT_FAILURE: &str = "injected async sink emit failure";

const FLUSH_FAILURE: &str = "injected async sink flush failure";

struct EmitFailureSink;

impl DiagnosticSink for EmitFailureSink {
    fn emit(&self, _diagnostic: &Diagnostic) -> SinkResult<()> {
        Err(SinkError::new(SinkErrorKind::Io, EMIT_FAILURE))
    }
}

struct FlushFailureSink;

impl DiagnosticSink for FlushFailureSink {
    fn emit(&self, _diagnostic: &Diagnostic) -> SinkResult<()> {
        Ok(())
    }

    fn flush(&self) -> SinkResult<()> {
        Err(SinkError::new(SinkErrorKind::Io, FLUSH_FAILURE))
    }
}

struct PanicSink;

impl DiagnosticSink for PanicSink {
    fn emit(&self, _diagnostic: &Diagnostic) -> SinkResult<()> {
        panic!("injected async sink worker panic");
    }
}

fn reporter() -> Reporter {
    Reporter::builder()
        .application("diagprint-async-failure-injection")
        .build()
        .expect("test reporter should build")
}

fn assert_worker_error(error: &AsyncSinkError, expected_message: &str) {
    match error {
        AsyncSinkError::Worker { error } => {
            assert_eq!(error.kind(), SinkErrorKind::Io);

            assert_eq!(error.message(), expected_message);
        }

        other => {
            panic!("expected worker error, got {other:?}");
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn emit_failure_is_sticky_and_shutdown_reports_it() {
    let sink = AsyncDiagnosticSink::spawn(EmitFailureSink, 8, BackpressurePolicy::Block)
        .expect("async diagnostic sink should spawn");

    assert_eq!(
        sink.emit(reporter().error("first failure-injection diagnostic",),)
            .await
            .expect("queue acceptance precedes worker delivery",),
        SubmitOutcome::Enqueued
    );

    let flush_error = sink
        .flush()
        .await
        .expect_err("worker emit failure must fail flush");

    assert_worker_error(&flush_error, EMIT_FAILURE);

    let later_error = sink
        .emit(reporter().error("later diagnostic"))
        .await
        .expect_err("worker failure must remain sticky");

    assert_worker_error(&later_error, EMIT_FAILURE);

    let shutdown_error = sink
        .shutdown()
        .await
        .expect_err("shutdown must retain worker failure");

    assert_worker_error(&shutdown_error, EMIT_FAILURE);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn flush_failure_is_sticky_and_blocks_future_submission() {
    let sink = AsyncDiagnosticSink::spawn(FlushFailureSink, 8, BackpressurePolicy::Block)
        .expect("async diagnostic sink should spawn");

    assert_eq!(
        sink.emit(reporter().warning("diagnostic before flush failure",),)
            .await
            .expect("diagnostic should enter queue",),
        SubmitOutcome::Enqueued
    );

    let flush_error = sink
        .flush()
        .await
        .expect_err("injected flush failure must surface");

    assert_worker_error(&flush_error, FLUSH_FAILURE);

    let later_error = sink
        .emit(reporter().warning("diagnostic after flush failure"))
        .await
        .expect_err("failed worker must reject later submissions");

    assert_worker_error(&later_error, FLUSH_FAILURE);

    let shutdown_error = sink
        .shutdown()
        .await
        .expect_err("shutdown must preserve flush failure");

    assert_worker_error(&shutdown_error, FLUSH_FAILURE);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn worker_panic_closes_submission_and_surfaces_join_failure() {
    let sink = AsyncDiagnosticSink::spawn(PanicSink, 8, BackpressurePolicy::Block)
        .expect("async diagnostic sink should spawn");

    assert_eq!(
        sink.emit(reporter().error("diagnostic that triggers worker panic",),)
            .await
            .expect("queue acceptance does not imply delivery success",),
        SubmitOutcome::Enqueued
    );

    let closed = sink
        .flush()
        .await
        .expect_err("panicked worker must close the queue");

    assert_eq!(closed, AsyncSinkError::Closed);

    let later = sink
        .emit(reporter().error("diagnostic after worker panic"))
        .await
        .expect_err("closed worker must reject later submission");

    assert_eq!(later, AsyncSinkError::Closed);

    let shutdown = sink
        .shutdown()
        .await
        .expect_err("worker panic must surface at lifecycle join");

    assert!(
        matches!(shutdown, AsyncSinkError::Join { .. }),
        "expected Join error after worker panic, got {shutdown:?}"
    );
}
