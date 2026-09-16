//! Bounded asynchronous diagnostic delivery for diagprint.
//!
//! Queues are always bounded and overload behavior is always explicit.
//! The synchronous `diagprint` crate remains independent of any async runtime.

use diagprint::{Diagnostic, DiagnosticReport, DiagnosticSink, Severity, SinkError, SinkResult};
use std::{
    error::Error,
    fmt,
    sync::{Arc, Mutex},
};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

/// Behavior when the bounded delivery queue reaches capacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackpressurePolicy {
    /// Wait asynchronously until queue capacity becomes available.
    #[default]
    Block,

    /// Reject the new diagnostic immediately when the queue is full.
    Reject,

    /// Drop the newest diagnostic only when its severity is less than or equal
    /// to `up_to`.
    ///
    /// Diagnostics above the configured threshold are rejected instead of
    /// silently discarded. Setting `up_to` to `Fatal` explicitly permits all
    /// severities to be dropped under overload.
    DropNewest { up_to: Severity },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmitOutcome {
    Enqueued,
    Dropped,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SubmissionSummary {
    pub enqueued: usize,
    pub dropped: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsyncSinkError {
    NoRuntime,

    InvalidCapacity,

    QueueFull { severity: Severity },

    Closed,

    Worker { error: SinkError },

    Join { message: String },
}

impl fmt::Display for AsyncSinkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRuntime => {
                formatter.write_str("diagprint-async requires an active Tokio runtime")
            }

            Self::InvalidCapacity => {
                formatter.write_str("async diagnostic queue capacity must be greater than zero")
            }

            Self::QueueFull { severity } => {
                write!(
                    formatter,
                    "async diagnostic queue is full; {severity} diagnostic was not accepted"
                )
            }

            Self::Closed => formatter.write_str("async diagnostic worker is closed"),

            Self::Worker { error } => {
                write!(formatter, "async diagnostic worker failed: {error}")
            }

            Self::Join { message } => {
                write!(formatter, "async diagnostic worker join failed: {message}")
            }
        }
    }
}

impl Error for AsyncSinkError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Worker { error } => Some(error),

            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct ReportSubmitError {
    pub error: AsyncSinkError,
    pub submitted: SubmissionSummary,
}

impl fmt::Display for ReportSubmitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} after {} enqueued and {} dropped diagnostics",
            self.error, self.submitted.enqueued, self.submitted.dropped,
        )
    }
}

impl Error for ReportSubmitError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.error)
    }
}

enum Command {
    Diagnostic(Box<Diagnostic>),

    Flush(oneshot::Sender<SinkResult<()>>),

    Shutdown(oneshot::Sender<SinkResult<()>>),
}

#[derive(Clone, Default)]
struct WorkerState {
    failure: Arc<Mutex<Option<SinkError>>>,
}

impl WorkerState {
    fn failure(&self) -> Option<SinkError> {
        self.failure
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn fail(&self, error: SinkError) {
        let mut failure = self
            .failure
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        if failure.is_none() {
            *failure = Some(error);
        }
    }
}

/// Bounded asynchronous front-end for a synchronous [`DiagnosticSink`].
///
/// Diagnostics are moved to a blocking worker thread. The queue never grows
/// beyond its configured capacity.
pub struct AsyncDiagnosticSink {
    sender: mpsc::Sender<Command>,

    worker: JoinHandle<SinkResult<()>>,

    state: WorkerState,

    backpressure: BackpressurePolicy,
}

impl AsyncDiagnosticSink {
    pub fn spawn<S>(
        sink: S,
        capacity: usize,
        backpressure: BackpressurePolicy,
    ) -> Result<Self, AsyncSinkError>
    where
        S: DiagnosticSink + 'static,
    {
        if capacity == 0 {
            return Err(AsyncSinkError::InvalidCapacity);
        }

        let runtime =
            tokio::runtime::Handle::try_current().map_err(|_| AsyncSinkError::NoRuntime)?;

        let (sender, receiver) = mpsc::channel(capacity);

        let state = WorkerState::default();

        let worker_state = state.clone();

        let sink: Box<dyn DiagnosticSink> = Box::new(sink);

        let worker = runtime.spawn_blocking(move || worker_loop(receiver, sink, worker_state));

        Ok(Self {
            sender,
            worker,
            state,
            backpressure,
        })
    }

    pub const fn backpressure(&self) -> BackpressurePolicy {
        self.backpressure
    }

    pub fn capacity(&self) -> usize {
        self.sender.capacity()
    }

    pub fn max_capacity(&self) -> usize {
        self.sender.max_capacity()
    }

