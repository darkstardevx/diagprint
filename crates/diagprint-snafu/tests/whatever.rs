use diagprint::Reporter;
use diagprint_snafu::{
    DiagprintOptionExt, DiagprintResultExt, SnafuCode, SnafuIdentity, WhateverDiagnosticContext,
};
use std::{error::Error as StdError, fmt, io, rc::Rc};

fn reporter() -> Reporter {
    Reporter::builder()
        .application("diagprint-snafu-whatever-test")
        .color(false)
        .build()
        .unwrap()
}

fn context(message: &str) -> WhateverDiagnosticContext {
    WhateverDiagnosticContext::new(SnafuIdentity::new("config.read").unwrap(), message)
        .code(SnafuCode::new("CONFIG_READ").unwrap())
}

#[test]
fn whatever_context_keeps_identity_separate_from_message() {
    let a = context("Could not read /home/alice/config.toml");
    let b = context("Could not read /srv/app/config.toml");

    assert_eq!(a.identity(), b.identity());
    assert_eq!(a.code_value(), b.code_value());
    assert_ne!(a.message(), b.message());
}

#[test]
fn whatever_context_rejects_message_shaped_identity() {
    assert!(SnafuIdentity::new("Could not read /tmp/config").is_err());
}

#[test]
fn result_strong_whatever_preserves_source_and_explicit_identity() {
    let reporter = reporter();

    let captured = Err::<u8, io::Error>(io::Error::other("disk failed"))
        .diagprint_whatever_context(&reporter, context("reading configuration"))
        .unwrap_err();

    assert_eq!(captured.error().to_string(), "reading configuration");
    assert!(captured.error().source().is_some());

    let output = captured.output().unwrap();
    assert_eq!(output.whatever_nodes(), 1);
    assert_eq!(output.whatever_local_nodes(), 0);
    assert_eq!(output.source_nodes(), 1);
}

#[test]
fn option_strong_whatever_is_message_independent_for_identity() {
    let reporter = reporter();

    let first = Option::<u8>::None
        .diagprint_whatever_context(&reporter, context("first message"))
        .unwrap_err();

    let second = Option::<u8>::None
        .diagprint_whatever_context(&reporter, context("second message"))
        .unwrap_err();

    let first_diagnostic = first.report().unwrap().iter().next().unwrap();
    let second_diagnostic = second.report().unwrap().iter().next().unwrap();

    assert_ne!(first_diagnostic.message, second_diagnostic.message);
    assert_eq!(
        first_diagnostic.fingerprint().qualified(),
        second_diagnostic.fingerprint().qualified()
    );
}

#[derive(Clone)]
struct LocalOnlyError(Rc<()>);

impl fmt::Debug for LocalOnlyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalOnlyError").finish_non_exhaustive()
    }
}

impl fmt::Display for LocalOnlyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _ = &self.0;
        f.write_str("local-only source")
    }
}

impl StdError for LocalOnlyError {}

#[test]
fn whatever_local_accepts_non_send_non_sync_source() {
    let reporter = reporter();

    let captured = Err::<u8, LocalOnlyError>(LocalOnlyError(Rc::new(())))
        .diagprint_whatever_local_context(
            &reporter,
            WhateverDiagnosticContext::new(
                SnafuIdentity::new("local.operation").unwrap(),
                "local operation failed",
            ),
        )
        .unwrap_err();

    let output = captured.output().unwrap();
    assert_eq!(output.whatever_nodes(), 0);
    assert_eq!(output.whatever_local_nodes(), 1);
    assert_eq!(output.source_nodes(), 1);
}
