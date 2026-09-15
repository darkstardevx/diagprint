use crate::{Applicability, DocumentationLink, Suggestion};
use std::io;

const IO_ERROR_KIND_DOCS: &str = "https://doc.rust-lang.org/std/io/enum.ErrorKind.html";

pub(crate) fn suggestion(error: &io::Error) -> Suggestion {
    let kind = error.kind();

    let (title, explanation) = match kind {
        io::ErrorKind::NotFound => (
            "Verify the referenced path or resource exists",
            "The error chain contains an I/O not-found error. Check the \
             path, filename, mount, or resource identifier before retrying.",
        ),

        io::ErrorKind::PermissionDenied => (
            "Check access permissions",
            "The error chain contains an I/O permission error. Verify the \
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
            "The error chain contains a standard-library I/O error. Review \
             its error kind and operating-system message before deciding on \
             a remedy.",
        ),
    };

    Suggestion::new(title)
        .explanation(format!(
            "{explanation} Detected std::io::ErrorKind::{kind:?}."
        ))
        .applicability(Applicability::Manual)
        .documentation(
            DocumentationLink::new("std::io::ErrorKind", IO_ERROR_KIND_DOCS).language("rust"),
        )
}
