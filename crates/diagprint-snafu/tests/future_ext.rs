#![cfg(feature = "futures")]

use diagprint::Reporter;
use diagprint_snafu::{
    SnafuBridgeError, SnafuCode, SnafuDiagnostic, SnafuDiagnosticMetadata, SnafuErrorMapper,
    SnafuErrorView, SnafuIdentity, prelude::*,
};
use futures::{executor::block_on, future};
use snafu::Snafu;
use std::{
    io,
    sync::atomic::{AtomicUsize, Ordering},
};

static METADATA_CALLS: AtomicUsize = AtomicUsize::new(0);

fn reporter() -> Reporter {
    Reporter::builder()
        .application("diagprint-snafu-future-test")
        .color(false)
        .build()
        .unwrap()
}

#[derive(Debug, Snafu)]
#[snafu(display("Future leaf {value}"))]
struct LeafError {
    value: u8,
}

impl SnafuDiagnostic for LeafError {
    fn diagprint_metadata(&self) -> Result<SnafuDiagnosticMetadata, SnafuBridgeError> {
        METADATA_CALLS.fetch_add(1, Ordering::SeqCst);

        Ok(SnafuDiagnosticMetadata::new(
            SnafuIdentity::new("future.leaf")?,
            format!("Future leaf {}", self.value),
        )
        .code(SnafuCode::new("FUTURE_LEAF")?))
    }
}

#[derive(Debug, Snafu)]
enum AppError {
    #[snafu(display("Loading failed"))]
    Loading { source: io::Error },
}

impl SnafuDiagnostic for AppError {
    fn diagprint_metadata(&self) -> Result<SnafuDiagnosticMetadata, SnafuBridgeError> {
        Ok(
            SnafuDiagnosticMetadata::new(SnafuIdentity::new("future.loading")?, "Loading failed")
                .code(SnafuCode::new("FUTURE_LOADING")?),
        )
    }
}

#[test]
fn future_capture_is_lazy_and_preserves_typed_error() {
    METADATA_CALLS.store(0, Ordering::SeqCst);
    let reporter = reporter();

    let wrapped =
        future::ready::<Result<u8, LeafError>>(Err(LeafError { value: 7 })).diagprint(&reporter);

    assert_eq!(METADATA_CALLS.load(Ordering::SeqCst), 0);

    let captured = block_on(wrapped).unwrap_err();

    assert_eq!(METADATA_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(captured.error().value, 7);
    assert_eq!(captured.report().unwrap().len(), 1);
}

#[test]
fn future_ok_value_passes_through() {
    let reporter = reporter();

    let value =
        block_on(future::ready::<Result<u8, LeafError>>(Ok(42)).diagprint(&reporter)).unwrap();

    assert_eq!(value, 42);
}

#[test]
fn future_context_is_built_on_error_then_captured() {
    let reporter = reporter();

    let wrapped = future::ready::<Result<u8, io::Error>>(Err(io::Error::new(
        io::ErrorKind::NotFound,
        "missing",
    )))
    .diagprint_context(&reporter, LoadingSnafu);

    let captured = block_on(wrapped).unwrap_err();

    assert!(matches!(captured.error(), AppError::Loading { .. }));
    assert_eq!(captured.report().unwrap().len(), 2);
    assert_eq!(captured.graph().unwrap().edge_count(), 1);
}

#[test]
fn future_lazy_context_receives_underlying_error() {
    let reporter = reporter();
    let calls = AtomicUsize::new(0);

    let wrapped = future::ready::<Result<u8, io::Error>>(Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        "denied",
    )))
    .diagprint_with_context(&reporter, |source| {
        assert_eq!(source.kind(), io::ErrorKind::PermissionDenied);
        calls.fetch_add(1, Ordering::SeqCst);
        LoadingSnafu
    });

    assert_eq!(calls.load(Ordering::SeqCst), 0);

    let captured = block_on(wrapped).unwrap_err();

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(captured.capture_error().is_none());
}

#[derive(Debug, Snafu)]
#[snafu(display("External future error {value}"))]
struct ExternalError {
    value: u8,
}

struct ExternalMapper;

impl SnafuErrorMapper for ExternalMapper {
    fn map(
        &self,
        view: SnafuErrorView<'_>,
    ) -> Result<Option<SnafuDiagnosticMetadata>, SnafuBridgeError> {
        let Some(error) = view.downcast_ref::<ExternalError>() else {
            return Ok(None);
        };

        Ok(Some(
            SnafuDiagnosticMetadata::new(
                SnafuIdentity::new("future.external")?,
                format!("External future error {}", error.value),
            )
            .code(SnafuCode::new("FUTURE_EXTERNAL")?),
        ))
    }
}

#[test]
fn future_mapper_route_preserves_external_error() {
    let reporter = reporter();
    let mapper = ExternalMapper;

    let captured = block_on(
        future::ready::<Result<u8, ExternalError>>(Err(ExternalError { value: 9 }))
            .diagprint_with(&reporter, &mapper),
    )
    .unwrap_err();

    assert_eq!(captured.error().value, 9);
    assert!(captured.capture_error().is_none());
}

#[test]
fn snafu_and_diagprint_preludes_compose_without_method_collision() {
    use snafu::prelude::*;

    let native: Result<u8, io::Error> = Err(io::Error::new(io::ErrorKind::NotFound, "missing"));
    let contextual: Result<u8, AppError> = native.context(LoadingSnafu);
    assert!(contextual.is_err());

    fn assert_future<T>(_: &T) {}

    let reporter = reporter();
    let future = future::ready::<Result<u8, LeafError>>(Ok(1)).diagprint(&reporter);

    assert_future(&future);
}
