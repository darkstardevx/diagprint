use axum::{
    extract::{FromRequestParts, MatchedPath, Request},
    http::{HeaderValue, Method, StatusCode, request::Parts},
    middleware::Next,
    response::{IntoResponse, Response},
};
use diagprint::Diagnostic;
use std::{error::Error, fmt};
use uuid::Uuid;

/// HTTP header used for request-level correlation.
pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Maximum accepted length of an inbound request ID.
pub const MAX_REQUEST_ID_LEN: usize = 128;

/// Error returned when a request ID does not satisfy the correlation-ID
/// contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidRequestId;

impl fmt::Display for InvalidRequestId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "request IDs must be 1 to 128 ASCII characters and contain only \
             letters, digits, '-', '_', '.', or ':'",
        )
    }
}

impl Error for InvalidRequestId {}

/// Error returned when a handler requests [`RequestContext`] without the
/// request-correlation middleware having established one.
///
/// This normally indicates an application wiring error: the route was not
/// wrapped in [`request_context_middleware`].
///
/// The Axum rejection intentionally exposes no internal configuration details
/// to the HTTP client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissingRequestContext;

impl fmt::Display for MissingRequestContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "request context is unavailable; ensure request_context_middleware is installed",
        )
    }
}

impl Error for MissingRequestContext {}

impl IntoResponse for MissingRequestContext {
    fn into_response(self) -> Response {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

/// Validated request-level correlation identifier.
///
/// Request IDs are intentionally separate from diagprint report IDs. A request
/// can produce multiple diagnostics, while each diagnostic retains its own
/// report identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RequestId(String);

impl RequestId {
    /// Validates and constructs a request ID.
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidRequestId> {
        let value = value.into();

        if !is_valid_request_id(&value) {
            return Err(InvalidRequestId);
        }

        Ok(Self(value))
    }

    /// Generates a new UUIDv7 request ID.
    pub fn generate() -> Self {
        Self(Uuid::now_v7().to_string())
    }

    /// Returns the request ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the request ID and returns its owned string.
    pub fn into_inner(self) -> String {
        self.0
    }

    fn header_value(&self) -> HeaderValue {
        HeaderValue::try_from(self.0.as_str())
            .expect("validated request ID must always be a valid HTTP header value")
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Privacy-safe HTTP request context attached by
/// [`request_context_middleware`].
///
/// The context deliberately stores only:
///
/// - the request ID;
/// - HTTP method;
/// - Axum's matched route pattern, when available.
///
/// It does not capture the raw request URI, query string, request body,
/// cookies, authorization headers, or arbitrary request headers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestContext {
    request_id: RequestId,
    method: Method,
    matched_route: Option<String>,
}

impl RequestContext {
    /// Creates request context from already validated components.
    pub fn new(request_id: RequestId, method: Method, matched_route: Option<String>) -> Self {
        Self {
            request_id,
            method,
            matched_route,
        }
    }

    /// Returns the request-level correlation ID.
    pub fn request_id(&self) -> &RequestId {
        &self.request_id
    }

    /// Returns the HTTP request method.
    pub fn method(&self) -> &Method {
        &self.method
    }

    /// Returns Axum's matched route pattern when available.
    ///
    /// This is a route template such as `/users/{id}`, not the raw request
    /// path.
    pub fn matched_route(&self) -> Option<&str> {
        self.matched_route.as_deref()
    }

    /// Adds safe HTTP correlation attributes to a diagnostic.
    ///
    /// The following structured attributes are added:
    ///
    /// - `http.request_id`
    /// - `http.method`
    /// - `http.route`, when Axum supplied a matched route
    pub fn annotate_diagnostic(&self, diagnostic: Diagnostic) -> Diagnostic {
        let diagnostic = diagnostic
            .attribute("http.request_id", self.request_id.as_str())
            .attribute("http.method", self.method.as_str());

        match self.matched_route() {
            Some(route) => diagnostic.attribute("http.route", route),
            None => diagnostic,
        }
    }
}

impl<S> FromRequestParts<S> for RequestContext
where
    S: Send + Sync,
{
    type Rejection = MissingRequestContext;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Self>()
            .cloned()
            .ok_or(MissingRequestContext)
    }
}

/// Axum middleware that establishes privacy-safe request correlation.
///
/// A valid inbound `x-request-id` value is preserved. Missing or invalid
/// values are replaced with a generated UUIDv7 identifier.
///
/// The resulting [`RequestContext`] is inserted into request extensions and
/// the selected request ID is propagated back through the response
/// `x-request-id` header.
///
/// This middleware does not emit or persist diagnostics.
pub async fn request_context_middleware(mut request: Request, next: Next) -> Response {
    let request_id = request
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| RequestId::new(value.to_owned()).ok())
        .unwrap_or_else(RequestId::generate);

    let matched_route = request
        .extensions()
        .get::<MatchedPath>()
        .map(|matched| matched.as_str().to_owned());

    let context = RequestContext::new(request_id, request.method().clone(), matched_route);

    request.extensions_mut().insert(context.clone());

    let mut response = next.run(request).await;

    response
        .headers_mut()
        .insert(REQUEST_ID_HEADER, context.request_id().header_value());

    response
}

fn is_valid_request_id(value: &str) -> bool {
    let bytes = value.as_bytes();

    !bytes.is_empty()
        && bytes.len() <= MAX_REQUEST_ID_LEN
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_' | b'.' | b':'))
}
