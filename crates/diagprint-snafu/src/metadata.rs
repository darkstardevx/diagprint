use crate::SnafuBridgeError;
use diagprint::{DocumentationLink, InteropDiagnostic, InteropLabel, Severity};
use diagprint_bridge::BridgeDiagnosticMetadata;
use std::fmt;

const MAX_IDENTITY_LEN: usize = 128;
const MAX_CODE_LEN: usize = 64;
const IDENTITY_PREFIX: &str = "snafu:";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SnafuIdentity(String);

impl SnafuIdentity {
    pub fn new(value: impl Into<String>) -> Result<Self, SnafuBridgeError> {
        let value = value.into();
        if valid_symbol(&value, MAX_IDENTITY_LEN) {
            Ok(Self(value))
        } else {
            Err(SnafuBridgeError::InvalidIdentity)
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub(crate) fn namespaced(&self) -> String {
        format!("{IDENTITY_PREFIX}{}", self.0)
    }
}

impl fmt::Display for SnafuIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SnafuCode(String);

impl SnafuCode {
    pub fn new(value: impl Into<String>) -> Result<Self, SnafuBridgeError> {
        let value = value.into();
        if valid_symbol(&value, MAX_CODE_LEN) {
            Ok(Self(value))
        } else {
            Err(SnafuBridgeError::InvalidCode)
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SnafuCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone)]
pub struct SnafuDiagnosticMetadata {
    identity: SnafuIdentity,
    code: Option<SnafuCode>,
    diagnostic: InteropDiagnostic,
}

impl SnafuDiagnosticMetadata {
    pub fn new(identity: SnafuIdentity, message: impl Into<String>) -> Self {
        Self {
            identity,
            code: None,
            diagnostic: InteropDiagnostic::new(message),
        }
    }
    pub fn identity(&self) -> &SnafuIdentity {
        &self.identity
    }
    pub fn code_value(&self) -> Option<&SnafuCode> {
        self.code.as_ref()
    }
    pub fn message(&self) -> &str {
        &self.diagnostic.message
    }
    pub fn severity(mut self, severity: Severity) -> Self {
        self.diagnostic = self.diagnostic.severity(severity);
        self
    }
    pub fn code(mut self, code: SnafuCode) -> Self {
        self.diagnostic = self.diagnostic.code(code.as_str());
        self.code = Some(code);
        self
    }
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.diagnostic = self.diagnostic.help(help);
        self
    }
    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.diagnostic = self.diagnostic.note(note);
        self
    }
    pub fn label(mut self, label: InteropLabel) -> Self {
        self.diagnostic = self.diagnostic.label(label);
        self
    }
    pub fn documentation(mut self, link: DocumentationLink) -> Self {
        self.diagnostic = self.diagnostic.documentation(link);
        self
    }
    pub fn into_bridge_metadata(self) -> Result<BridgeDiagnosticMetadata, SnafuBridgeError> {
        Ok(BridgeDiagnosticMetadata::from_interop(self.diagnostic)
            .identity(self.identity.namespaced())?)
    }
}

#[derive(Debug, Clone)]
pub struct WhateverDiagnosticContext {
    metadata: SnafuDiagnosticMetadata,
}

impl WhateverDiagnosticContext {
    pub fn new(identity: SnafuIdentity, message: impl Into<String>) -> Self {
        Self {
            metadata: SnafuDiagnosticMetadata::new(identity, message),
        }
    }
    pub fn identity(&self) -> &SnafuIdentity {
        self.metadata.identity()
    }
    pub fn code_value(&self) -> Option<&SnafuCode> {
        self.metadata.code_value()
    }
    pub fn message(&self) -> &str {
        self.metadata.message()
    }
    pub fn code(mut self, code: SnafuCode) -> Self {
        self.metadata = self.metadata.code(code);
        self
    }
    pub fn severity(mut self, severity: Severity) -> Self {
        self.metadata = self.metadata.severity(severity);
        self
    }
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.metadata = self.metadata.help(help);
        self
    }
    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.metadata = self.metadata.note(note);
        self
    }
    pub fn into_metadata(self) -> SnafuDiagnosticMetadata {
        self.metadata
    }

    pub(crate) fn into_parts(self) -> (String, SnafuDiagnosticMetadata) {
        let message = self.metadata.message().to_owned();
        (message, self.metadata)
    }
}

fn valid_symbol(value: &str, max_len: usize) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > max_len || !bytes[0].is_ascii_alphanumeric() {
        return false;
    }
    bytes
        .iter()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b':'))
}
