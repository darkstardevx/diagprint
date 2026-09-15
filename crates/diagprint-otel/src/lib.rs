//! Privacy-aware OpenTelemetry integration for diagprint.
//!
//! `diagprint-otel` maps structured diagnostics to OpenTelemetry events without
//! owning exporter, collector, or SDK configuration.
//!
//! Applications remain free to choose OTLP, Jaeger, Zipkin, stdout, custom
//! exporters, sampling, batching, and runtime configuration.
//!
//! Free-form diagnostic content is not exported in plaintext by default.

mod adapter;
mod event;
mod policy;

pub use adapter::TelemetryAdapter;

pub use event::{DIAGNOSTIC_EVENT_NAME, TelemetryEvent};

pub use policy::{LocationExport, TelemetryPolicy, TextExport};

pub use opentelemetry;
pub use tracing_opentelemetry;
