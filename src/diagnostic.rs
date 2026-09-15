use crate::{Severity, Suggestion};
use chrono::{DateTime, Local};
use serde::Serialize;
use std::error::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct SourceLocation {
    pub file: String,
    pub line: u32,
    pub column: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Label {
    pub location: SourceLocation,
    pub length: Option<usize>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Cause {
    pub message: String,
    pub source: Option<Box<Cause>>,
}

impl Cause {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            source: None,
        }
    }

    pub fn caused_by(mut self, cause: Cause) -> Self {
        self.source = Some(Box::new(cause));
        self
    }

    pub fn from_error(error: &(dyn Error + 'static)) -> Self {
        let mut root = Cause::new(error.to_string());
        let mut tail = &mut root;
        let mut next = error.source();

        while let Some(error) = next {
            tail.source = Some(Box::new(Cause::new(error.to_string())));
            tail = tail
                .source
                .as_mut()
                .expect("cause was inserted immediately before access");

            next = error.source();
        }

        root
    }

    pub fn iter(&self) -> CauseIter<'_> {
        CauseIter { next: Some(self) }
    }
}

pub struct CauseIter<'a> {
    next: Option<&'a Cause>,
}

impl<'a> Iterator for CauseIter<'a> {
    type Item = &'a Cause;

    fn next(&mut self) -> Option<Self::Item> {
        let cause = self.next?;
        self.next = cause.source.as_deref();
        Some(cause)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub report_id: Uuid,
    pub session_id: Uuid,
    pub timestamp: DateTime<Local>,
    pub application: String,
    pub pid: u32,
    pub hostname: String,

    pub severity: Severity,
    pub code: Option<String>,
    pub message: String,

    pub labels: Vec<Label>,
    pub notes: Vec<String>,
    pub help: Option<String>,
    pub cause: Option<Cause>,

    pub suggestions: Vec<Suggestion>,
}

impl Diagnostic {
    pub(crate) fn new(
        session_id: Uuid,
        application: String,
        severity: Severity,
        message: impl Into<String>,
    ) -> Self {
        Self {
            report_id: Uuid::now_v7(),
            session_id,
            timestamp: Local::now(),
            application,
            pid: std::process::id(),
            hostname: hostname::get()
                .ok()
                .and_then(|hostname| hostname.into_string().ok())
                .unwrap_or_else(|| "unknown".into()),

            severity,
            code: None,
            message: message.into(),

            labels: Vec::new(),
            notes: Vec::new(),
            help: None,
            cause: None,

            suggestions: Vec::new(),
        }
    }

    pub fn code(mut self, value: impl Into<String>) -> Self {
        self.code = Some(value.into());
        self
    }

    pub fn note(mut self, value: impl Into<String>) -> Self {
        self.notes.push(value.into());
        self
    }

    pub fn help(mut self, value: impl Into<String>) -> Self {
        self.help = Some(value.into());
        self
    }

    pub fn cause(mut self, value: impl Into<String>) -> Self {
        self.cause = Some(match self.cause.take() {
            None => Cause::new(value),
            Some(old) => old.caused_by(Cause::new(value)),
        });

        self
    }

    pub fn cause_chain(mut self, cause: Cause) -> Self {
        self.cause = Some(cause);
        self
    }

    pub fn from_error(mut self, error: &(dyn Error + 'static)) -> Self {
        self.cause = Some(Cause::from_error(error));
        self
    }

    pub fn label(
        mut self,
        file: impl Into<String>,
        line: u32,
        column: Option<u32>,
        length: Option<usize>,
        message: Option<impl Into<String>>,
    ) -> Self {
        self.labels.push(Label {
            location: SourceLocation {
                file: file.into(),
                line,
                column,
            },
            length,
            message: message.map(Into::into),
        });

        self
    }

    pub fn source(self, file: impl Into<String>, line: u32, column: Option<u32>) -> Self {
        self.label(file, line, column, None, None::<String>)
    }

    pub fn suggestion(mut self, suggestion: Suggestion) -> Self {
        self.suggestions.push(suggestion);
        self
    }

    pub fn suggestions(mut self, suggestions: impl IntoIterator<Item = Suggestion>) -> Self {
        self.suggestions.extend(suggestions);
        self
    }
}
