use axum::{
    Json,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use diagprint::Diagnostic;
use serde::Serialize;

/// Policy controlling which diagnostic information may cross the HTTP
/// response boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponsePolicy {
    expose_diagnostic_message: bool,
    expose_diagnostic_code: bool,
    include_report_id: bool,
    client_error_message: String,
    server_error_message: String,
}

impl Default for ResponsePolicy {
    fn default() -> Self {
        Self {
            expose_diagnostic_message: false,
            expose_diagnostic_code: false,
            include_report_id: true,
            client_error_message: "Request could not be completed.".into(),
            server_error_message: "Internal server error.".into(),
        }
    }
}

impl ResponsePolicy {
    /// Returns the conservative default policy.
    pub fn redacted() -> Self {
        Self::default()
    }

    /// Controls whether the internal diagnostic message may reach the client.
    pub fn expose_diagnostic_message(mut self, expose: bool) -> Self {
        self.expose_diagnostic_message = expose;
        self
    }

    /// Controls whether the diagnostic code may reach the client.
    pub fn expose_diagnostic_code(mut self, expose: bool) -> Self {
        self.expose_diagnostic_code = expose;
        self
    }

    /// Controls whether the report ID is included for correlation.
    pub fn include_report_id(mut self, include: bool) -> Self {
        self.include_report_id = include;
        self
    }

    /// Replaces the default generic message used for non-5xx responses.
    pub fn client_error_message(mut self, message: impl Into<String>) -> Self {
        self.client_error_message = message.into();
        self
    }

    /// Replaces the default generic message used for 5xx responses.
    pub fn server_error_message(mut self, message: impl Into<String>) -> Self {
        self.server_error_message = message.into();
        self
    }

    pub(crate) fn fallback_message(&self, status: StatusCode) -> &str {
        if status.is_server_error() {
            &self.server_error_message
        } else {
            &self.client_error_message
        }
    }
}

/// JSON envelope returned to an HTTP client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClientErrorEnvelope {
    /// Client-visible error information.
    pub error: ClientErrorBody,
}

/// Privacy-filtered diagnostic information returned to an HTTP client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClientErrorBody {
    /// HTTP status code as an integer.
    pub status: u16,

    /// Client-safe message selected by [`ResponsePolicy`].
    pub message: String,

    /// diagprint report ID for internal correlation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_id: Option<String>,

    /// Optional public diagnostic code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// Axum HTTP response backed by one structured diagprint diagnostic.
///
/// Creating this wrapper does not emit, log, persist, or otherwise forward the
/// internal diagnostic.
#[derive(Debug, Clone)]
pub struct DiagnosticResponse {
    status: StatusCode,
    diagnostic: Box<Diagnostic>,
    policy: ResponsePolicy,
}

impl DiagnosticResponse {
    /// Creates a response using the privacy-safe default policy.
    pub fn new(status: StatusCode, diagnostic: Diagnostic) -> Self {
        Self {
            status,
            diagnostic: Box::new(diagnostic),
            policy: ResponsePolicy::default(),
        }
    }

    /// Replaces the HTTP response policy.
    pub fn with_policy(mut self, policy: ResponsePolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Returns the HTTP status.
    pub const fn status(&self) -> StatusCode {
        self.status
    }

    /// Returns the underlying internal diagnostic.
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    /// Returns the current client-response policy.
    pub const fn policy(&self) -> &ResponsePolicy {
        &self.policy
    }

    /// Consumes the wrapper and returns the internal diagnostic.
    pub fn into_diagnostic(self) -> Diagnostic {
        *self.diagnostic
    }

    /// Builds the privacy-filtered client body.
    pub fn client_body(&self) -> ClientErrorEnvelope {
        let message = if self.policy.expose_diagnostic_message {
            self.diagnostic.message.clone()
        } else {
            self.policy.fallback_message(self.status).to_owned()
        };

        let report_id = if self.policy.include_report_id {
            Some(self.diagnostic.report_id.to_string())
        } else {
            None
        };

        let code = if self.policy.expose_diagnostic_code {
            self.diagnostic.code.clone()
        } else {
            None
        };

        ClientErrorEnvelope {
            error: ClientErrorBody {
                status: self.status.as_u16(),
                message,
                report_id,
                code,
            },
        }
    }
}

impl IntoResponse for DiagnosticResponse {
    fn into_response(self) -> Response {
        let status = self.status;
        let body = self.client_body();

        let mut response = (status, Json(body)).into_response();

        response.headers_mut().insert(
            header::CACHE_CONTROL,
            header::HeaderValue::from_static("no-store"),
        );

        response
    }
}

/// Convenience conversion from a diagnostic into an HTTP response.
pub trait DiagnosticResponseExt {
    /// Wraps this diagnostic in a privacy-safe Axum response.
    fn into_http_response(self, status: StatusCode) -> DiagnosticResponse;
}

impl DiagnosticResponseExt for Diagnostic {
    fn into_http_response(self, status: StatusCode) -> DiagnosticResponse {
        DiagnosticResponse::new(status, self)
    }
}

impl From<(StatusCode, Diagnostic)> for DiagnosticResponse {
    fn from((status, diagnostic): (StatusCode, Diagnostic)) -> Self {
        Self::new(status, diagnostic)
    }
}

/// Handler result whose error side is a privacy-safe diagnostic response.
pub type DiagnosticResult<T> = Result<T, DiagnosticResponse>;
