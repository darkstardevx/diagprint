use crate::{
    DeltaArtifact, DeltaPolicy, DiagnosticDelta, ExportAttributes, ExportPath, ExportPolicy,
    ExportRemediation, ExportText, ExportUrl, ReportDigest,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use sha2::{Digest as _, Sha256};
use std::{error::Error, fmt};

/// Stable schema identifier for export receipts.
pub const RECEIPT_V1_SCHEMA: &str = "diagprint.receipt/v1";

/// Media type used by `diagprint.delta/v1` JSON artifacts.
pub const DELTA_V1_MEDIA_TYPE: &str = "application/vnd.diagprint.delta+json;version=1";

/// SHA-256 identity of exact exported artifact bytes.
///
/// This is intentionally distinct from [`ReportDigest`].
///
/// `ReportDigest` identifies semantic diagnostic report content.
/// `ArtifactDigest` identifies one exact external byte representation.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArtifactDigest {
    bytes: [u8; 32],
}

impl ArtifactDigest {
    /// Computes the SHA-256 digest of exact artifact bytes.
    pub fn compute(bytes: &[u8]) -> Self {
        let digest = Sha256::digest(bytes);
        let mut output = [0_u8; 32];
        output.copy_from_slice(&digest);

        Self { bytes: output }
    }

    pub const fn algorithm(&self) -> &'static str {
        "sha256"
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.bytes
    }

    pub fn to_hex(self) -> String {
        hex(&self.bytes)
    }
}

impl fmt::Debug for ArtifactDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ArtifactDigest")
            .field(&self.to_string())
            .finish()
    }
}

impl fmt::Display for ArtifactDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.algorithm())?;
        formatter.write_str(":")?;
        formatter.write_str(&hex(&self.bytes))
    }
}

impl<'de> Deserialize<'de> for ArtifactDigest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;

        let encoded = value
            .strip_prefix("sha256:")
            .ok_or_else(|| D::Error::custom("artifact digest must start with `sha256:`"))?;

        if encoded.len() != 64 {
            return Err(D::Error::custom(
                "artifact SHA-256 digest must contain exactly 64 hexadecimal characters",
            ));
        }

        let encoded = encoded.as_bytes();
        let mut bytes = [0_u8; 32];

        for index in 0..32 {
            let high = decode_hex_nibble(encoded[index * 2]).ok_or_else(|| {
                D::Error::custom("artifact digest contains invalid hexadecimal data")
            })?;

            let low = decode_hex_nibble(encoded[index * 2 + 1]).ok_or_else(|| {
                D::Error::custom("artifact digest contains invalid hexadecimal data")
            })?;

            bytes[index] = (high << 4) | low;
        }

        Ok(Self { bytes })
    }
}

const fn decode_hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

impl Serialize for ArtifactDigest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// External encoding used to produce an exact artifact byte stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactEncoding {
    CompactJson,
    PrettyJson,
}

/// Privacy-safe description of the export boundary.
///
/// Repository-relative mode deliberately records only the mode, not the
/// repository root path itself. The resulting artifact digest still identifies
/// the exact bytes produced by that boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ExportPolicyDescriptor {
    pub text: &'static str,
    pub paths: &'static str,
    pub attributes: &'static str,
    pub remediation: &'static str,
    pub urls: &'static str,

    pub include_application: bool,
    pub include_hostname: bool,
    pub include_process_id: bool,
}

impl From<&ExportPolicy> for ExportPolicyDescriptor {
    fn from(policy: &ExportPolicy) -> Self {
        Self {
            text: match policy.text() {
                ExportText::Preserve => "preserve",
                ExportText::Redact => "redact",
            },

            paths: match policy.paths() {
                ExportPath::Omit => "omit",
                ExportPath::FileName => "file_name",
                ExportPath::FullPath => "full_path",
                ExportPath::RepositoryRelative(_) => "repository_relative",
            },

            attributes: match policy.attributes() {
                ExportAttributes::Omit => "omit",
                ExportAttributes::Redact => "redact",
                ExportAttributes::RedactSensitive => "redact_sensitive",
                ExportAttributes::Full => "full",
            },

            remediation: match policy.remediation() {
                ExportRemediation::Omit => "omit",
                ExportRemediation::MetadataOnly => "metadata_only",
            },

            urls: match policy.urls() {
                ExportUrl::Omit => "omit",
                ExportUrl::Sanitize => "sanitize",
                ExportUrl::Full => "full",
            },

            include_application: policy.includes_application(),
            include_hostname: policy.includes_hostname(),
            include_process_id: policy.includes_process_id(),
        }
    }
}

/// Lightweight CI result retained in an export receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ReceiptEvaluation {
    pub status: &'static str,
    pub exit_code: u8,
}

/// Receipt for one exact exported artifact.
///
/// The receipt records:
///
/// - which external schema was emitted;
/// - which report states produced it;
/// - which privacy boundary was used;
/// - which encoding produced the bytes;
/// - the exact byte length;
/// - the SHA-256 digest of those exact bytes;
/// - optional CI evaluation status.
///
/// Receipts do not contain source text or remediation payloads.
#[derive(Debug, Clone, Serialize)]
pub struct ExportReceipt {
    pub schema: &'static str,

    pub artifact_schema: &'static str,
    pub media_type: &'static str,
    pub encoding: ArtifactEncoding,

