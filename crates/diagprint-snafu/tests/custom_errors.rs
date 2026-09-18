use diagprint::{DiagnosticRelationshipEvidence, DiagnosticRelationshipKind, Reporter, Severity};
use diagprint_snafu::{
    SnafuBridge, SnafuBridgeError, SnafuCode, SnafuDiagnostic, SnafuDiagnosticMetadata,
    SnafuErrorMapper, SnafuErrorView, SnafuIdentity,
};
use snafu::{ErrorCompat, Snafu};
use std::{error::Error as StdError, fmt, io, path::PathBuf};

fn reporter() -> Reporter {
    Reporter::builder()
        .application("diagprint-snafu-test")
        .color(false)
        .build()
        .unwrap()
}

#[derive(Debug, Snafu)]
enum AppError {
    #[snafu(display("Could not read config {}",path.display()))]
    ReadConfig { path: PathBuf, source: io::Error },
    #[snafu(display("Invalid port {value}"))]
    InvalidPort { value: u16 },
    #[snafu(display("Missing config {name}"))]
    MissingConfig { name: String },
}
impl SnafuDiagnostic for AppError {
    fn diagprint_metadata(&self) -> Result<SnafuDiagnosticMetadata, SnafuBridgeError> {
        match self {
            Self::ReadConfig { path, .. } => Ok(SnafuDiagnosticMetadata::new(
                SnafuIdentity::new("config.read")?,
                "Could not read configuration",
            )
            .code(SnafuCode::new("CONFIG_READ")?)
            .severity(Severity::Error)
            .note(format!("config path: {}", path.display()))),
            Self::InvalidPort { value } => Ok(SnafuDiagnosticMetadata::new(
                SnafuIdentity::new("config.port.invalid")?,
                format!("Invalid port {value}"),
            )
            .code(SnafuCode::new("CONFIG_PORT")?)
            .severity(Severity::Warning)
            .note(format!("port value: {value}"))),
            Self::MissingConfig { name } => Ok(SnafuDiagnosticMetadata::new(
                SnafuIdentity::new("config.missing")?,
                format!("Missing configuration {name}"),
            )
            .code(SnafuCode::new("CONFIG_MISSING")?)),
        }
    }
}

#[derive(Debug, Snafu)]
#[snafu(display("Unable to parse value {value}"))]
struct ParseValueError {
    value: u8,
}
impl SnafuDiagnostic for ParseValueError {
    fn diagprint_metadata(&self) -> Result<SnafuDiagnosticMetadata, SnafuBridgeError> {
        Ok(SnafuDiagnosticMetadata::new(
            SnafuIdentity::new("value.parse")?,
            format!("Unable to parse value {}", self.value),
        )
        .code(SnafuCode::new("VALUE_PARSE")?))
    }
}

