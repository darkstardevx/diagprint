use crate::{Cause, Reporter, Severity};
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

/// A `tracing-subscriber` layer that converts tracing events into structured
/// `diagprint` diagnostics.
///
/// By default, warning and error events are emitted. Lower-severity tracing
/// events can be enabled with [`TracingLayer::minimum_severity`].
///
/// The following tracing fields have special meaning:
///
/// - `diag.code` becomes [`crate::Diagnostic::code`].
/// - `diag.help` becomes [`crate::Diagnostic::help`].
/// - `diag.note` becomes a diagnostic note.
/// - `message` becomes the diagnostic message.
///
/// Other structured fields can be preserved as diagnostic notes.
///
/// If a tracing event records an actual `std::error::Error` value rather than
/// merely its formatted string, its complete source chain is preserved as the
/// diagnostic cause chain.
#[derive(Debug, Clone)]
pub struct TracingLayer {
    reporter: Reporter,
    minimum_severity: Severity,
    include_target: bool,
    include_fields: bool,
    include_span_context: bool,
    include_source_location: bool,
    emission_failures: Arc<Mutex<Vec<String>>>,
}

impl TracingLayer {
    /// Creates a tracing layer backed by the provided reporter.
    pub fn new(reporter: Reporter) -> Self {
        Self {
            reporter,
            minimum_severity: Severity::Warning,
            include_target: true,
            include_fields: true,
            include_span_context: true,
            include_source_location: true,
            emission_failures: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Sets the minimum tracing severity converted into diagnostics.
    pub fn minimum_severity(mut self, severity: Severity) -> Self {
        self.minimum_severity = severity;
        self
    }

    /// Controls whether the tracing target is attached as a diagnostic note.
    pub fn with_target(mut self, enabled: bool) -> Self {
        self.include_target = enabled;
        self
    }

    /// Controls whether non-reserved event fields are preserved as notes.
    pub fn with_fields(mut self, enabled: bool) -> Self {
        self.include_fields = enabled;
        self
    }

    /// Controls whether active tracing span names are preserved.
    pub fn with_span_context(mut self, enabled: bool) -> Self {
        self.include_span_context = enabled;
        self
    }

    /// Controls whether tracing source file and line metadata are attached.
    pub fn with_source_location(mut self, enabled: bool) -> Self {
        self.include_source_location = enabled;
        self
    }

    /// Returns reporter failures encountered while handling tracing events.
    ///
    /// `tracing_subscriber::Layer::on_event` cannot return an I/O error, so
    /// failures are retained here instead of being silently discarded.
    pub fn emission_failures(&self) -> Vec<String> {
        let failures = match self.emission_failures.lock() {
            Ok(failures) => failures,
            Err(poisoned) => poisoned.into_inner(),
        };

        failures.clone()
    }

    /// Removes and returns all stored reporter failures.
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
            fields,
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

        if self.include_target {
            diagnostic = diagnostic.note(format!("trace target: {}", event.metadata().target()));
        }

        if self.include_fields {
            for (name, value) in fields {
                diagnostic = diagnostic.note(format!("field {name}={value}"));
            }
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
    fields: Vec<(String, String)>,
    cause: Option<Cause>,
}

impl EventVisitor {
    fn record_value(&mut self, field: &Field, value: impl Into<String>) {
        let value = value.into();

        match field.name() {
            "message" => {
                self.message = Some(value);
            }

            "diag.code" => {
                self.code = Some(value);
            }

            "diag.help" => {
                self.help = Some(value);
            }

            "diag.note" => {
                self.notes.push(value);
            }

            _ => {
                self.fields.push((field.name().to_string(), value));
            }
        }
    }
}

impl Visit for EventVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.record_value(field, format!("{value:?}"));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.record_value(field, value);
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.record_value(field, value.to_string());
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.record_value(field, value.to_string());
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record_value(field, value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.record_value(field, value.to_string());
    }

    fn record_i128(&mut self, field: &Field, value: i128) {
        self.record_value(field, value.to_string());
    }

    fn record_u128(&mut self, field: &Field, value: u128) {
        self.record_value(field, value.to_string());
    }

    fn record_bytes(&mut self, field: &Field, value: &[u8]) {
        self.record_value(field, format!("{value:?}"));
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        if self.cause.is_none() {
            self.cause = Some(Cause::from_error(value));
        } else {
            self.fields
                .push((field.name().to_string(), value.to_string()));
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