    pub artifact_digest: ArtifactDigest,
    pub byte_length: usize,

    pub baseline_report: ReportDigest,
    pub candidate_report: ReportDigest,

    pub export_policy: ExportPolicyDescriptor,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub evaluation: Option<ReceiptEvaluation>,
}

impl ExportReceipt {
    /// Verifies arbitrary bytes against this receipt.
    pub fn verify_bytes(&self, bytes: &[u8]) -> Result<(), ArtifactVerificationError> {
        if bytes.len() != self.byte_length {
            return Err(ArtifactVerificationError::Length {
                expected: self.byte_length,
                actual: bytes.len(),
            });
        }

        let actual = ArtifactDigest::compute(bytes);

        if actual != self.artifact_digest {
            return Err(ArtifactVerificationError::Digest {
                expected: self.artifact_digest,
                actual,
            });
        }

        Ok(())
    }
}

/// Exact artifact payload plus its export receipt.
#[derive(Debug, Clone)]
pub struct ExportedArtifact {
    bytes: Vec<u8>,
    receipt: ExportReceipt,
}

impl ExportedArtifact {
    fn new(bytes: Vec<u8>, receipt: ExportReceipt) -> Self {
        Self { bytes, receipt }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn receipt(&self) -> &ExportReceipt {
        &self.receipt
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Verifies that the retained payload still matches its receipt.
    pub fn verify(&self) -> Result<(), ArtifactVerificationError> {
        self.receipt.verify_bytes(&self.bytes)
    }

    /// Returns the JSON payload as UTF-8 text.
    ///
    /// Delta JSON exporters always emit valid UTF-8, so this conversion is
    /// exposed fallibly rather than relying on a panic-prone internal
    /// assumption.
    pub fn as_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.bytes)
    }
}

/// Failure while checking artifact bytes against an export receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactVerificationError {
    Length {
        expected: usize,
        actual: usize,
    },

    Digest {
        expected: ArtifactDigest,
        actual: ArtifactDigest,
    },
}

impl fmt::Display for ArtifactVerificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { expected, actual } => write!(
                formatter,
                "artifact byte length mismatch: expected {expected}, got {actual}"
            ),

            Self::Digest { expected, actual } => write!(
                formatter,
                "artifact digest mismatch: expected {expected}, got {actual}"
            ),
        }
    }
}

impl Error for ArtifactVerificationError {}

impl DiagnosticDelta {
    /// Exports `diagprint.delta/v1` as compact JSON with an integrity receipt.
    pub fn export_json(
        &self,
        export_policy: &ExportPolicy,
    ) -> serde_json::Result<ExportedArtifact> {
        export_delta(self, None, export_policy, ArtifactEncoding::CompactJson)
    }

    /// Exports an evaluated `diagprint.delta/v1` artifact as compact JSON.
    pub fn export_evaluated_json(
        &self,
        policy: &DeltaPolicy,
        export_policy: &ExportPolicy,
    ) -> serde_json::Result<ExportedArtifact> {
        export_delta(
            self,
            Some(policy),
            export_policy,
            ArtifactEncoding::CompactJson,
        )
    }

    /// Exports `diagprint.delta/v1` as pretty JSON with an integrity receipt.
    pub fn export_json_pretty(
        &self,
        export_policy: &ExportPolicy,
    ) -> serde_json::Result<ExportedArtifact> {
        export_delta(self, None, export_policy, ArtifactEncoding::PrettyJson)
    }

    /// Exports an evaluated `diagprint.delta/v1` artifact as pretty JSON.
    pub fn export_evaluated_json_pretty(
        &self,
        policy: &DeltaPolicy,
        export_policy: &ExportPolicy,
    ) -> serde_json::Result<ExportedArtifact> {
        export_delta(
            self,
            Some(policy),
            export_policy,
            ArtifactEncoding::PrettyJson,
        )
    }
}

fn export_delta(
    delta: &DiagnosticDelta,
    policy: Option<&DeltaPolicy>,
    export_policy: &ExportPolicy,
    encoding: ArtifactEncoding,
) -> serde_json::Result<ExportedArtifact> {
    let artifact = match policy {
        Some(policy) => DeltaArtifact::evaluated(delta, policy, export_policy),

        None => DeltaArtifact::new(delta, export_policy),
    };

    let bytes = match encoding {
        ArtifactEncoding::CompactJson => serde_json::to_vec(&artifact)?,

        ArtifactEncoding::PrettyJson => serde_json::to_vec_pretty(&artifact)?,
    };

    let evaluation = artifact
        .evaluation
        .as_ref()
        .map(|evaluation| ReceiptEvaluation {
            status: evaluation.status,
            exit_code: evaluation.exit_code,
        });

    let receipt = ExportReceipt {
        schema: RECEIPT_V1_SCHEMA,

        artifact_schema: artifact.schema,
        media_type: DELTA_V1_MEDIA_TYPE,
        encoding,

        artifact_digest: ArtifactDigest::compute(&bytes),
        byte_length: bytes.len(),

        baseline_report: artifact.baseline_report,
        candidate_report: artifact.candidate_report,

        export_policy: ExportPolicyDescriptor::from(export_policy),

        evaluation,
    };

    Ok(ExportedArtifact::new(bytes, receipt))
}

fn hex(bytes: &[u8; 32]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";

    let mut output = String::with_capacity(64);

    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));

        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }

    output
}
