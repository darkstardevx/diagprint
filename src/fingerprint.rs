use crate::{
    Diagnostic, DiagnosticAttribute, DiagnosticReport, DiagnosticValue, Edit, LabelKind, Severity,
    SuggestedCommand, Suggestion,
    canonical::{
        CanonicalHasher, CanonicalizationError, CanonicalizationVersion, canonical_f64_bits,
        canonical_path,
    },
};
use serde::{Serialize, Serializer, ser::SerializeStruct};
use std::fmt;

const DIAGNOSTIC_FINGERPRINT_DOMAIN: &str = "diagprint.diagnostic-fingerprint/v1";
const DIAGNOSTIC_DIGEST_DOMAIN: &str = "diagprint.diagnostic-digest/v1";
const REPORT_DIGEST_DOMAIN: &str = "diagprint.report-digest/v1";

/// Reserved structured attribute understood by canonical fingerprinting.
///
/// Producers which already own a stable identity can attach this attribute to
/// make fingerprints independent of wording and source-location changes.
pub const IDENTITY_ATTRIBUTE: &str = "diagprint.identity";

/// Source identity included in derived diagnostic fingerprints.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum FingerprintSource {
    /// Do not use source identity when deriving a fingerprint.
    Omit,

    /// Use only the final filename component, treating `/` and `\\` as
    /// separators so fingerprints are portable across common path styles.
    #[default]
    FileName,

    /// Use the source name exactly as stored by the diagnostic.
    Full,
}

/// Controls which stable semantic fields participate in a diagnostic
/// fingerprint.
///
/// Severity, line/column, report/session IDs, timestamps, PID, hostname,
/// source revisions, attributes other than [`IDENTITY_ATTRIBUTE`], causes,
/// notes, help, and remediation are deliberately excluded by default so a
/// logical diagnostic can remain identifiable when its presentation changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FingerprintPolicy {
    include_code: bool,
    include_message: bool,
    source: FingerprintSource,
    include_primary_label_message: bool,
    prefer_explicit_identity: bool,
}

impl Default for FingerprintPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl FingerprintPolicy {
    /// Creates the canonical v1 default fingerprint policy.
    pub const fn new() -> Self {
        Self {
            include_code: true,
            include_message: true,
            source: FingerprintSource::FileName,
            include_primary_label_message: true,
            prefer_explicit_identity: true,
        }
    }

    pub const fn with_code(mut self, enabled: bool) -> Self {
        self.include_code = enabled;
        self
    }

    pub const fn with_message(mut self, enabled: bool) -> Self {
        self.include_message = enabled;
        self
    }

    pub const fn with_source(mut self, source: FingerprintSource) -> Self {
        self.source = source;
        self
    }

    pub const fn with_primary_label_message(mut self, enabled: bool) -> Self {
        self.include_primary_label_message = enabled;
        self
    }

    pub const fn with_explicit_identity(mut self, enabled: bool) -> Self {
        self.prefer_explicit_identity = enabled;
        self
    }

    pub const fn includes_code(&self) -> bool {
        self.include_code
    }

    pub const fn includes_message(&self) -> bool {
        self.include_message
    }

    pub const fn source(&self) -> FingerprintSource {
        self.source
    }

    pub const fn includes_primary_label_message(&self) -> bool {
        self.include_primary_label_message
    }

    pub const fn prefers_explicit_identity(&self) -> bool {
        self.prefer_explicit_identity
    }
}

/// Hash algorithm used by canonical v1 identity values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DigestAlgorithm {
    Sha256,
}

impl DigestAlgorithm {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sha256 => "sha256",
        }
    }
}

impl fmt::Display for DigestAlgorithm {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

macro_rules! digest_type {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name {
            version: CanonicalizationVersion,
            bytes: [u8; 32],
        }

        impl $name {
            pub(crate) const fn from_v1(bytes: [u8; 32]) -> Self {
                Self {
                    version: CanonicalizationVersion::V1,
                    bytes,
                }
            }

            pub const fn version(&self) -> CanonicalizationVersion {
                self.version
            }

            pub const fn algorithm(&self) -> DigestAlgorithm {
                DigestAlgorithm::Sha256
            }

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.bytes
            }

            pub fn to_hex(self) -> String {
                hex(&self.bytes)
            }

            /// Fully qualified identity including canonicalization schema and
            /// hash algorithm.
            pub fn qualified(self) -> String {
                format!("{}:{}:{}", self.version, self.algorithm(), self.to_hex())
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_struct(stringify!($name))
                    .field("canonicalization", &self.version.as_str())
                    .field("algorithm", &self.algorithm().as_str())
                    .field("value", &hex(&self.bytes))
                    .finish()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.algorithm().as_str())?;
                formatter.write_str(":")?;
                formatter.write_str(&hex(&self.bytes))
            }
        }

        // Serialization exposes the completed identity value for interchange.
        // Serde does not participate in canonical-v1 byte construction.
        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                let mut state = serializer.serialize_struct(stringify!($name), 3)?;
                state.serialize_field("canonicalization", self.version.as_str())?;
                state.serialize_field("algorithm", self.algorithm().as_str())?;
                state.serialize_field("value", &hex(&self.bytes))?;
                state.end()
            }
        }
    };
}

