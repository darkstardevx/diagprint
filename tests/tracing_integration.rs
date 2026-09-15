#![cfg(feature = "tracing")]

use diagprint::{Reporter, Severity, TracingLayer};
use std::{error::Error, fs, io, path::PathBuf};
use tracing_subscriber::prelude::*;
use uuid::Uuid;

fn log_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("diagprint-{name}-{}.log", Uuid::now_v7()))
}

fn reporter(path: &PathBuf) -> Reporter {
    Reporter::builder()
        .application("tracing-test")
        .file(path)
        .color(false)
        .build()
        .unwrap()
}

#[test]
fn tracing_event_metadata_is_preserved() {
    let path = log_path("metadata");

    let layer = TracingLayer::new(reporter(&path))
        .minimum_severity(Severity::Trace)
        .with_target(false)
        .with_span_context(false)
        .with_source_location(false);

    let subscriber = tracing_subscriber::registry().with(layer);

    tracing::subscriber::with_default(subscriber, || {
        tracing::warn!(
            diag.code = "TRACE-001",
            diag.help = "Refresh the cache.",
            diag.note = "cache subsystem reported stale state",
            user_id = 42_u64,
            retry = true,
            delta = -2_i64,
            ratio = 1.5_f64,
            cache = "primary",
            "cache is stale"
        );
    });

    let output = fs::read_to_string(&path).unwrap();

    assert!(output.contains("WARNING [TRACE-001]: cache is stale"));

    assert!(output.contains("help: Refresh the cache."));

    assert!(output.contains("note: cache subsystem reported stale state"));

    assert!(output.contains("attributes:"));
    assert!(output.contains("user_id=42"));
    assert!(output.contains("retry=true"));
    assert!(output.contains("delta=-2"));
    assert!(output.contains("ratio=1.5"));
    assert!(output.contains("cache=primary"));

    fs::remove_file(path).unwrap();
}

#[test]
fn active_span_context_is_preserved() {
    let path = log_path("spans");

    let layer = TracingLayer::new(reporter(&path))
        .with_target(false)
        .with_fields(false)
        .with_source_location(false);

    let subscriber = tracing_subscriber::registry().with(layer);

    tracing::subscriber::with_default(subscriber, || {
        let request = tracing::info_span!("request");

        let _request = request.enter();

        let config = tracing::info_span!("load_config");

        let _config = config.enter();

        tracing::warn!("configuration is stale");
    });

    let output = fs::read_to_string(&path).unwrap();

    assert!(output.contains("note: trace spans: request > load_config"));

    fs::remove_file(path).unwrap();
}

#[test]
fn real_error_values_become_cause_chains() {
    let path = log_path("error");

    let layer = TracingLayer::new(reporter(&path))
        .with_target(false)
        .with_fields(false)
        .with_span_context(false)
        .with_source_location(false);

    let subscriber = tracing_subscriber::registry().with(layer);

    tracing::subscriber::with_default(subscriber, || {
        let error = io::Error::new(io::ErrorKind::NotFound, "configuration file is missing");

        tracing::error!(
            error = &error as &(dyn Error + 'static),
            "configuration loading failed"
        );
    });

    let output = fs::read_to_string(&path).unwrap();

    assert!(output.contains("ERROR: configuration loading failed"));

    assert!(output.contains("configuration file is missing"));

    fs::remove_file(path).unwrap();
}

#[test]
fn default_layer_ignores_info_events() {
    let path = log_path("threshold");

    let layer = TracingLayer::new(reporter(&path)).with_source_location(false);

    let subscriber = tracing_subscriber::registry().with(layer);

    tracing::subscriber::with_default(subscriber, || {
        tracing::info!("ordinary informational event");
    });

    assert!(!path.exists());
}

#[test]
fn reporter_failures_are_retained() {
    let reporter = Reporter::builder()
        .application("tracing-test")
        .file(std::env::temp_dir())
        .color(false)
        .build()
        .unwrap();

    let layer = TracingLayer::new(reporter)
        .with_target(false)
        .with_fields(false)
        .with_span_context(false)
        .with_source_location(false);

    let monitor = layer.clone();

    let subscriber = tracing_subscriber::registry().with(layer);

    tracing::subscriber::with_default(subscriber, || {
        tracing::error!("this write will fail");
    });

    let failures = monitor.take_emission_failures();

    assert_eq!(failures.len(), 1);
    assert!(monitor.emission_failures().is_empty());
}

#[test]
fn span_path_attribute_is_explicit_and_structured() {
    let path = log_path("span-path");

    let layer = TracingLayer::new(reporter(&path))
        .with_target(false)
        .with_fields(false)
        .with_span_context(false)
        .with_source_location(false)
        .with_span_path_attribute(true);

    let subscriber = tracing_subscriber::registry().with(layer);

    tracing::subscriber::with_default(subscriber, || {
        let request = tracing::info_span!("request", request_id = 42_u64);

        let _guard = request.enter();

        tracing::error!("request failed");
    });

    let output = fs::read_to_string(&path).unwrap();

    assert!(output.contains("tracing.span_path=request"));

    assert!(!output.contains("trace spans:"));

    fs::remove_file(path).unwrap();
}
