use diagprint::REDACTED;
use std::path::Path;

/// Policy for exporting free-form diagnostic text.
///
/// Plaintext export is always an explicit opt-in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TextExport {
    /// Do not export the field.
    Omit,

    /// Export the standard diagprint redaction marker.
    #[default]
    Redact,

    /// Export the original text.
    Plaintext,
}

impl TextExport {
    pub(crate) fn apply(self, value: &str) -> Option<String> {
        match self {
            Self::Omit => None,

            Self::Redact => Some(REDACTED.to_owned()),

            Self::Plaintext => Some(value.to_owned()),
        }
    }
}

/// Policy for exporting diagnostic source paths.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LocationExport {
    /// Do not export source locations.
    #[default]
    Omit,

    /// Export only the final filename component.
    FileName,

    /// Export the complete source path/name.
    FullPath,
}

impl LocationExport {
    pub(crate) fn apply(self, value: &str) -> Option<String> {
        match self {
            Self::Omit => None,

            Self::FileName => Path::new(value)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned()),

            Self::FullPath => Some(value.to_owned()),
        }
    }
}

/// Privacy and span-behavior policy for diagnostic telemetry.
///
/// Defaults are intentionally conservative:
///
/// - diagnostic messages are redacted;
/// - help, notes, causes, and label messages are omitted;
/// - source locations are omitted;
/// - hostname and PID are omitted;
/// - application, IDs, severity, code, timestamps, and structural counts are
///   exported;
/// - Error and Fatal diagnostics mark their span as failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TelemetryPolicy {
    pub message: TextExport,
    pub help: TextExport,
    pub notes: TextExport,
    pub causes: TextExport,
    pub label_messages: TextExport,
    pub locations: LocationExport,

    pub include_application: bool,
    pub include_hostname: bool,
    pub include_process_id: bool,

    pub mark_error_status: bool,
}

impl Default for TelemetryPolicy {
    fn default() -> Self {
        Self {
            message: TextExport::Redact,

            help: TextExport::Omit,

            notes: TextExport::Omit,

            causes: TextExport::Omit,

            label_messages: TextExport::Omit,

            locations: LocationExport::Omit,

            include_application: true,
            include_hostname: false,
            include_process_id: false,

            mark_error_status: true,
        }
    }
}

impl TelemetryPolicy {
    pub const fn new() -> Self {
        Self {
            message: TextExport::Redact,

            help: TextExport::Omit,

            notes: TextExport::Omit,

            causes: TextExport::Omit,

            label_messages: TextExport::Omit,

            locations: LocationExport::Omit,

            include_application: true,
            include_hostname: false,
            include_process_id: false,

            mark_error_status: true,
        }
    }

    pub const fn with_message(mut self, policy: TextExport) -> Self {
        self.message = policy;
        self
    }

    pub const fn with_help(mut self, policy: TextExport) -> Self {
        self.help = policy;
        self
    }

    pub const fn with_notes(mut self, policy: TextExport) -> Self {
        self.notes = policy;
        self
    }

    pub const fn with_causes(mut self, policy: TextExport) -> Self {
        self.causes = policy;
        self
    }

    pub const fn with_label_messages(mut self, policy: TextExport) -> Self {
        self.label_messages = policy;
        self
    }

    pub const fn with_locations(mut self, policy: LocationExport) -> Self {
        self.locations = policy;
        self
    }

    pub const fn with_application(mut self, enabled: bool) -> Self {
        self.include_application = enabled;
        self
    }

    pub const fn with_hostname(mut self, enabled: bool) -> Self {
        self.include_hostname = enabled;
        self
    }

    pub const fn with_process_id(mut self, enabled: bool) -> Self {
        self.include_process_id = enabled;
        self
    }

    pub const fn with_error_status(mut self, enabled: bool) -> Self {
        self.mark_error_status = enabled;
        self
    }
}