digest_type!(
    DiagnosticFingerprint,
    "Stable semantic identity for one logical diagnostic under diagprint.canonical/v1."
);
digest_type!(
    DiagnosticDigest,
    "Deterministic digest of one diagnostic's meaningful content under diagprint.canonical/v1."
);
digest_type!(
    ReportDigest,
    "Order-independent deterministic digest of a diagnostic report under diagprint.canonical/v1."
);

impl DiagnosticFingerprint {
    pub fn compute(diagnostic: &Diagnostic) -> Self {
        Self::compute_with_policy(diagnostic, &FingerprintPolicy::default())
    }

    pub fn compute_with_policy(diagnostic: &Diagnostic, policy: &FingerprintPolicy) -> Self {
        let mut canonical = CanonicalHasher::new(DIAGNOSTIC_FINGERPRINT_DOMAIN);

        let explicit = if policy.prefer_explicit_identity {
            explicit_identity_attributes(&diagnostic.attributes)
        } else {
            Vec::new()
        };

        if explicit.is_empty() {
            canonical.string("identity_mode", "derived");

            write_optional_code(&mut canonical, diagnostic, policy.include_code);

            if policy.include_message {
                canonical.string("message", &diagnostic.message);
            } else {
                canonical.string("message_mode", "omitted");
            }

            let mut labels = diagnostic
                .labels
                .iter()
                .filter(|label| label.kind == LabelKind::Primary)
                .map(|label| {
                    let source = match policy.source {
                        FingerprintSource::Omit => None,
                        FingerprintSource::FileName => {
                            Some(portable_file_name(&label.location.file))
                        }
                        FingerprintSource::Full => Some(label.location.file.clone()),
                    };

                    let message = policy
                        .include_primary_label_message
                        .then(|| label.message.clone())
                        .flatten();

                    (source, message)
                })
                .collect::<Vec<_>>();

            labels.sort();

            canonical.sequence("primary_labels", labels.len());

            for (source, message) in labels {
                write_optional_string(&mut canonical, "source", source.as_deref());
                write_optional_string(&mut canonical, "label_message", message.as_deref());
            }
        } else {
            canonical.string("identity_mode", "explicit");
            write_optional_code(&mut canonical, diagnostic, policy.include_code);
            canonical.sequence("identity_attributes", explicit.len());

            for attribute in explicit {
                write_value(&mut canonical, "identity", &attribute.value);
            }
        }

        Self::from_v1(canonical.finish())
    }
}

impl DiagnosticDigest {
    pub fn compute(diagnostic: &Diagnostic) -> Result<Self, CanonicalizationError> {
        let mut canonical = CanonicalHasher::new(DIAGNOSTIC_DIGEST_DOMAIN);

        canonical.string("application", &diagnostic.application);
        canonical.string("severity", severity_token(diagnostic.severity));
        write_optional_string(&mut canonical, "code", diagnostic.code.as_deref());
        canonical.string("message", &diagnostic.message);

        let mut attributes = diagnostic.attributes.iter().collect::<Vec<_>>();
        attributes.sort_by(compare_attributes);

        canonical.sequence("attributes", attributes.len());
        for attribute in attributes {
            canonical.string("name", &attribute.name);
            write_value(&mut canonical, "value", &attribute.value);
        }

        canonical.sequence("labels", diagnostic.labels.len());
        for label in &diagnostic.labels {
            canonical.string("kind", label_kind_token(label.kind));
            canonical.string("file", &label.location.file);
            canonical.u32("line", label.location.line);
            write_optional_u32(&mut canonical, "column", label.location.column);
            write_optional_usize(&mut canonical, "length", label.length);
            write_optional_string(&mut canonical, "label_message", label.message.as_deref());
        }

        canonical.sequence("notes", diagnostic.notes.len());
        for note in &diagnostic.notes {
            canonical.string("note", note);
        }

        write_optional_string(&mut canonical, "help", diagnostic.help.as_deref());

        match &diagnostic.cause {
            Some(cause) => {
                canonical.option_some("cause");
                let causes = cause.iter().collect::<Vec<_>>();
                canonical.sequence("cause_chain", causes.len());
                for item in causes {
                    canonical.string("cause_message", &item.message);
                }
            }
            None => canonical.option_none("cause"),
        }

        canonical.sequence("suggestions", diagnostic.suggestions.len());
        for suggestion in &diagnostic.suggestions {
            write_suggestion(&mut canonical, suggestion)?;
        }

        Ok(Self::from_v1(canonical.finish()))
    }
}

