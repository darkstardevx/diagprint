use crate::{Diagnostic, SourceCache, SourceSnapshot};

/// A diagnostic paired with the immutable source snapshot it was produced
/// against.
///
/// `CapturedDiagnostic` is useful for editor, compiler, parser, and language
/// server workflows where live source text may continue changing after a
/// diagnostic has been created.
///
/// Constructing a captured diagnostic binds any unversioned labels whose source
/// exists in the supplied snapshot to that source's captured revision.
///
/// Source text remains outside [`struct@Diagnostic`] itself and is therefore not
/// serialized into diagnostic JSON.
#[derive(Debug, Clone)]
pub struct CapturedDiagnostic {
    diagnostic: Diagnostic,
    sources: SourceSnapshot,
}

impl CapturedDiagnostic {
    /// Creates a captured diagnostic from a diagnostic and source snapshot.
    ///
    /// Existing explicit source revisions are preserved. Unversioned labels
    /// are bound to revisions available in `sources`.
    pub fn new(diagnostic: Diagnostic, sources: SourceSnapshot) -> Self {
        let diagnostic = diagnostic.bind_source_revisions(&sources);

        Self {
            diagnostic,
            sources,
        }
    }

    /// Returns the captured diagnostic.
    pub fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    /// Returns the immutable source snapshot associated with this diagnostic.
    pub fn sources(&self) -> &SourceSnapshot {
        &self.sources
    }

    /// Returns whether any revision-bound source used by this diagnostic no
    /// longer matches the supplied live cache.
    pub fn is_stale(&self, live: &SourceCache) -> bool {
        self.diagnostic.has_stale_sources(live)
    }

    /// Returns whether all revision-bound sources still match the supplied
    /// live cache.
    pub fn is_current(&self, live: &SourceCache) -> bool {
        !self.is_stale(live)
    }

    /// Consumes the captured diagnostic and returns its parts.
    pub fn into_parts(self) -> (Diagnostic, SourceSnapshot) {
        (self.diagnostic, self.sources)
    }

    /// Consumes the wrapper and returns only the diagnostic.
    pub fn into_diagnostic(self) -> Diagnostic {
        self.diagnostic
    }
}
