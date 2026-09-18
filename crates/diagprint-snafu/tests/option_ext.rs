use diagprint::Reporter;
use diagprint_snafu::{
    SnafuBridgeError, SnafuCode, SnafuDiagnostic, SnafuDiagnosticMetadata, SnafuIdentity,
    prelude::*,
};
use snafu::Snafu;
use std::cell::Cell;
fn reporter() -> Reporter {
    Reporter::builder()
        .application("diagprint-snafu-option-test")
        .color(false)
        .build()
        .unwrap()
}
#[derive(Debug, Snafu)]
enum AppError {
    #[snafu(display("Missing configuration {name}"))]
    MissingConfig { name: String },
}
impl SnafuDiagnostic for AppError {
    fn diagprint_metadata(&self) -> Result<SnafuDiagnosticMetadata, SnafuBridgeError> {
        match self {
            Self::MissingConfig { name } => Ok(SnafuDiagnosticMetadata::new(
                SnafuIdentity::new("config.missing")?,
                format!("Missing configuration {name}"),
            )
            .code(SnafuCode::new("CONFIG_MISSING")?)),
        }
    }
}
#[test]
fn some_option_passes_through_without_context() {
    let v = Some(12u8)
        .diagprint_context(
            &reporter(),
            MissingConfigSnafu {
                name: "unused".to_owned(),
            },
        )
        .unwrap();
    assert_eq!(v, 12);
}
#[test]
fn none_option_builds_typed_snafu_error_and_captures_it() {
    let c = Option::<u8>::None
        .diagprint_context(
            &reporter(),
            MissingConfigSnafu {
                name: "main".to_owned(),
            },
        )
        .unwrap_err();
    assert!(matches!(c.error(),AppError::MissingConfig{name} if name=="main"));
    assert_eq!(c.report().unwrap().len(), 1);
}
#[test]
fn lazy_option_context_only_runs_for_none() {
    let calls = Cell::new(0usize);
    let v = Some(4u8)
        .diagprint_with_context(&reporter(), || {
            calls.set(calls.get() + 1);
            MissingConfigSnafu {
                name: "unused".to_owned(),
            }
        })
        .unwrap();
    assert_eq!(v, 4);
    assert_eq!(calls.get(), 0);
    let c = Option::<u8>::None
        .diagprint_with_context(&reporter(), || {
            calls.set(calls.get() + 1);
            MissingConfigSnafu {
                name: "main".to_owned(),
            }
        })
        .unwrap_err();
    assert_eq!(calls.get(), 1);
    assert!(c.capture_error().is_none());
}
