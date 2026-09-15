use crate::{Applicability, Cause, Diagnostic, DocumentationLink, Reporter, Severity, Suggestion};
use std::io;

const IO_ERROR_KIND_DOCS: &str = "https://doc.rust-lang.org/std/io/enum.ErrorKind.html";

/// Converts `anyhow::Error` values into structured `diagprint` diagnostics.
///
/// The complete Anyhow context chain is preserved:
///
/// - the outermost context becomes the diagnostic message;
/// - remaining context and source errors become the diagnostic cause chain;
/// - known standard-library errors may contribute conservative diagnostic
///   suggestions.
///
/// Automatically generated suggestions from this adapter are intentionally
/// manual. Merely recognizing an error kind is not sufficient evidence for
/// `diagprint` to mutate application state.
pub trait AnyhowDiagnosticExt {
    /// Converts this error into an error-severity diagnostic.
    fn to_diagprint(&self, reporter: &Reporter) -> Diagnostic;

    /// Converts this error using an explicitly selected severity.
    fn to_diagprint_with_severity(&self, reporter: &Reporter, severity: Severity) -> Diagnostic;
}

impl AnyhowDiagnosticExt for ::anyhow::Error {
    fn to_diagprint(&self, reporter: &Reporter) -> Diagnostic {
        build_diagnostic(reporter, Severity::Error, self)
    }

    fn to_diagprint_with_severity(&self, reporter: &Reporter, severity: Severity) -> Diagnostic {
        build_diagnostic(reporter, severity, self)
    }
}

impl Reporter {
    /// Converts an `anyhow::Error` into an error-severity diagnostic.
    ///
    /// The error's context and source chain are preserved structurally rather
    /// than flattened into one formatted string.
    pub fn error_from_anyhow(&self, error: &::anyhow::Error) -> Diagnostic {
        build_diagnostic(self, Severity::Error, error)
    }

    /// Converts an `anyhow::Error` into a diagnostic with the requested
    /// severity.
    pub fn diagnostic_from_anyhow(
        &self,
        severity: Severity,
        error: &::anyhow::Error,
    ) -> Diagnostic {
        build_diagnostic(self, severity, error)
    }
}

fn build_diagnostic(
    reporter: &Reporter,
    severity: Severity,
    error: &::anyhow::Error,
) -> Diagnostic {
    let mut chain = error.chain();

    let message = chain
        .next()
        .map(ToString::to_string)
        .unwrap_or_else(|| error.to_string());

    let cause_messages: Vec<String> = chain.map(ToString::to_string).collect();

    let mut diagnostic = reporter.diagnostic(severity, message);

    if let Some(cause) = cause_chain(cause_messages) {
        diagnostic = diagnostic.cause_chain(cause);
    }

    if let Some(suggestion) = io_error_suggestion(error) {
        diagnostic = diagnostic.suggestion(suggestion);
    }

    diagnostic
}

fn cause_chain(messages: Vec<String>) -> Option<Cause> {
    let mut current = None;

    for message in messages.into_iter().rev() {
        let cause = match current.take() {
            Some(source) => Cause::new(message).caused_by(source),
            None => Cause::new(message),
        };

        current = Some(cause);
    }

    current
}

fn io_error_suggestion(error: &::anyhow::Error) -> Option<Suggestion> {
    let io_error = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<io::Error>())?;

    let kind = io_error.kind();

    let (title, explanation) = match kind {
        io::ErrorKind::NotFound => (
            "Verify the referenced path or resource exists",
            "The Anyhow chain contains an I/O not-found error. Check the \
             path, filename, mount, or resource identifier before retrying.",
        ),

        io::ErrorKind::PermissionDenied => (
            "Check access permissions",
            "The Anyhow chain contains an I/O permission error. Verify the \
             current user has the required read, write, or execute access.",
        ),

        io::ErrorKind::ConnectionRefused => (
            "Verify the target service is running",
            "The connection was actively refused. Check that the target \
             service is running and listening on the expected address.",
        ),

        io::ErrorKind::ConnectionReset => (
            "Inspect why the peer reset the connection",
            "The peer reset an established connection. Check service logs, \
             protocol state, and network stability.",
        ),

        io::ErrorKind::ConnectionAborted => (
            "Inspect the aborted connection",
            "The connection was aborted before the operation completed. \
             Check the peer, local network state, and retry policy.",
        ),

        io::ErrorKind::NotConnected => (
            "Establish the connection before retrying",
            "The operation requires an active connection, but the resource \
             is not currently connected.",
        ),

        io::ErrorKind::AddrInUse => (
            "Check which process already owns the address",
            "The requested network address or port is already in use. \
             Inspect existing listeners before selecting another address.",
        ),

        io::ErrorKind::AddrNotAvailable => (
            "Verify the requested local address exists",
            "The requested address is not available on this system. Check \
             interface configuration and the address being bound.",
        ),

        io::ErrorKind::BrokenPipe => (
            "Inspect the closed communication channel",
            "The receiving side of the pipe or connection closed before the \
             write completed.",
        ),

        io::ErrorKind::AlreadyExists => (
            "Check the existing destination",
            "The operation attempted to create something that already \
             exists. Verify whether replacement or reuse is intended.",
        ),

        io::ErrorKind::WouldBlock => (
            "Retry when the resource becomes ready",
            "The operation would block in the current nonblocking state. \
             Wait for readiness before retrying.",
        ),

        io::ErrorKind::InvalidInput => (
            "Validate the operation input",
            "The operating system rejected one or more input parameters. \
             Verify paths, flags, addresses, offsets, and other arguments.",
        ),

        io::ErrorKind::InvalidData => (
            "Validate the input data",
            "The operation encountered malformed or otherwise invalid data. \
             Check the data source and expected format.",
        ),

        io::ErrorKind::TimedOut => (
            "Inspect timeout and reachability",
            "The I/O operation exceeded its timeout. Check reachability, \
             service responsiveness, and whether the timeout is appropriate.",
        ),

        io::ErrorKind::WriteZero => (
            "Inspect the output destination",
            "A write operation returned zero bytes unexpectedly. Check the \
             destination state and whether the underlying resource closed.",
        ),

        io::ErrorKind::Interrupted => (
            "Retry the interrupted operation when appropriate",
            "The operation was interrupted before completion. Some APIs can \
             be retried safely, but the calling code should decide.",
        ),

        io::ErrorKind::UnexpectedEof => (
            "Inspect the truncated input",
            "The input ended earlier than expected. Check whether the source \
             is incomplete or the expected format length is incorrect.",
        ),

        _ => (
            "Inspect the underlying I/O failure",
            "The Anyhow chain contains a standard-library I/O error. Review \
             its error kind and operating-system message before deciding on \
             a remedy.",
        ),
    };

    Some(
        Suggestion::new(title)
            .explanation(format!(
                "{explanation} Detected std::io::ErrorKind::{kind:?}."
            ))
            .applicability(Applicability::Manual)
            .documentation(
                DocumentationLink::new("std::io::ErrorKind", IO_ERROR_KIND_DOCS).language("rust"),
            ),
    )
}
