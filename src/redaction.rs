use serde::{Serialize, Serializer};
use std::fmt;

/// Default representation for protected values.
pub const REDACTED: &str = "[REDACTED]";

/// A value which must be explicitly exposed before its contents are rendered.
///
/// `Display`, `Debug`, and `Serialize` never reveal the wrapped value.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sensitive<T> {
    value: T,
}

impl<T> Sensitive<T> {
    /// Wraps a sensitive value.
    pub const fn new(value: T) -> Self {
        Self { value }
    }

    /// Explicitly borrows the underlying value.
    pub const fn expose(&self) -> &T {
        &self.value
    }

    /// Explicitly consumes the wrapper and returns its underlying value.
    pub fn into_inner(self) -> T {
        self.value
    }

    /// Transforms the protected value while preserving protection.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Sensitive<U> {
        Sensitive::new(f(self.value))
    }
}

impl<T> From<T> for Sensitive<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T> fmt::Display for Sensitive<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(REDACTED)
    }
}

impl<T> fmt::Debug for Sensitive<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Sensitive([REDACTED])")
    }
}

impl<T> Serialize for Sensitive<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(REDACTED)
    }
}

/// Explicit policy for presenting a [`Sensitive`] value.
///
/// Serialization of `Sensitive<T>` remains redacted regardless of this policy.
/// Revealing data requires an explicit call through this policy or `expose()`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RedactionPolicy {
    /// Hide protected data.
    #[default]
    Redact,

    /// Explicitly expose protected data.
    Reveal,
}

impl RedactionPolicy {
    /// Formats a protected value according to this policy.
    pub fn format<T>(self, value: &Sensitive<T>) -> String
    where
        T: fmt::Display,
    {
        match self {
            Self::Redact => REDACTED.to_owned(),
            Self::Reveal => value.expose().to_string(),
        }
    }
}

/// Returns whether an attribute/key name commonly represents sensitive data.
///
/// Matching is ASCII case-insensitive and recognizes both complete normalized
/// names such as `api_key` and components such as `http.authorization`.
pub fn is_sensitive_key(name: &str) -> bool {
    let normalized: String = name
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();

    if matches!(
        normalized.as_str(),
        "password"
            | "passwd"
            | "token"
            | "accesstoken"
            | "refreshtoken"
            | "secret"
            | "clientsecret"
            | "authorization"
            | "cookie"
            | "setcookie"
            | "apikey"
    ) {
        return true;
    }

    name.split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .any(|part| {
            let part = part.to_ascii_lowercase();

            matches!(
                part.as_str(),
                "password" | "passwd" | "token" | "secret" | "authorization" | "cookie"
            )
        })
}

/// Reduces a filesystem path or source name to its final component.
///
/// This prevents external exports from revealing parent directories such as
/// usernames, home directories, workspace names, or temporary directories.
pub fn sanitize_path(value: &str) -> String {
    std::path::Path::new(value)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| REDACTED.to_owned())
}

/// Removes URL user information, query parameters, and fragments.
///
/// The sanitizer intentionally performs a small dependency-free transformation
/// rather than introducing a URL parser into diagprint core.
///
/// For example:
///
/// `https://alice:secret@example.com/docs?token=abc#private`
///
/// becomes:
///
/// `https://example.com/docs`
pub fn sanitize_url(value: &str) -> String {
    let without_fragment = value.split_once('#').map_or(value, |(head, _)| head);

    let without_query = without_fragment
        .split_once('?')
        .map_or(without_fragment, |(head, _)| head);

    let Some((scheme, rest)) = without_query.split_once("://") else {
        return without_query.to_owned();
    };

    let (authority, path) = match rest.split_once('/') {
        Some((authority, path)) => (authority, Some(path)),

        None => (rest, None),
    };

    let safe_authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);

    match path {
        Some(path) => {
            format!("{scheme}://{safe_authority}/{path}")
        }

        None => {
            format!("{scheme}://{safe_authority}")
        }
    }
}
