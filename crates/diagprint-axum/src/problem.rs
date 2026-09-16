use crate::DiagnosticResponse;
use axum::{
    Json,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Serialize;
use serde_json::Value;
use std::{collections::BTreeMap, error::Error, fmt};

/// RFC 9457 media type for JSON Problem Details responses.
pub const PROBLEM_JSON_MEDIA_TYPE: &str = "application/problem+json";

/// RFC 9457 generic problem type.
///
/// `about:blank` means that the problem has no semantics beyond the HTTP
/// status code.
pub const ABOUT_BLANK: &str = "about:blank";

const RESERVED_EXTENSION_NAMES: &[&str] = &[
    "type",
    "title",
    "status",
    "detail",
    "instance",
    "report_id",
    "request_id",
    "code",
];

/// Error returned when an RFC 9457 extension member cannot be added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProblemExtensionError {
    /// The extension name does not follow the portable naming rules enforced
    /// by diagprint-axum.
    InvalidName(String),

    /// The extension name is reserved by RFC 9457 or diagprint-axum.
    ReservedName(String),

    /// The extension name has already been defined for this problem response.
    DuplicateKey(String),
}

impl fmt::Display for ProblemExtensionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(name) => write!(
                f,
                "invalid Problem Details extension name `{name}`; names must be at least \
                 three ASCII characters, start with a letter, and contain only letters, \
                 digits, or `_`"
            ),
            Self::ReservedName(name) => {
                write!(f, "Problem Details extension name `{name}` is reserved")
            }
            Self::DuplicateKey(name) => {
                write!(
                    f,
                    "Problem Details extension name `{name}` is already defined"
                )
            }
        }
    }
}

impl Error for ProblemExtensionError {}

/// Policy controlling the RFC 9457 representation of a diagnostic response.
///
/// Diagnostic disclosure remains controlled by the underlying
/// [`crate::ResponsePolicy`]. This policy only controls Problem Details
/// representation metadata and whether explicitly registered internal
/// extensions may cross the HTTP boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProblemDetailsPolicy {
    type_uri: String,
    title: Option<String>,
    instance: Option<String>,
    include_request_id: bool,
    expose_internal_extensions: bool,
}

