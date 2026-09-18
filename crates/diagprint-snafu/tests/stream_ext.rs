#![cfg(feature = "futures")]

use diagprint::Reporter;
use diagprint_snafu::{
    SnafuBridgeError, SnafuCode, SnafuDiagnostic, SnafuDiagnosticMetadata, SnafuIdentity,
    prelude::*,
};
use futures::{StreamExt as _, executor::block_on, stream};
use snafu::Snafu;
use std::{cell::Cell, io};

fn reporter() -> Reporter {
    Reporter::builder()
        .application("diagprint-snafu-stream-test")
        .color(false)
        .build()
        .unwrap()
}

#[derive(Debug, Snafu)]
#[snafu(display("Stream leaf {value}"))]
struct LeafError {
    value: u8,
}

impl SnafuDiagnostic for LeafError {
    fn diagprint_metadata(&self) -> Result<SnafuDiagnosticMetadata, SnafuBridgeError> {
        Ok(SnafuDiagnosticMetadata::new(
            SnafuIdentity::new("stream.leaf")?,
            format!("Stream leaf {}", self.value),
        )
        .code(SnafuCode::new("STREAM_LEAF")?))
    }
}

#[derive(Debug, Snafu)]
enum AppError {
    #[snafu(display("Streaming load failed"))]
    Loading { source: io::Error },
}

impl SnafuDiagnostic for AppError {
    fn diagprint_metadata(&self) -> Result<SnafuDiagnosticMetadata, SnafuBridgeError> {
        Ok(SnafuDiagnosticMetadata::new(
            SnafuIdentity::new("stream.loading")?,
            "Streaming load failed",
        )
        .code(SnafuCode::new("STREAM_LOADING")?))
    }
}

#[test]
fn stream_captures_each_error_independently_without_accumulation() {
    let reporter = reporter();

    let source = stream::iter(vec![
        Ok::<u8, LeafError>(1),
        Err(LeafError { value: 2 }),
        Ok(3),
        Err(LeafError { value: 4 }),
    ]);

    let items = block_on(source.diagprint(&reporter).collect::<Vec<_>>());

    assert_eq!(items.len(), 4);
    assert_eq!(items[0].as_ref().unwrap(), &1);
    assert_eq!(items[2].as_ref().unwrap(), &3);

    let first = items[1].as_ref().unwrap_err();
    let second = items[3].as_ref().unwrap_err();

    assert_eq!(first.error().value, 2);
    assert_eq!(second.error().value, 4);
    assert_eq!(first.report().unwrap().len(), 1);
    assert_eq!(second.report().unwrap().len(), 1);
    assert!(!std::ptr::eq(
        first.report().unwrap(),
        second.report().unwrap()
    ));
}

#[test]
fn stream_context_applies_to_each_error_item() {
    let reporter = reporter();

    let source = stream::iter(vec![
        Err::<u8, io::Error>(io::Error::new(io::ErrorKind::NotFound, "one")),
        Ok(8),
        Err(io::Error::new(io::ErrorKind::PermissionDenied, "two")),
    ]);

    let items = block_on(
        source
            .diagprint_context(&reporter, LoadingSnafu)
            .collect::<Vec<_>>(),
    );

    assert_eq!(items.len(), 3);

    for index in [0usize, 2usize] {
        let captured = items[index].as_ref().unwrap_err();
        assert!(matches!(captured.error(), AppError::Loading { .. }));
        assert_eq!(captured.report().unwrap().len(), 2);
        assert_eq!(captured.graph().unwrap().edge_count(), 1);
    }

    assert_eq!(items[1].as_ref().unwrap(), &8);
}

#[test]
fn stream_lazy_context_receives_each_underlying_error() {
    let reporter = reporter();
    let calls = Cell::new(0usize);

    let source = stream::iter(vec![
        Err::<u8, io::Error>(io::Error::new(io::ErrorKind::NotFound, "one")),
        Err(io::Error::new(io::ErrorKind::PermissionDenied, "two")),
    ]);

    let wrapped = source.diagprint_with_context(&reporter, |source| {
        assert!(matches!(
            source.kind(),
            io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
        ));
        calls.set(calls.get() + 1);
        LoadingSnafu
    });

    assert_eq!(calls.get(), 0);

    let items = block_on(wrapped.collect::<Vec<_>>());

    assert_eq!(calls.get(), 2);
    assert_eq!(items.len(), 2);
    assert!(items.iter().all(Result::is_err));
}

#[test]
fn stream_wrapper_is_lazy_until_polled() {
    let reporter = reporter();
    let calls = Cell::new(0usize);

    let source = stream::iter(vec![Err::<u8, io::Error>(io::Error::other("boom"))]);

    let wrapped = source.diagprint_with_context(&reporter, |_| {
        calls.set(calls.get() + 1);
        LoadingSnafu
    });

    assert_eq!(calls.get(), 0);

    let _ = block_on(wrapped.collect::<Vec<_>>());

    assert_eq!(calls.get(), 1);
}
