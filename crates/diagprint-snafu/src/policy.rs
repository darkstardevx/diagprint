use crate::bridge::DefaultSnafuErrorMapper;

/// Controls presentation of source errors that were not explicitly mapped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnafuTextPolicy {
    /// Preserve the source error's `Display` text.
    #[default]
    Display,
    /// Replace fallback source text with a fixed redaction marker.
    RedactUnmapped,
}

/// Controls export of the concrete root error's SNAFU backtrace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnafuBacktracePolicy {
    /// Do not copy backtrace text into diagprint diagnostics.
    #[default]
    Omit,
    /// Add the root backtrace as presentation-only diagnostic note text.
    DisplayText,
}

/// Advanced capture configuration.
///
/// The mapper remains responsible for stable metadata on explicitly mapped
/// source nodes. Text and backtrace policies affect presentation only.
#[derive(Debug, Clone)]
pub struct SnafuCaptureProfile<M = DefaultSnafuErrorMapper> {
    mapper: M,
    text_policy: SnafuTextPolicy,
    backtrace_policy: SnafuBacktracePolicy,
}

impl Default for SnafuCaptureProfile<DefaultSnafuErrorMapper> {
    fn default() -> Self {
        Self::new(DefaultSnafuErrorMapper)
    }
}

impl<M> SnafuCaptureProfile<M> {
    pub const fn new(mapper: M) -> Self {
        Self {
            mapper,
            text_policy: SnafuTextPolicy::Display,
            backtrace_policy: SnafuBacktracePolicy::Omit,
        }
    }

    pub const fn mapper(&self) -> &M {
        &self.mapper
    }

    pub const fn text_policy_value(&self) -> SnafuTextPolicy {
        self.text_policy
    }

    pub const fn backtrace_policy_value(&self) -> SnafuBacktracePolicy {
        self.backtrace_policy
    }

    #[must_use]
    pub const fn text_policy(mut self, policy: SnafuTextPolicy) -> Self {
        self.text_policy = policy;
        self
    }

    #[must_use]
    pub const fn backtrace_policy(mut self, policy: SnafuBacktracePolicy) -> Self {
        self.backtrace_policy = policy;
        self
    }

    pub fn into_mapper(self) -> M {
        self.mapper
    }
}