#[test]
fn custom_enum_variants_control_stable_metadata() {
    let a = SnafuBridge::new()
        .convert(&AppError::InvalidPort { value: 1000 }, &reporter())
        .unwrap();
    let b = SnafuBridge::new()
        .convert(&AppError::InvalidPort { value: 2000 }, &reporter())
        .unwrap();
    let ad = a.report().iter().next().unwrap();
    let bd = b.report().iter().next().unwrap();
    assert_eq!(ad.severity, Severity::Warning);
    assert_eq!(ad.code.as_deref(), Some("CONFIG_PORT"));
    assert_ne!(ad.message, bd.message);
    assert_eq!(ad.fingerprint().qualified(), bd.fingerprint().qualified());
    assert_ne!(
        ad.digest().unwrap().qualified(),
        bd.digest().unwrap().qualified()
    );
}
#[test]
fn distinct_variant_identity_stays_distinct() {
    let a = SnafuBridge::new()
        .convert(&AppError::InvalidPort { value: 42 }, &reporter())
        .unwrap();
    let b = SnafuBridge::new()
        .convert(
            &AppError::MissingConfig {
                name: "main".into(),
            },
            &reporter(),
        )
        .unwrap();
    assert_ne!(
        a.report().iter().next().unwrap().fingerprint().qualified(),
        b.report().iter().next().unwrap().fingerprint().qualified()
    );
}
#[test]
fn custom_struct_error_uses_same_typed_contract() {
    let o = SnafuBridge::new()
        .convert(&ParseValueError { value: 17 }, &reporter())
        .unwrap();
    assert_eq!(
        o.report().iter().next().unwrap().code.as_deref(),
        Some("VALUE_PARSE")
    );
    assert_eq!(o.mapped_nodes(), 1);
}
#[test]
fn source_chain_becomes_m4_source_evidence() {
    let e = AppError::ReadConfig {
        path: "/tmp/config.toml".into(),
        source: io::Error::new(io::ErrorKind::PermissionDenied, "permission denied"),
    };
    let o = SnafuBridge::new().convert(&e, &reporter()).unwrap();
    assert_eq!(o.report().len(), 2);
    assert_eq!(o.source_nodes(), 1);
    assert_eq!(o.source_relationships(), 1);
    assert_eq!(o.unmapped_nodes(), 1);
    let edge = o.graph().edges.first().unwrap();
    assert_eq!(edge.kind, DiagnosticRelationshipKind::ContributesTo);
    assert_eq!(edge.evidence, DiagnosticRelationshipEvidence::SourceChain);
    assert_eq!(edge.producer, "snafu");
}
#[test]
fn strong_symbol_validation_rejects_runtime_shaped_values() {
    for v in [
        "",
        ".leading",
        "contains space",
        "path/to/file",
        "request=123",
        "\ncontrol",
    ] {
        assert!(SnafuIdentity::new(v).is_err());
        assert!(SnafuCode::new(v).is_err());
    }
    assert!(SnafuIdentity::new("a".repeat(129)).is_err());
    assert!(SnafuCode::new("a".repeat(65)).is_err());
    assert!(SnafuIdentity::new("config.read").is_ok());
}

#[derive(Debug, Snafu)]
#[snafu(display("Externally mapped error {value}"))]
struct ExternalMappedError {
    value: u8,
}
struct ExternalMapper;
impl SnafuErrorMapper for ExternalMapper {
    fn map(
        &self,
        view: SnafuErrorView<'_>,
    ) -> Result<Option<SnafuDiagnosticMetadata>, SnafuBridgeError> {
        if let Some(e) = view.downcast_ref::<ExternalMappedError>() {
            return Ok(Some(
                SnafuDiagnosticMetadata::new(
                    SnafuIdentity::new("external.mapped")?,
                    format!("Mapped external value {}", e.value),
                )
                .code(SnafuCode::new("EXTERNAL_MAPPED")?),
            ));
        }
        Ok(None)
    }
}
#[test]
fn mapper_route_supports_non_trait_customization() {
    let o = SnafuBridge::new()
        .convert_with_mapper(
            &ExternalMappedError { value: 7 },
            &reporter(),
            &ExternalMapper,
        )
        .unwrap();
    assert_eq!(o.mapped_nodes(), 1);
}
struct EmptyMapper;
impl SnafuErrorMapper for EmptyMapper {
    fn map(
        &self,
        _: SnafuErrorView<'_>,
    ) -> Result<Option<SnafuDiagnosticMetadata>, SnafuBridgeError> {
        Ok(None)
    }
}
#[test]
fn mapper_route_fails_closed_without_root_identity() {
    assert!(matches!(
        SnafuBridge::new().convert_with_mapper(
            &ExternalMappedError { value: 7 },
            &reporter(),
            &EmptyMapper
        ),
        Err(SnafuBridgeError::MissingRootIdentity)
    ));
}

#[derive(Debug)]
struct CycleError;
impl fmt::Display for CycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("cycle")
    }
}
impl StdError for CycleError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(self)
    }
}
impl ErrorCompat for CycleError {}
struct CycleMapper;
impl SnafuErrorMapper for CycleMapper {
    fn map(
        &self,
        view: SnafuErrorView<'_>,
    ) -> Result<Option<SnafuDiagnosticMetadata>, SnafuBridgeError> {
        if view.is_root() {
            Ok(Some(SnafuDiagnosticMetadata::new(
                SnafuIdentity::new("cycle.root")?,
                "cycle root",
            )))
        } else {
            Ok(None)
        }
    }
}
#[test]
fn malformed_source_cycle_fails_closed() {
    assert!(matches!(
        SnafuBridge::new().convert_with_mapper(&CycleError, &reporter(), &CycleMapper),
        Err(SnafuBridgeError::SourceCycle { depth: 1 })
    ));
}