impl ReportDigest {
    pub fn compute(report: &DiagnosticReport) -> Result<Self, CanonicalizationError> {
        let mut diagnostics = report
            .iter()
            .map(DiagnosticDigest::compute)
            .collect::<Result<Vec<_>, _>>()?;

        diagnostics.sort();

        let mut canonical = CanonicalHasher::new(REPORT_DIGEST_DOMAIN);
        canonical.sequence("diagnostics", diagnostics.len());

        for digest in diagnostics {
            canonical.bytes("diagnostic_digest", digest.as_bytes());
        }

        Ok(Self::from_v1(canonical.finish()))
    }
}

impl Diagnostic {
    /// Computes the default stable semantic fingerprint for this diagnostic.
    pub fn fingerprint(&self) -> DiagnosticFingerprint {
        DiagnosticFingerprint::compute(self)
    }

    /// Computes a stable semantic fingerprint using an explicit policy.
    pub fn fingerprint_with_policy(&self, policy: &FingerprintPolicy) -> DiagnosticFingerprint {
        DiagnosticFingerprint::compute_with_policy(self, policy)
    }

    /// Computes the canonical v1 content digest for this diagnostic.
    pub fn digest(&self) -> Result<DiagnosticDigest, CanonicalizationError> {
        DiagnosticDigest::compute(self)
    }
}

impl DiagnosticReport {
    /// Computes an order-independent canonical v1 digest for this report.
    pub fn digest(&self) -> Result<ReportDigest, CanonicalizationError> {
        ReportDigest::compute(self)
    }
}

fn write_optional_code(canonical: &mut CanonicalHasher, diagnostic: &Diagnostic, enabled: bool) {
    if enabled {
        write_optional_string(canonical, "code", diagnostic.code.as_deref());
    } else {
        canonical.string("code_mode", "omitted");
    }
}

fn write_optional_string(canonical: &mut CanonicalHasher, name: &str, value: Option<&str>) {
    match value {
        Some(value) => {
            canonical.option_some(name);
            canonical.string("value", value);
        }
        None => canonical.option_none(name),
    }
}

fn write_optional_u32(canonical: &mut CanonicalHasher, name: &str, value: Option<u32>) {
    match value {
        Some(value) => {
            canonical.option_some(name);
            canonical.u32("value", value);
        }
        None => canonical.option_none(name),
    }
}

fn write_optional_usize(canonical: &mut CanonicalHasher, name: &str, value: Option<usize>) {
    match value {
        Some(value) => {
            canonical.option_some(name);
            canonical.u128("value", value as u128);
        }
        None => canonical.option_none(name),
    }
}

fn write_value(canonical: &mut CanonicalHasher, name: &str, value: &DiagnosticValue) {
    canonical.field(name);

    match value {
        DiagnosticValue::String(value) => {
            canonical.string("type", "string");
            canonical.string("data", value);
        }
        DiagnosticValue::Bool(value) => {
            canonical.string("type", "bool");
            canonical.bool("data", *value);
        }
        DiagnosticValue::I64(value) => {
            canonical.string("type", "i64");
            canonical.i64("data", *value);
        }
        DiagnosticValue::U64(value) => {
            canonical.string("type", "u64");
            canonical.u64("data", *value);
        }
        DiagnosticValue::I128(value) => {
            canonical.string("type", "i128");
            canonical.i128("data", *value);
        }
        DiagnosticValue::U128(value) => {
            canonical.string("type", "u128");
            canonical.u128("data", *value);
        }
        DiagnosticValue::F64(value) => {
            canonical.string("type", "f64");
            canonical.f64("data", *value);
        }
    }
}

fn write_suggestion(
    canonical: &mut CanonicalHasher,
    suggestion: &Suggestion,
) -> Result<(), CanonicalizationError> {
    canonical.string("title", &suggestion.title);
    write_optional_string(canonical, "explanation", suggestion.explanation.as_deref());
    canonical.string("applicability", suggestion.applicability.as_str());

    canonical.sequence("documentation", suggestion.documentation.len());
    for link in &suggestion.documentation {
        canonical.string("label", &link.label);
        canonical.string("url", &link.url);
        write_optional_string(canonical, "language_hint", link.language_hint.as_deref());
    }

    canonical.sequence("edits", suggestion.edits.len());
    for edit in &suggestion.edits {
        write_edit(canonical, edit)?;
    }

    canonical.sequence("commands", suggestion.commands.len());
    for command in &suggestion.commands {
        write_command(canonical, command);
    }

    Ok(())
}

