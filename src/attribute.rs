use serde::Serialize;
use std::fmt;

/// Typed value attached to a structured diagnostic attribute.
///
/// Values remain typed in JSON and integrations instead of being flattened
/// into formatted strings.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum DiagnosticValue {
    String(String),
    Bool(bool),
    I64(i64),
    U64(u64),
    I128(i128),
    U128(u128),
    F64(f64),
}

impl fmt::Display for DiagnosticValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String(value) => formatter.write_str(value),

            Self::Bool(value) => {
                write!(formatter, "{value}")
            }

            Self::I64(value) => {
                write!(formatter, "{value}")
            }

            Self::U64(value) => {
                write!(formatter, "{value}")
            }

            Self::I128(value) => {
                write!(formatter, "{value}")
            }

            Self::U128(value) => {
                write!(formatter, "{value}")
            }

            Self::F64(value) => {
                write!(formatter, "{value}")
            }
        }
    }
}

impl From<String> for DiagnosticValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for DiagnosticValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<bool> for DiagnosticValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i64> for DiagnosticValue {
    fn from(value: i64) -> Self {
        Self::I64(value)
    }
}

impl From<u64> for DiagnosticValue {
    fn from(value: u64) -> Self {
        Self::U64(value)
    }
}

impl From<i128> for DiagnosticValue {
    fn from(value: i128) -> Self {
        Self::I128(value)
    }
}

impl From<u128> for DiagnosticValue {
    fn from(value: u128) -> Self {
        Self::U128(value)
    }
}

impl From<f64> for DiagnosticValue {
    fn from(value: f64) -> Self {
        Self::F64(value)
    }
}

/// Named structured value attached to a diagnostic.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DiagnosticAttribute {
    pub name: String,
    pub value: DiagnosticValue,
}

impl DiagnosticAttribute {
    pub fn new(name: impl Into<String>, value: impl Into<DiagnosticValue>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}
