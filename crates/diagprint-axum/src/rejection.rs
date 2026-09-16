use crate::{DiagnosticResponse, ResponsePolicy};
use axum::{
    extract::rejection::{
        ExtensionRejection, FormRejection, JsonRejection, PathRejection, QueryRejection,
    },
    http::StatusCode,
};
use diagprint::{Diagnostic, Reporter, Severity};

mod sealed {
    pub trait Sealed {}
}

/// Broad extractor-rejection category represented by a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectionKind {
    /// JSON body extraction.
    Json,

    /// Path parameter extraction.
    Path,

    /// Query-string extraction.
    Query,

    /// Form extraction.
    Form,

    /// Required request-extension extraction.
    Extension,
}

impl RejectionKind {
    /// Stable short identifier used in structured attributes.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Path => "path",
            Self::Query => "query",
            Self::Form => "form",
            Self::Extension => "extension",
        }
    }

    /// Stable diagprint code for this rejection category.
    pub const fn diagnostic_code(self) -> &'static str {
        match self {
            Self::Json => "axum.rejection.json",
            Self::Path => "axum.rejection.path",
            Self::Query => "axum.rejection.query",
            Self::Form => "axum.rejection.form",
            Self::Extension => "axum.rejection.extension",
        }
    }
}

/// Converts supported Axum extractor rejections into structured diagnostics.
///
/// This trait is sealed so diagprint-axum can evolve its supported rejection
/// contract without creating an accidental third-party implementation API.
///
/// The generated diagnostic retains Axum's internal rejection detail for
/// server-side observability. Client responses remain redacted unless the
/// application explicitly changes [`ResponsePolicy`].
pub trait AxumRejectionExt: sealed::Sealed {
    /// Returns the broad rejection category.
    fn rejection_kind(&self) -> RejectionKind;

    /// Returns Axum's HTTP status for this rejection.
    fn rejection_status(&self) -> StatusCode;

    /// Returns Axum's detailed rejection text.
    ///
    /// This text is internal diagnostic material and should not automatically
    /// be exposed to HTTP clients.
    fn rejection_detail(&self) -> String;

    /// Converts this rejection into a structured diagprint diagnostic.
    fn to_diagnostic(&self, reporter: &Reporter) -> Diagnostic {
        let kind = self.rejection_kind();
        let status = self.rejection_status();
        let detail = self.rejection_detail();

        reporter
            .diagnostic(
                severity_for_status(status),
                format!(
                    "Axum {} extractor rejected request: {detail}",
                    kind.as_str()
                ),
            )
            .code(kind.diagnostic_code())
            .attribute("http.status", u64::from(status.as_u16()))
            .attribute("http.status_class", status_class(status))
            .attribute("axum.rejection.kind", kind.as_str())
    }

    /// Converts this rejection into the default privacy-safe HTTP response.
    fn to_diagnostic_response(&self, reporter: &Reporter) -> DiagnosticResponse {
        DiagnosticResponse::new(self.rejection_status(), self.to_diagnostic(reporter))
    }

    /// Converts this rejection using an explicit HTTP response policy.
    fn to_diagnostic_response_with_policy(
        &self,
        reporter: &Reporter,
        policy: ResponsePolicy,
    ) -> DiagnosticResponse {
        self.to_diagnostic_response(reporter).with_policy(policy)
    }
}

fn severity_for_status(status: StatusCode) -> Severity {
    if status.is_server_error() {
        Severity::Error
    } else {
        Severity::Warning
    }
}

fn status_class(status: StatusCode) -> &'static str {
    if status.is_informational() {
        "informational"
    } else if status.is_success() {
        "success"
    } else if status.is_redirection() {
        "redirection"
    } else if status.is_client_error() {
        "client_error"
    } else if status.is_server_error() {
        "server_error"
    } else {
        "unknown"
    }
}

macro_rules! implement_rejection {
    ($type:ty, $kind:expr) => {
        impl sealed::Sealed for $type {}

        impl AxumRejectionExt for $type {
            fn rejection_kind(&self) -> RejectionKind {
                $kind
            }

            fn rejection_status(&self) -> StatusCode {
                self.status()
            }

            fn rejection_detail(&self) -> String {
                self.body_text()
            }
        }
    };
}

implement_rejection!(JsonRejection, RejectionKind::Json);
implement_rejection!(PathRejection, RejectionKind::Path);
implement_rejection!(QueryRejection, RejectionKind::Query);
implement_rejection!(FormRejection, RejectionKind::Form);
implement_rejection!(ExtensionRejection, RejectionKind::Extension);
