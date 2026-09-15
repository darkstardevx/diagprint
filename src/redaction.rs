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