fn write_edit(canonical: &mut CanonicalHasher, edit: &Edit) -> Result<(), CanonicalizationError> {
    match edit {
        Edit::Replace {
            file,
            range,
            expected,
            replacement,
        } => {
            canonical.string("edit_kind", "replace");
            canonical.string("file", canonical_path(file)?);
            canonical.u128("start", range.start as u128);
            canonical.u128("end", range.end as u128);
            canonical.string("expected", expected);
            canonical.string("replacement", replacement);
        }
        Edit::Insert {
            file,
            offset,
            expected_before,
            text,
        } => {
            canonical.string("edit_kind", "insert");
            canonical.string("file", canonical_path(file)?);
            canonical.u128("offset", *offset as u128);
            write_optional_string(canonical, "expected_before", expected_before.as_deref());
            canonical.string("text", text);
        }
        Edit::Delete {
            file,
            range,
            expected,
        } => {
            canonical.string("edit_kind", "delete");
            canonical.string("file", canonical_path(file)?);
            canonical.u128("start", range.start as u128);
            canonical.u128("end", range.end as u128);
            canonical.string("expected", expected);
        }
    }

    Ok(())
}

fn write_command(canonical: &mut CanonicalHasher, command: &SuggestedCommand) {
    canonical.string("command", &command.command);
    write_optional_string(canonical, "explanation", command.explanation.as_deref());
}

fn explicit_identity_attributes(attributes: &[DiagnosticAttribute]) -> Vec<&DiagnosticAttribute> {
    let mut identities = attributes
        .iter()
        .filter(|attribute| attribute.name == IDENTITY_ATTRIBUTE)
        .collect::<Vec<_>>();

    identities.sort_by(compare_attributes);
    identities
}

fn compare_attributes(
    left: &&DiagnosticAttribute,
    right: &&DiagnosticAttribute,
) -> std::cmp::Ordering {
    left.name
        .cmp(&right.name)
        .then_with(|| value_rank(&left.value).cmp(&value_rank(&right.value)))
        .then_with(|| value_sort_bytes(&left.value).cmp(&value_sort_bytes(&right.value)))
}

fn value_rank(value: &DiagnosticValue) -> u8 {
    match value {
        DiagnosticValue::String(_) => 0,
        DiagnosticValue::Bool(_) => 1,
        DiagnosticValue::I64(_) => 2,
        DiagnosticValue::U64(_) => 3,
        DiagnosticValue::I128(_) => 4,
        DiagnosticValue::U128(_) => 5,
        DiagnosticValue::F64(_) => 6,
    }
}

// Produces an explicit deterministic ordering key for otherwise unordered
// diagnostic attributes. These bytes select ordering only; canonical-v1
// field encoding is performed separately by `write_value`.
fn value_sort_bytes(value: &DiagnosticValue) -> Vec<u8> {
    match value {
        DiagnosticValue::String(value) => value.as_bytes().to_vec(),
        DiagnosticValue::Bool(value) => vec![u8::from(*value)],
        DiagnosticValue::I64(value) => value.to_be_bytes().to_vec(),
        DiagnosticValue::U64(value) => value.to_be_bytes().to_vec(),
        DiagnosticValue::I128(value) => value.to_be_bytes().to_vec(),
        DiagnosticValue::U128(value) => value.to_be_bytes().to_vec(),
        DiagnosticValue::F64(value) => canonical_f64_bits(*value).to_be_bytes().to_vec(),
    }
}

fn severity_token(severity: Severity) -> &'static str {
    match severity {
        Severity::Trace => "trace",
        Severity::Debug => "debug",
        Severity::Info => "info",
        Severity::Warning => "warning",
        Severity::Error => "error",
        Severity::Fatal => "fatal",
    }
}

fn label_kind_token(kind: LabelKind) -> &'static str {
    match kind {
        LabelKind::Primary => "primary",
        LabelKind::Secondary => "secondary",
    }
}

fn portable_file_name(value: &str) -> String {
    let value = value.trim_end_matches(&['/', '\\'][..]);

    let separator = value
        .char_indices()
        .rev()
        .find(|(_, character)| *character == '/' || *character == '\\')
        .map(|(index, _)| index);

    match separator {
        Some(index) => value[index + 1..].to_owned(),
        None => value.to_owned(),
    }
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
