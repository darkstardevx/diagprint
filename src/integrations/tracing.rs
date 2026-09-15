use crate::{Cause, DiagnosticAttribute, DiagnosticValue, Reporter, Severity};
use ::tracing::{
    Event, Level, Subscriber,
    field::{Field, Visit},
};
use ::tracing_subscriber::{
    layer::{Context, Layer},
    registry::LookupSpan,
};
use std::{
    fmt,
    sync::{Arc, Mutex},
};

#[cfg(feature = "tracing-error")]
/// A `tracing-subscriber` layer that converts tracing events into structured
/// `diagprint` diagnostics.
///
/// By default, warning and error events are emitted. Lower-severity tracing
/// events can be enabled with [`TracingLayer::minimum_severity`].
///
/// Reserved tracing fields:
///
/// - `diag.code` becomes the diagnostic code.
/// - `diag.help` becomes diagnostic help.
/// - `diag.note` becomes a diagnostic note.
/// - `message` becomes the diagnostic message.
///
/// Other tracing fields are preserved as typed [`DiagnosticAttribute`] values
/// rather than flattened into notes.
///
/// When the `tracing-error` feature is enabled, [`TracingLayer::with_span_trace`]
/// can capture the current span trace. Applications must install
/// `tracing_error::ErrorLayer` for span-trace formatting support.
#[derive(Debug, Clone)]
pub struct TracingLayer {
    reporter: Reporter,
    minimum_severity: Severity,
    include_target: bool,
    include_fields: bool,
    include_span_context: bool,
    include_source_location: bool,

    #[cfg(feature = "tracing-error")]
    capture_span_trace: bool,

    emission_failures: Arc<Mutex<Vec<String>>>,
}

impl TracingLayer {
    pub fn new(reporter: Reporter) -> Self {
        Self {
            reporter,
            minimum_severity: Severity::Warning,
            include_target: true,
            include_fields: true,
            include_span_context: true,
            include_source_location: true,

            #[cfg(feature = "tracing-error")]
            capture_span_trace: false,

            emission_failures: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn minimum_severity(mut self, severity: Severity) -> Self {
        self.minimum_severity = severity;
        self
    }

    pub fn with_target(mut self, enabled: bool) -> Self {
        self.include_target = enabled;
        self
    }

    /// Controls whether ordinary tracing event fields become typed diagnostic
    /// attributes.
    pub fn with_fields(mut self, enabled: bool) -> Self {
        self.include_fields = enabled;
        self
    }

    pub fn with_span_context(mut self, enabled: bool) -> Self {
        self.include_span_context = enabled;
        self
    }

    pub fn with_source_location(mut self, enabled: bool) -> Self {
        self.include_source_location = enabled;
        self
    }

    /// Controls capture of the event's active tracing span tree.
    ///
    /// The trace is derived directly from the event scope supplied by
    /// `tracing-subscriber`, so it remains reliable while this layer is
    /// processing the event.
    #[cfg(feature = "tracing-error")]
    pub fn with_span_trace(mut self, enabled: bool) -> Self {
        self.capture_span_trace = enabled;
        self
    }

    pub fn emission_failures(&self) -> Vec<String> {
        let failures = match self.emission_failures.lock() {
            Ok(failures) => failures,

            Err(poisoned) => poisoned.into_inner(),
        };

        failures.clone()
    }

    pub fn take_emission_failures(&self) -> Vec<String> {
        let mut failures = match self.emission_failures.lock() {
            Ok(failures) => failures,

            Err(poisoned) => poisoned.into_inner(),
        };

        std::mem::take(&mut *failures)
    }

    fn record_emission_failure(&self, error: impl fmt::Display) {
        let mut failures = match self.emission_failures.lock() {
            Ok(failures) => failures,

            Err(poisoned) => poisoned.into_inner(),
        };

        failures.push(error.to_string());
    }
}

impl<S> Layer<S> for TracingLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let severity = severity_from_level(event.metadata().level());

        if severity < self.minimum_severity {
            return;
        }

        let mut visitor = EventVisitor::default();

        event.record(&mut visitor);

        let EventVisitor {
            message,
            code,
            help,
            notes,
            attributes,
            cause,
        } = visitor;

        let message = message.unwrap_or_else(|| event.metadata().name().to_string());

        let mut diagnostic = self.reporter.diagnostic(severity, message);

        if let Some(code) = code {
            diagnostic = diagnostic.code(code);
        }

        if let Some(help) = help {
            diagnostic = diagnostic.help(help);
        }

        for note in notes {
            diagnostic = diagnostic.note(note);
        }

        if let Some(cause) = cause {
            diagnostic = diagnostic.cause_chain(cause);
        }

        if self.include_fields {
            diagnostic = diagnostic.attributes(attributes);
        }

        if self.include_target {
            diagnostic = diagnostic.note(format!("trace target: {}", event.metadata().target()));
        }

        if self.include_span_context {
            if let Some(scope) = ctx.event_scope(event) {
                let spans: Vec<&str> = scope
                    .from_root()
                    .map(|span| span.metadata().name())
                    .collect();

                if !spans.is_empty() {
                    diagnostic = diagnostic.note(format!("trace spans: {}", spans.join(" > ")));
                }
            }
        }

        if self.include_source_location {
            if let (Some(file), Some(line)) = (event.metadata().file(), event.metadata().line()) {
                diagnostic = diagnostic.source(file, line, None);
            }
        }

        #[cfg(feature = "tracing-error")]
        #[cfg(feature = "tracing-error")]
        if self.capture_span_trace {
            if let Some(scope) = ctx.event_scope(event) {
                let spans = scope
                    .from_root()
                    .map(|span| span.metadata().name())
                    .collect::<Vec<_>>();

                if !spans.is_empty() {
                    diagnostic = diagnostic.attribute("tracing.span_trace", spans.join(" > "));
                }
            }
        }

        if let Err(error) = self.reporter.emit(&diagnostic) {
            self.record_emission_failure(error);
        }
    }
}