    pub async fn emit(&self, diagnostic: Diagnostic) -> Result<SubmitOutcome, AsyncSinkError> {
        if let Some(error) = self.state.failure() {
            return Err(AsyncSinkError::Worker { error });
        }

        let severity = diagnostic.severity;

        let command = Command::Diagnostic(Box::new(diagnostic));

        match self.backpressure {
            BackpressurePolicy::Block => {
                self.sender
                    .send(command)
                    .await
                    .map_err(|_| self.closed_error())?;

                Ok(SubmitOutcome::Enqueued)
            }

            BackpressurePolicy::Reject => match self.sender.try_send(command) {
                Ok(()) => Ok(SubmitOutcome::Enqueued),

                Err(mpsc::error::TrySendError::Full(_)) => {
                    Err(AsyncSinkError::QueueFull { severity })
                }

                Err(mpsc::error::TrySendError::Closed(_)) => Err(self.closed_error()),
            },

            BackpressurePolicy::DropNewest { up_to } => match self.sender.try_send(command) {
                Ok(()) => Ok(SubmitOutcome::Enqueued),

                Err(mpsc::error::TrySendError::Full(_)) if severity <= up_to => {
                    Ok(SubmitOutcome::Dropped)
                }

                Err(mpsc::error::TrySendError::Full(_)) => {
                    Err(AsyncSinkError::QueueFull { severity })
                }

                Err(mpsc::error::TrySendError::Closed(_)) => Err(self.closed_error()),
            },
        }
    }

    pub async fn emit_ref(&self, diagnostic: &Diagnostic) -> Result<SubmitOutcome, AsyncSinkError> {
        self.emit(diagnostic.clone()).await
    }

    pub async fn emit_report(
        &self,
        report: &DiagnosticReport,
    ) -> Result<SubmissionSummary, ReportSubmitError> {
        let mut submitted = SubmissionSummary::default();

        for diagnostic in report {
            match self.emit_ref(diagnostic).await {
                Ok(SubmitOutcome::Enqueued) => {
                    submitted.enqueued += 1;
                }

                Ok(SubmitOutcome::Dropped) => {
                    submitted.dropped += 1;
                }

                Err(error) => {
                    return Err(ReportSubmitError { error, submitted });
                }
            }
        }

        Ok(submitted)
    }

    /// Waits until all previously submitted diagnostics have been delivered
    /// and the underlying sink has flushed.
    pub async fn flush(&self) -> Result<(), AsyncSinkError> {
        if let Some(error) = self.state.failure() {
            return Err(AsyncSinkError::Worker { error });
        }

        let (response_tx, response_rx) = oneshot::channel();

        self.sender
            .send(Command::Flush(response_tx))
            .await
            .map_err(|_| self.closed_error())?;

        let result = response_rx.await.map_err(|_| self.closed_error())?;

        result.map_err(|error| AsyncSinkError::Worker { error })
    }

    /// Flushes all queued diagnostics and shuts the worker down.
    pub async fn shutdown(self) -> Result<(), AsyncSinkError> {
        let state = self.state.clone();

        let (response_tx, response_rx) = oneshot::channel();

        let send_result = self.sender.send(Command::Shutdown(response_tx)).await;

        let response_result = if send_result.is_ok() {
            match response_rx.await {
                Ok(result) => result.map_err(|error| AsyncSinkError::Worker { error }),

                Err(_) => Err(state_error_or_closed(&state)),
            }
        } else {
            Err(state_error_or_closed(&state))
        };

        drop(self.sender);

        let worker_result = self
            .worker
            .await
            .map_err(|error| AsyncSinkError::Join {
                message: error.to_string(),
            })?
            .map_err(|error| AsyncSinkError::Worker { error });

        response_result?;
        worker_result
    }

    fn closed_error(&self) -> AsyncSinkError {
        state_error_or_closed(&self.state)
    }
}

fn state_error_or_closed(state: &WorkerState) -> AsyncSinkError {
    match state.failure() {
        Some(error) => AsyncSinkError::Worker { error },

        None => AsyncSinkError::Closed,
    }
}

fn worker_loop(
    mut receiver: mpsc::Receiver<Command>,
    sink: Box<dyn DiagnosticSink>,
    state: WorkerState,
) -> SinkResult<()> {
    while let Some(command) = receiver.blocking_recv() {
        match command {
            Command::Diagnostic(diagnostic) => {
                if let Err(error) = sink.emit(&diagnostic) {
                    state.fail(error.clone());

                    receiver.close();

                    return Err(error);
                }
            }

            Command::Flush(response) => {
                let result = sink.flush();

                if let Err(error) = &result {
                    state.fail(error.clone());

                    receiver.close();
                }

                let failed = result.is_err();

                let _ = response.send(result.clone());

                if failed {
                    return result;
                }
            }

            Command::Shutdown(response) => {
                receiver.close();

                let result = sink.flush();

                if let Err(error) = &result {
                    state.fail(error.clone());
                }

                let _ = response.send(result.clone());

                return result;
            }
        }
    }

    let result = sink.flush();

    if let Err(error) = &result {
        state.fail(error.clone());
    }

    result
}
