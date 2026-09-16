use crate::DiagnosticResponse;
use axum::{
    Json,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Serialize;

/// RFC 9457 media type for JSON Problem Details responses.
pub const PROBLEM_JSON_MEDIA_TYPE: &str = "application/problem+json";

/// RFC 9457 generic problem type.
///
/// `about:blank` means that the problem has no semantics beyond the HTTP
/// status code.
pub const ABOUT_BLANK: &str = "about:blank";

/// Policy controlling the RFC 9457 representation of a diagnostic response.
///
/// Diagnostic disclosure remains controlled by the underlying
/// [`crate::ResponsePolicy`]. This policy only controls Problem Details
/// representation metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProblemDetailsPolicy {
    type_uri: String,
    title: Option<String>,
    instance: Option<String>,
    include_request_id: bool,
}

impl Default for ProblemDetailsPolicy {
    fn default() -> Self {
        Self {
            type_uri: ABOUT_BLANK.to_owned(),
            title: None,
            instance: None,
            include_request_id: true,
        }
    }
}

impl ProblemDetailsPolicy {
    /// Returns the conservative RFC 9457 `about:blank` policy.
    pub fn about_blank() -> Self {
        Self::default()
    }

    /// Replaces the problem type URI.
    ///
    /// Applications defining custom problem types should use a stable URI
    /// whose documentation describes the type's semantics.
    pub fn with_type_uri(mut self, type_uri: impl Into<String>) -> Self {
        self.type_uri = type_uri.into();
        self
    }

    /// Replaces the human-readable problem title.
    ///
    /// When omitted, the HTTP status code's canonical reason phrase is used.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Sets the RFC 9457 problem-instance URI reference.
    ///
    /// This value identifies this particular problem occurrence. It is not
    /// automatically populated from a diagprint report ID or HTTP request ID.
    pub fn with_instance(mut self, instance: impl Into<String>) -> Self {
        self.instance = Some(instance.into());
        self
    }

    /// Controls whether a supplied HTTP request ID is included as an extension
    /// member.
    pub fn include_request_id(mut self, include: bool) -> Self {
        self.include_request_id = include;
        self
    }

    /// Returns the configured problem type URI.
    pub fn type_uri(&self) -> &str {
        &self.type_uri
    }

    /// Returns the configured title override.
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Returns the configured problem-instance URI reference.
    pub fn instance(&self) -> Option<&str> {
        self.instance.as_deref()
    }
}

/// RFC 9457 Problem Details JSON document.
///
/// `report_id`, `request_id`, and `code` are extension members. They remain
/// subject to diagprint's response-disclosure policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProblemDetails {
    /// URI reference identifying the problem type.
    #[serde(rename = "type")]
    pub type_uri: String,

    /// Short, human-readable summary of the problem type.
    pub title: String,

    /// HTTP status code used for this response.
    pub status: u16,

    /// Privacy-filtered human-readable explanation of this occurrence.
    pub detail: String,

    /// Optional URI reference identifying this specific problem occurrence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,

    /// Optional diagprint report ID for diagnostic correlation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_id: Option<String>,

    /// Optional HTTP request ID for request-level correlation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,

    /// Optional public diagnostic code.
    ///
    /// This is omitted unless diagnostic-code exposure is explicitly enabled
    /// by the underlying response policy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// Axum response that renders a [`DiagnosticResponse`] as RFC 9457 Problem
/// Details.
///
/// Creating or rendering this response does not emit, log, persist, or
/// otherwise forward the underlying diagnostic.
#[derive(Debug, Clone)]
pub struct ProblemDetailsResponse {
    response: DiagnosticResponse,
    problem_policy: Box<ProblemDetailsPolicy>,
    request_id: Option<String>,
}

impl ProblemDetailsResponse {
    /// Creates an RFC 9457 representation using the default Problem Details
    /// policy.
    pub fn new(response: DiagnosticResponse) -> Self {
        Self {
            response,
            problem_policy: Box::new(ProblemDetailsPolicy::default()),
            request_id: None,
        }
    }

    /// Replaces the RFC 9457 representation policy.
    pub fn with_problem_policy(mut self, policy: ProblemDetailsPolicy) -> Self {
        self.problem_policy = Box::new(policy);
        self
    }

    /// Associates an HTTP request ID with this response.
    ///
    /// Request IDs and diagprint report IDs are intentionally separate
    /// correlation identifiers.
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    /// Returns the underlying HTTP status code.
    pub const fn status(&self) -> StatusCode {
        self.response.status()
    }

    /// Returns the underlying diagnostic response.
    pub const fn diagnostic_response(&self) -> &DiagnosticResponse {
        &self.response
    }

    /// Returns the Problem Details representation policy.
    pub const fn problem_policy(&self) -> &ProblemDetailsPolicy {
        &self.problem_policy
    }

    /// Consumes the wrapper and returns the underlying diagnostic response.
    pub fn into_diagnostic_response(self) -> DiagnosticResponse {
        self.response
    }

    /// Builds the privacy-filtered RFC 9457 Problem Details document.
    pub fn problem_details(&self) -> ProblemDetails {
        let status = self.response.status();
        let client = self.response.client_body();

        let title = self
            .problem_policy
            .title
            .clone()
            .unwrap_or_else(|| status_title(status).to_owned());

        let request_id = if self.problem_policy.include_request_id {
            self.request_id.clone()
        } else {
            None
        };

        ProblemDetails {
            type_uri: self.problem_policy.type_uri.clone(),
            title,
            status: status.as_u16(),
            detail: client.error.message,
            instance: self.problem_policy.instance.clone(),
            report_id: client.error.report_id,
            request_id,
            code: client.error.code,
        }
    }
}

impl From<DiagnosticResponse> for ProblemDetailsResponse {
    fn from(response: DiagnosticResponse) -> Self {
        Self::new(response)
    }
}

impl IntoResponse for ProblemDetailsResponse {
    fn into_response(self) -> Response {
        let status = self.status();
        let body = self.problem_details();

        let mut response = (status, Json(body)).into_response();

        response.headers_mut().insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static(PROBLEM_JSON_MEDIA_TYPE),
        );

        response.headers_mut().insert(
            header::CACHE_CONTROL,
            header::HeaderValue::from_static("no-store"),
        );

        response
    }
}

/// Extension trait for converting an existing [`DiagnosticResponse`] into an
/// RFC 9457 response.
pub trait ProblemDetailsResponseExt {
    /// Converts this diagnostic response into RFC 9457 Problem Details.
    fn into_problem_details(self) -> ProblemDetailsResponse;
}

impl ProblemDetailsResponseExt for DiagnosticResponse {
    fn into_problem_details(self) -> ProblemDetailsResponse {
        ProblemDetailsResponse::new(self)
    }
}

fn status_title(status: StatusCode) -> &'static str {
    status.canonical_reason().unwrap_or("HTTP Error")
}
