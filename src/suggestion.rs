use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Applicability {
    MachineApplicable,
    MaybeIncorrect,
    HasPlaceholders,

    #[default]
    Manual,
}

impl Applicability {
    pub fn can_apply_automatically(self) -> bool {
        matches!(self, Self::MachineApplicable)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::MachineApplicable => "machine-applicable",
            Self::MaybeIncorrect => "maybe-incorrect",
            Self::HasPlaceholders => "has-placeholders",
            Self::Manual => "manual",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextRange {
    pub start: usize,
    pub end: usize,
}

impl TextRange {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(self) -> bool {
        self.start == self.end
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Edit {
    Replace {
        file: PathBuf,
        range: TextRange,
        expected: String,
        replacement: String,
    },

    Insert {
        file: PathBuf,
        offset: usize,
        expected_before: Option<String>,
        text: String,
    },

    Delete {
        file: PathBuf,
        range: TextRange,
        expected: String,
    },
}

impl Edit {
    pub fn replace(
        file: impl Into<PathBuf>,
        range: TextRange,
        expected: impl Into<String>,
        replacement: impl Into<String>,
    ) -> Self {
        Self::Replace {
            file: file.into(),
            range,
            expected: expected.into(),
            replacement: replacement.into(),
        }
    }

    pub fn insert(file: impl Into<PathBuf>, offset: usize, text: impl Into<String>) -> Self {
        Self::Insert {
            file: file.into(),
            offset,
            expected_before: None,
            text: text.into(),
        }
    }

    pub fn insert_after(
        file: impl Into<PathBuf>,
        offset: usize,
        expected_before: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self::Insert {
            file: file.into(),
            offset,
            expected_before: Some(expected_before.into()),
            text: text.into(),
        }
    }

    pub fn delete(file: impl Into<PathBuf>, range: TextRange, expected: impl Into<String>) -> Self {
        Self::Delete {
            file: file.into(),
            range,
            expected: expected.into(),
        }
    }

    pub fn file(&self) -> &Path {
        match self {
            Self::Replace { file, .. } | Self::Insert { file, .. } | Self::Delete { file, .. } => {
                file
            }
        }
    }

    pub fn start(&self) -> usize {
        match self {
            Self::Replace { range, .. } | Self::Delete { range, .. } => range.start,
            Self::Insert { offset, .. } => *offset,
        }
    }

    pub fn end(&self) -> usize {
        match self {
            Self::Replace { range, .. } | Self::Delete { range, .. } => range.end,
            Self::Insert { offset, .. } => *offset,
        }
    }

    pub fn preview_lines(&self) -> Vec<String> {
        match self {
            Self::Replace {
                file,
                expected,
                replacement,
                ..
            } => {
                let mut lines = vec![format!("PATCH {}", file.display())];

                for line in expected.lines() {
                    lines.push(format!("- {line}"));
                }

                for line in replacement.lines() {
                    lines.push(format!("+ {line}"));
                }

                lines
            }

            Self::Insert { file, text, .. } => {
                let mut lines = vec![format!("PATCH {}", file.display())];

                for line in text.lines() {
                    lines.push(format!("+ {line}"));
                }

                lines
            }

            Self::Delete { file, expected, .. } => {
                let mut lines = vec![format!("PATCH {}", file.display())];

                for line in expected.lines() {
                    lines.push(format!("- {line}"));
                }

                lines
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentationLink {
    pub label: String,
    pub url: String,
    pub language_hint: Option<String>,
}

impl DocumentationLink {
    pub fn new(label: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            url: url.into(),
            language_hint: None,
        }
    }

    pub fn language(mut self, language: impl Into<String>) -> Self {
        self.language_hint = Some(language.into());
        self
    }

    pub fn docs_rs(crate_name: &str, version: &str, path: &str) -> Self {
        let path = path.trim_start_matches('/');

        Self::new(
            format!("{crate_name} documentation"),
            format!("https://docs.rs/{crate_name}/{version}/{crate_name}/{path}"),
        )
        .language("rust")
    }

    pub fn rust_error(code: &str) -> Self {
        let code = code.trim().to_uppercase();

        Self::new(
            format!("Rust error {code}"),
            format!("https://doc.rust-lang.org/error_codes/{code}.html"),
        )
        .language("rust")
    }

    pub fn cargo_book(path: &str) -> Self {
        let path = path.trim_start_matches('/');

        Self::new(
            "Cargo Book",
            format!("https://doc.rust-lang.org/cargo/{path}"),
        )
        .language("toml")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuggestedCommand {
    pub command: String,
    pub explanation: Option<String>,
}

impl SuggestedCommand {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            explanation: None,
        }
    }

    pub fn explanation(mut self, explanation: impl Into<String>) -> Self {
        self.explanation = Some(explanation.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Suggestion {
    pub title: String,
    pub explanation: Option<String>,
    pub applicability: Applicability,
    pub documentation: Vec<DocumentationLink>,
    pub edits: Vec<Edit>,
    pub commands: Vec<SuggestedCommand>,
}

impl Suggestion {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            explanation: None,
            applicability: Applicability::Manual,
            documentation: Vec::new(),
            edits: Vec::new(),
            commands: Vec::new(),
        }
    }

    pub fn explanation(mut self, explanation: impl Into<String>) -> Self {
        self.explanation = Some(explanation.into());
        self
    }

    pub fn applicability(mut self, applicability: Applicability) -> Self {
        self.applicability = applicability;
        self
    }

    pub fn documentation(mut self, link: DocumentationLink) -> Self {
        self.documentation.push(link);
        self
    }

    pub fn edit(mut self, edit: Edit) -> Self {
        self.edits.push(edit);
        self
    }

    pub fn command(mut self, command: SuggestedCommand) -> Self {
        self.commands.push(command);
        self
    }

    pub fn is_machine_applicable(&self) -> bool {
        self.applicability.can_apply_automatically()
    }

    pub fn has_edits(&self) -> bool {
        !self.edits.is_empty()
    }
}
