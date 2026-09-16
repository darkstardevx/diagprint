use sha2::{Digest as _, Sha256};
use std::{error::Error, fmt, path::Path};

/// Stable canonicalization namespace used by diagprint's first identity schema.
pub const CANONICAL_V1_NAMESPACE: &str = "diagprint.canonical/v1";

/// Version of diagprint's canonical identity encoding.
///
/// Existing versions are immutable. Any future semantic or encoding change
/// must introduce a new version rather than changing `V1` in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalizationVersion {
    V1,
}

impl CanonicalizationVersion {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V1 => CANONICAL_V1_NAMESPACE,
        }
    }
}

impl fmt::Display for CanonicalizationVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Failure while projecting diagnostic data into a portable canonical form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalizationError {
    /// Remediation paths must be valid UTF-8 in canonical v1 so identical
    /// canonical bytes have the same meaning across supported platforms.
    NonUtf8Path { display: String },
}

impl fmt::Display for CanonicalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonUtf8Path { display } => write!(
                formatter,
                "diagprint.canonical/v1 cannot encode non-UTF-8 remediation path: {display}"
            ),
        }
    }
}

impl Error for CanonicalizationError {}

pub(crate) fn canonical_path(path: &Path) -> Result<&str, CanonicalizationError> {
    path.to_str()
        .ok_or_else(|| CanonicalizationError::NonUtf8Path {
            // This display value is diagnostic text for the error only.
            // It is never used as canonical-v1 hash input.
            display: path.display().to_string(),
        })
}

pub(crate) struct CanonicalHasher {
    hasher: Sha256,
}

impl CanonicalHasher {
    pub(crate) fn new(domain: &str) -> Self {
        let mut value = Self {
            hasher: Sha256::new(),
        };

        value.token(CANONICAL_V1_NAMESPACE.as_bytes());
        value.token(domain.as_bytes());
        value
    }

    pub(crate) fn finish(self) -> [u8; 32] {
        let digest = self.hasher.finalize();
        let mut bytes = [0_u8; 32];
        bytes.copy_from_slice(&digest);
        bytes
    }

    pub(crate) fn field(&mut self, name: &str) {
        self.token(name.as_bytes());
    }

    pub(crate) fn string(&mut self, name: &str, value: &str) {
        self.field(name);
        self.token(value.as_bytes());
    }

    pub(crate) fn bytes(&mut self, name: &str, value: &[u8]) {
        self.field(name);
        self.token(value);
    }

    pub(crate) fn bool(&mut self, name: &str, value: bool) {
        self.field(name);
        self.raw(&[u8::from(value)]);
    }

    pub(crate) fn u32(&mut self, name: &str, value: u32) {
        self.field(name);
        self.raw(&value.to_be_bytes());
    }

    pub(crate) fn u64(&mut self, name: &str, value: u64) {
        self.field(name);
        self.raw(&value.to_be_bytes());
    }

    pub(crate) fn u128(&mut self, name: &str, value: u128) {
        self.field(name);
        self.raw(&value.to_be_bytes());
    }

    pub(crate) fn i64(&mut self, name: &str, value: i64) {
        self.field(name);
        self.raw(&value.to_be_bytes());
    }

    pub(crate) fn i128(&mut self, name: &str, value: i128) {
        self.field(name);
        self.raw(&value.to_be_bytes());
    }

    pub(crate) fn f64(&mut self, name: &str, value: f64) {
        self.field(name);
        self.raw(&canonical_f64_bits(value).to_be_bytes());
    }

    pub(crate) fn option_none(&mut self, name: &str) {
        self.field(name);
        self.raw(&[0]);
    }

    pub(crate) fn option_some(&mut self, name: &str) {
        self.field(name);
        self.raw(&[1]);
    }

    pub(crate) fn sequence(&mut self, name: &str, len: usize) {
        self.field(name);
        self.raw(&(len as u128).to_be_bytes());
    }

    fn token(&mut self, value: &[u8]) {
        self.raw(&(value.len() as u128).to_be_bytes());
        self.raw(value);
    }

    fn raw(&mut self, value: &[u8]) {
        self.hasher.update(value);
    }
}

pub(crate) fn canonical_f64_bits(value: f64) -> u64 {
    if value.is_nan() {
        0x7ff8_0000_0000_0000
    } else if value == 0.0 {
        0
    } else {
        value.to_bits()
    }
}
