use crate::{SnafuBridgeError, SnafuBridgeOutput};
use diagprint::{DiagnosticRelationshipGraph, DiagnosticReport};
use snafu::{Backtrace, ErrorCompat};
use std::{error::Error as StdError, fmt};

pub struct CapturedSnafuError<E> {
    error: E,
    capture: Box<Result<SnafuBridgeOutput, SnafuBridgeError>>,
}
impl<E> CapturedSnafuError<E> {
    pub(crate) fn new(error: E, capture: Result<SnafuBridgeOutput, SnafuBridgeError>) -> Self {
        Self {
            error,
            capture: Box::new(capture),
        }
    }
    pub const fn error(&self) -> &E {
        &self.error
    }
    pub fn capture(&self) -> &Result<SnafuBridgeOutput, SnafuBridgeError> {
        self.capture.as_ref()
    }
    pub fn output(&self) -> Result<&SnafuBridgeOutput, &SnafuBridgeError> {
        self.capture.as_ref().as_ref()
    }
    pub fn report(&self) -> Result<&DiagnosticReport, &SnafuBridgeError> {
        self.output().map(SnafuBridgeOutput::report)
    }
    pub fn graph(&self) -> Result<&DiagnosticRelationshipGraph, &SnafuBridgeError> {
        self.output().map(SnafuBridgeOutput::graph)
    }
    pub fn capture_error(&self) -> Option<&SnafuBridgeError> {
        self.capture.as_ref().as_ref().err()
    }
    pub fn into_error(self) -> E {
        self.error
    }
    pub fn into_parts(self) -> (E, Result<SnafuBridgeOutput, SnafuBridgeError>) {
        (self.error, *self.capture)
    }
}
impl<E> AsRef<E> for CapturedSnafuError<E> {
    fn as_ref(&self) -> &E {
        self.error()
    }
}
impl<E: fmt::Display> fmt::Display for CapturedSnafuError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}
impl<E: fmt::Debug> fmt::Debug for CapturedSnafuError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CapturedSnafuError")
            .field("error", &self.error)
            .field("capture_succeeded", &self.capture.as_ref().is_ok())
            .field("capture_error", &self.capture.as_ref().as_ref().err())
            .finish()
    }
}
impl<E: StdError + 'static> StdError for CapturedSnafuError<E> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&self.error)
    }
}
impl<E: ErrorCompat> ErrorCompat for CapturedSnafuError<E> {
    fn backtrace(&self) -> Option<&Backtrace> {
        ErrorCompat::backtrace(&self.error)
    }
}