impl Default for ProblemDetailsPolicy {
    fn default() -> Self {
        Self {
            type_uri: ABOUT_BLANK.to_owned(),
            title: None,
            instance: None,
            include_request_id: true,
            expose_internal_extensions: false,
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

    /// Controls whether explicitly registered internal extensions may cross
    /// the HTTP boundary.
    ///
    /// Internal extensions are redacted by default.
    pub fn expose_internal_extensions(mut self, expose: bool) -> Self {
        self.expose_internal_extensions = expose;
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

    /// Returns whether internal extensions are allowed across the HTTP
    /// boundary.
    pub const fn internal_extensions_exposed(&self) -> bool {
        self.expose_internal_extensions
    }
}

/// RFC 9457 Problem Details JSON document.
///
/// `report_id`, `request_id`, and `code` are diagprint extension members.
/// Additional application-defined members are serialized at the root level of
/// the JSON object.
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

    #[serde(flatten)]
    extensions: BTreeMap<String, Value>,
}

impl ProblemDetails {
    /// Returns the custom RFC 9457 extension members included in this
    /// representation.
    pub fn extensions(&self) -> &BTreeMap<String, Value> {
        &self.extensions
    }
}

#[derive(Debug, Clone)]
struct ProblemDetailsState {
    problem_policy: ProblemDetailsPolicy,
    request_id: Option<String>,
    public_extensions: BTreeMap<String, Value>,
    internal_extensions: BTreeMap<String, Value>,
}

/// Axum response that renders a [`DiagnosticResponse`] as RFC 9457 Problem
/// Details.
///
/// Creating or rendering this response does not emit, log, persist, or
/// otherwise forward the underlying diagnostic.
#[derive(Debug, Clone)]
pub struct ProblemDetailsResponse {
    response: DiagnosticResponse,
    state: Box<ProblemDetailsState>,
}

impl ProblemDetailsResponse {
    /// Creates an RFC 9457 representation using the default Problem Details
    /// policy.
    pub fn new(response: DiagnosticResponse) -> Self {
        Self {
            response,
            state: Box::new(ProblemDetailsState {
                problem_policy: ProblemDetailsPolicy::default(),
                request_id: None,
                public_extensions: BTreeMap::new(),
                internal_extensions: BTreeMap::new(),
            }),
        }
    }

    /// Replaces the RFC 9457 representation policy.
    pub fn with_problem_policy(mut self, policy: ProblemDetailsPolicy) -> Self {
        self.state.problem_policy = policy;
        self
    }

    /// Associates an HTTP request ID with this response.
    ///
    /// Request IDs and diagprint report IDs are intentionally separate
    /// correlation identifiers.
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.state.request_id = Some(request_id.into());
        self
    }

    /// Adds a public root-level RFC 9457 extension member.
    ///
    /// Extension names are validated against the portable RFC 9457 naming
    /// recommendation. Reserved names and duplicate names are rejected.
    ///
    /// Duplicate detection spans both public and internal extensions so one
    /// JSON member can never silently replace another.
    pub fn with_extension(
        mut self,
        name: impl Into<String>,
        value: Value,
    ) -> Result<Self, ProblemExtensionError> {
        let name = name.into();
        self.validate_new_extension(&name)?;
        self.state.public_extensions.insert(name, value);
        Ok(self)
    }

    /// Adds an internal root-level extension member.
    ///
    /// Internal extensions are retained by the response object but are not
    /// serialized unless [`ProblemDetailsPolicy::expose_internal_extensions`]
    /// is explicitly enabled.
    ///
    /// Reserved names and duplicate names are rejected.
    pub fn with_internal_extension(
        mut self,
        name: impl Into<String>,
        value: Value,
    ) -> Result<Self, ProblemExtensionError> {
        let name = name.into();
        self.validate_new_extension(&name)?;
        self.state.internal_extensions.insert(name, value);
        Ok(self)
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
    pub fn problem_policy(&self) -> &ProblemDetailsPolicy {
        &self.state.problem_policy
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
            .state
            .problem_policy
            .title
            .clone()
            .unwrap_or_else(|| status_title(status).to_owned());

        let request_id = if self.state.problem_policy.include_request_id {
            self.state.request_id.clone()
        } else {
            None
        };

        let mut extensions = self.state.public_extensions.clone();

        if self.state.problem_policy.expose_internal_extensions {
            for (name, value) in &self.state.internal_extensions {
                let previous = extensions.insert(name.clone(), value.clone());
                debug_assert!(
                    previous.is_none(),
                    "duplicate Problem Details extension escaped construction validation"
                );
            }
        }

        ProblemDetails {
            type_uri: self.state.problem_policy.type_uri.clone(),
            title,
            status: status.as_u16(),
            detail: client.error.message,
            instance: self.state.problem_policy.instance.clone(),
            report_id: client.error.report_id,
            request_id,
            code: client.error.code,
            extensions,
        }
    }

    fn validate_new_extension(&self, name: &str) -> Result<(), ProblemExtensionError> {
        validate_extension_name(name)?;

        if RESERVED_EXTENSION_NAMES.contains(&name) {
            return Err(ProblemExtensionError::ReservedName(name.to_owned()));
        }

        if self.state.public_extensions.contains_key(name)
            || self.state.internal_extensions.contains_key(name)
        {
            return Err(ProblemExtensionError::DuplicateKey(name.to_owned()));
        }

        Ok(())
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

fn validate_extension_name(name: &str) -> Result<(), ProblemExtensionError> {
    let bytes = name.as_bytes();

    if bytes.len() < 3
        || !bytes
            .first()
            .is_some_and(|first| first.is_ascii_alphabetic())
        || !bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    {
        return Err(ProblemExtensionError::InvalidName(name.to_owned()));
    }

    Ok(())
}

fn status_title(status: StatusCode) -> &'static str {
    status.canonical_reason().unwrap_or("HTTP Error")
}
