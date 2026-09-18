use diagprint::Reporter;
use diagprint_snafu::{
    SnafuBridgeError, SnafuCode, SnafuDiagnostic, SnafuDiagnosticMetadata, SnafuErrorMapper,
    SnafuErrorView, SnafuIdentity, prelude::*,
};
use snafu::Snafu;
use std::{cell::Cell, io, path::PathBuf};
fn reporter() -> Reporter {
    Reporter::builder()
        .application("diagprint-snafu-result-test")
        .color(false)
        .build()
        .unwrap()
}
#[derive(Debug, Snafu)]
enum AppError {
    #[snafu(display("Read config {}",path.display()))]
    ReadConfig { path: PathBuf, source: io::Error },
    #[snafu(display("Leaf error {value}"))]
    Leaf { value: u8 },
}
impl SnafuDiagnostic for AppError {
    fn diagprint_metadata(&self) -> Result<SnafuDiagnosticMetadata, SnafuBridgeError> {
        match self {
            Self::ReadConfig { path, .. } => Ok(SnafuDiagnosticMetadata::new(
                SnafuIdentity::new("config.read")?,
                "Read configuration",
            )
            .code(SnafuCode::new("CONFIG_READ")?)
            .note(format!("path: {}", path.display()))),
            Self::Leaf { value } => Ok(SnafuDiagnosticMetadata::new(
                SnafuIdentity::new("leaf.error")?,
                format!("Leaf error {value}"),
            )
            .code(SnafuCode::new("LEAF_ERROR")?)),
        }
    }
}
#[test]
fn ok_result_passes_through_without_capture() {
    let r: Result<u8, AppError> = Ok(7);
    assert_eq!(r.diagprint(&reporter()).unwrap(), 7);
}
#[test]
fn err_result_preserves_typed_error_and_output() {
    let r: Result<(), AppError> = Err(AppError::Leaf { value: 9 });
    let c = r.diagprint(&reporter()).unwrap_err();
    assert!(matches!(c.error(), AppError::Leaf { value: 9 }));
    assert!(c.capture_error().is_none());
    assert_eq!(c.report().unwrap().len(), 1);
    assert!(matches!(c.into_error(), AppError::Leaf { value: 9 }));
}
#[derive(Debug, Snafu)]
#[snafu(display("Broken metadata error"))]
struct BrokenMetadataError;
impl SnafuDiagnostic for BrokenMetadataError {
    fn diagprint_metadata(&self) -> Result<SnafuDiagnosticMetadata, SnafuBridgeError> {
        Err(SnafuBridgeError::MissingRootIdentity)
    }
}
#[test]
fn capture_failure_never_replaces_application_error() {
    let r: Result<(), BrokenMetadataError> = Err(BrokenMetadataError);
    let c = r.diagprint(&reporter()).unwrap_err();
    assert!(matches!(
        c.capture_error(),
        Some(SnafuBridgeError::MissingRootIdentity)
    ));
    assert!(c.output().is_err());
    assert_eq!(c.to_string(), "Broken metadata error");
    let _: BrokenMetadataError = c.into_error();
}
#[test]
fn result_context_builds_snafu_error_then_captures_it() {
    let src: Result<(), io::Error> = Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied"));
    let c = src
        .diagprint_context(
            &reporter(),
            ReadConfigSnafu {
                path: PathBuf::from("/tmp/config.toml"),
            },
        )
        .unwrap_err();
    assert!(matches!(c.error(), AppError::ReadConfig { .. }));
    assert_eq!(c.report().unwrap().len(), 2);
    assert_eq!(c.graph().unwrap().edge_count(), 1);
}
#[test]
fn lazy_result_context_runs_only_for_err_and_receives_source() {
    let calls = Cell::new(0usize);
    let ok: Result<u8, io::Error> = Ok(4);
    assert_eq!(
        ok.diagprint_with_context(&reporter(), |_| {
            calls.set(calls.get() + 1);
            ReadConfigSnafu {
                path: PathBuf::from("/unused"),
            }
        })
        .unwrap(),
        4
    );
    assert_eq!(calls.get(), 0);
    let err: Result<u8, io::Error> = Err(io::Error::new(io::ErrorKind::NotFound, "missing"));
    let c = err
        .diagprint_with_context(&reporter(), |source| {
            assert_eq!(source.kind(), io::ErrorKind::NotFound);
            calls.set(calls.get() + 1);
            ReadConfigSnafu {
                path: PathBuf::from("/tmp/config.toml"),
            }
        })
        .unwrap_err();
    assert_eq!(calls.get(), 1);
    assert!(c.capture_error().is_none());
}
#[derive(Debug, Snafu)]
#[snafu(display("External {value}"))]
struct ExternalError {
    value: u8,
}
struct ExternalMapper;
impl SnafuErrorMapper for ExternalMapper {
    fn map(
        &self,
        view: SnafuErrorView<'_>,
    ) -> Result<Option<SnafuDiagnosticMetadata>, SnafuBridgeError> {
        let Some(e) = view.downcast_ref::<ExternalError>() else {
            return Ok(None);
        };
        Ok(Some(
            SnafuDiagnosticMetadata::new(
                SnafuIdentity::new("external.error")?,
                format!("External {}", e.value),
            )
            .code(SnafuCode::new("EXTERNAL")?),
        ))
    }
}
#[test]
fn result_mapper_route_preserves_external_concrete_error() {
    let r: Result<(), ExternalError> = Err(ExternalError { value: 3 });
    let c = r.diagprint_with(&reporter(), &ExternalMapper).unwrap_err();
    assert_eq!(c.error().value, 3);
    assert!(c.capture_error().is_none());
}
