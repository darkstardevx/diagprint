use crate::{DIAGNOSTIC_EVENT_NAME, TelemetryEvent, TelemetryPolicy, event::build_event};
use diagprint::{Diagnostic, DiagnosticReport};
use opentelemetry::{
    KeyValue,
    trace::{Span as OtelSpan, Status},
};
use tracing::Span as TracingSpan;
use tracing_opentelemetry::OpenTelemetrySpanExt;

#[derive(Debug, Clone, Copy, Default)]
pub struct TelemetryAdapter {
    policy: TelemetryPolicy,
}

impl TelemetryAdapter {
    pub const fn new(policy: TelemetryPolicy) -> Self {
        Self { policy }
    }

    pub const fn policy(&self) -> TelemetryPolicy {
        self.policy
    }

    pub fn event(&self, diagnostic: &Diagnostic) -> TelemetryEvent {
        build_event(diagnostic, self.policy)
    }

    /// Records a diagnostic directly on a native OpenTelemetry span.
    pub fn record_on_otel_span<S>(&self, span: &mut S, diagnostic: &Diagnostic)
    where
        S: OtelSpan,
    {
        let event = self.event(diagnostic);

        if event.marks_span_error() {
            span.set_status(Status::error("diagprint error diagnostic"));
        }

        span.add_event(DIAGNOSTIC_EVENT_NAME, event.into_attributes());
    }

    /// Records a diagnostic on a tracing span through tracing-opentelemetry.
    ///
    /// If the application has not installed an OpenTelemetry tracing layer,
    /// the underlying tracing/OpenTelemetry bridge determines the resulting
    /// no-op behavior.
    pub fn record_on_tracing_span(&self, span: &TracingSpan, diagnostic: &Diagnostic) {
        let event = self.event(diagnostic);

        if event.marks_span_error() {
            span.set_status(Status::error("diagprint error diagnostic"));
        }

        span.add_event(DIAGNOSTIC_EVENT_NAME, event.into_attributes());
    }

    /// Records a diagnostic on the currently active tracing span.
    pub fn record_current(&self, diagnostic: &Diagnostic) {
        self.record_on_tracing_span(&TracingSpan::current(), diagnostic);
    }

    /// Records every diagnostic in a report on one native OpenTelemetry span.
    pub fn record_report_on_otel_span<S>(&self, span: &mut S, report: &DiagnosticReport) -> usize
    where
        S: OtelSpan,
    {
        let counts = report.counts();

        span.set_attribute(KeyValue::new(
            "diagprint.report.total",
            usize_to_i64(counts.total()),
        ));

        span.set_attribute(KeyValue::new(
            "diagprint.report.failures",
            usize_to_i64(counts.failures()),
        ));

        for diagnostic in report {
            self.record_on_otel_span(span, diagnostic);
        }

        report.len()
    }

    /// Records every diagnostic in a report on one tracing/OpenTelemetry span.
    pub fn record_report_on_tracing_span(
        &self,
        span: &TracingSpan,
        report: &DiagnosticReport,
    ) -> usize {
        let counts = report.counts();

        span.set_attribute("diagprint.report.total", usize_to_i64(counts.total()));

        span.set_attribute("diagprint.report.failures", usize_to_i64(counts.failures()));

        for diagnostic in report {
            self.record_on_tracing_span(span, diagnostic);
        }

        report.len()
    }

    pub fn record_report_current(&self, report: &DiagnosticReport) -> usize {
        self.record_report_on_tracing_span(&TracingSpan::current(), report)
    }
}

fn usize_to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}