#[derive(Debug, Default)]
struct EventVisitor {
    message: Option<String>,
    code: Option<String>,
    help: Option<String>,
    notes: Vec<String>,

    attributes: Vec<DiagnosticAttribute>,

    cause: Option<Cause>,
}

impl EventVisitor {
    fn record_value(&mut self, field: &Field, value: DiagnosticValue) {
        match field.name() {
            "message" => {
                self.message = Some(value.to_string());
            }

            "diag.code" => {
                self.code = Some(value.to_string());
            }

            "diag.help" => {
                self.help = Some(value.to_string());
            }

            "diag.note" => {
                self.notes.push(value.to_string());
            }

            _ => {
                self.attributes
                    .push(DiagnosticAttribute::new(field.name(), value));
            }
        }
    }
}

impl Visit for EventVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.record_value(field, DiagnosticValue::String(format!("{value:?}")));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.record_value(field, DiagnosticValue::String(value.to_owned()));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.record_value(field, DiagnosticValue::Bool(value));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.record_value(field, DiagnosticValue::F64(value));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record_value(field, DiagnosticValue::I64(value));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.record_value(field, DiagnosticValue::U64(value));
    }

    fn record_i128(&mut self, field: &Field, value: i128) {
        self.record_value(field, DiagnosticValue::I128(value));
    }

    fn record_u128(&mut self, field: &Field, value: u128) {
        self.record_value(field, DiagnosticValue::U128(value));
    }

    fn record_bytes(&mut self, field: &Field, value: &[u8]) {
        self.record_value(field, DiagnosticValue::String(format!("{value:?}")));
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        if self.cause.is_none() {
            self.cause = Some(Cause::from_error(value));
        } else {
            self.attributes
                .push(DiagnosticAttribute::new(field.name(), value.to_string()));
        }
    }
}

fn severity_from_level(level: &Level) -> Severity {
    match *level {
        Level::ERROR => Severity::Error,
        Level::WARN => Severity::Warning,
        Level::INFO => Severity::Info,
        Level::DEBUG => Severity::Debug,
        Level::TRACE => Severity::Trace,
    }
}
