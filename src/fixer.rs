use crate::{Applicability, Diagnostic, Edit, Suggestion, TextRange};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt, fs,
    io::{self, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Debug)]
pub enum FixError {
    Io(io::Error),

    InvalidRange {
        file: PathBuf,
        range: TextRange,
    },

    InvalidUtf8Boundary {
        file: PathBuf,
        offset: usize,
    },

    StaleEdit {
        file: PathBuf,
        expected: String,
        actual: String,
    },

    StaleInsert {
        file: PathBuf,
        expected_before: String,
    },

    OverlappingEdits {
        file: PathBuf,
    },
}

impl fmt::Display for FixError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),

            Self::InvalidRange { file, range } => write!(
                f,
                "invalid edit range {}..{} for {}",
                range.start,
                range.end,
                file.display()
            ),

            Self::InvalidUtf8Boundary { file, offset } => write!(
                f,
                "edit offset {offset} is not a UTF-8 boundary in {}",
                file.display()
            ),

            Self::StaleEdit {
                file,
                expected,
                actual,
            } => write!(
                f,
                "refusing stale edit in {}: expected {:?}, found {:?}",
                file.display(),
                expected,
                actual
            ),

            Self::StaleInsert {
                file,
                expected_before,
            } => write!(
                f,
                "refusing stale insert in {}: text before insertion no longer ends with {:?}",
                file.display(),
                expected_before
            ),

            Self::OverlappingEdits { file } => {
                write!(f, "refusing overlapping edits in {}", file.display())
            }
        }
    }
}

impl Error for FixError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for FixError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Debug, Clone)]
pub struct FixPreview {
    pub title: String,
    pub applicability: Applicability,
    pub lines: Vec<String>,
}

impl FixPreview {
    pub fn render(&self) -> String {
        let mut output = format!(
            "SUGGESTION  {}\nAPPLICABILITY  {}\n",
            self.title,
            self.applicability.as_str()
        );

        for line in &self.lines {
            output.push_str(line);
            output.push('\n');
        }

        output
    }
}

#[derive(Debug, Clone, Default)]
pub struct FixCheck {
    pub applicable_suggestions: usize,
    pub affected_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct FixReport {
    pub applied_suggestions: usize,
    pub changed_files: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct Fixer {
    backup: bool,
    backup_suffix: String,
}

#[derive(Debug)]
struct PreparedFile {
    path: PathBuf,
    original: String,
    updated: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InteractiveAction {
    Apply,
    Skip,
    Docs,
    Quit,
}

impl Default for Fixer {
    fn default() -> Self {
        Self {
            backup: false,
            backup_suffix: ".diagprint.bak".into(),
        }
    }
}

impl Fixer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn backups(mut self, enabled: bool) -> Self {
        self.backup = enabled;
        self
    }

    pub fn backup_suffix(mut self, suffix: impl Into<String>) -> Self {
        self.backup_suffix = suffix.into();
        self
    }

    pub fn preview(&self, diagnostic: &Diagnostic) -> Vec<FixPreview> {
        diagnostic
            .suggestions
            .iter()
            .map(Self::preview_suggestion)
            .collect()
    }

    pub fn preview_suggestion(suggestion: &Suggestion) -> FixPreview {
        let mut lines = Vec::new();

        if let Some(explanation) = &suggestion.explanation {
            lines.push(format!("WHY    {explanation}"));
        }

        for edit in &suggestion.edits {
            lines.extend(edit.preview_lines());
        }

        for documentation in &suggestion.documentation {
            lines.push(format!("DOCS   {}", documentation.label));
            lines.push(format!("       {}", documentation.url));
        }

        for command in &suggestion.commands {
            lines.push(format!("COMMAND  {}", command.command));

            if let Some(explanation) = &command.explanation {
                lines.push(format!("         {explanation}"));
            }
        }

        lines.push(format!(
            "FIX    {}",
            if suggestion.is_machine_applicable() && suggestion.has_edits() {
                "automatic fix available"
            } else {
                "manual review required"
            }
        ));

        if !suggestion.commands.is_empty() {
            lines.push("       commands are suggestions only and are never auto-executed".into());
        }

        FixPreview {
            title: suggestion.title.clone(),
            applicability: suggestion.applicability,
            lines,
        }
    }

    /// Validates every machine-applicable edit against the current filesystem
    /// without changing any files.
    pub fn check(&self, diagnostic: &Diagnostic) -> Result<FixCheck, FixError> {
        let suggestions = applicable_suggestions(diagnostic);
        let prepared = prepare_suggestions(&suggestions)?;

        let affected_files = prepared
            .iter()
            .filter(|prepared| prepared.original != prepared.updated)
            .map(|prepared| prepared.path.clone())
            .collect();

        Ok(FixCheck {
            applicable_suggestions: suggestions.len(),
            affected_files,
        })
    }

    pub fn apply(&self, diagnostic: &Diagnostic) -> Result<FixReport, FixError> {
        let suggestions = applicable_suggestions(diagnostic);
        self.apply_suggestions(&suggestions)
    }

    pub fn apply_interactive(&self, diagnostic: &Diagnostic) -> Result<FixReport, FixError> {
        let mut report = FixReport::default();

        for suggestion in &diagnostic.suggestions {
            println!();
            print!("{}", Self::preview_suggestion(suggestion).render());

            let mut can_apply = suggestion.is_machine_applicable() && suggestion.has_edits();

            if can_apply {
                match prepare_suggestions(&[suggestion]) {
                    Ok(_) => {
                        println!("VERIFY  current file contents match the proposed edit");
                    }

                    Err(error) => {
                        println!("VERIFY  blocked: {error}");
                        can_apply = false;
                    }
                }
            }

            let has_docs = !suggestion.documentation.is_empty();

            loop {
                let prompt = interactive_prompt(can_apply, has_docs);

                print!("{prompt}");
                io::stdout().flush()?;

                let mut answer = String::new();
                io::stdin().read_line(&mut answer)?;

                let Some(action) = parse_interactive_action(&answer, can_apply, has_docs) else {
                    println!("Unknown action.");
                    continue;
                };

                match action {
                    InteractiveAction::Apply => {
                        let current = self.apply_suggestions(&[suggestion])?;

                        report.applied_suggestions += current.applied_suggestions;

                        for file in current.changed_files {
                            if !report.changed_files.contains(&file) {
                                report.changed_files.push(file);
                            }
                        }

                        println!("applied.");
                        break;
                    }

                    InteractiveAction::Skip => {
                        println!("skipped.");
                        break;
                    }

                    InteractiveAction::Docs => {
                        self.show_documentation(suggestion);
                    }

                    InteractiveAction::Quit => {
                        return Ok(report);
                    }
                }
            }
        }

        Ok(report)
    }

    fn show_documentation(&self, suggestion: &Suggestion) {
        if suggestion.documentation.is_empty() {
            println!("No documentation links.");
            return;
        }

        #[cfg(feature = "terminal-docs")]
        {
            let viewer = crate::TerminalDocViewer::new();

            for link in &suggestion.documentation {
                println!();

                if let Err(error) = viewer.open_and_print(link) {
                    eprintln!(
                        "diagprint: could not open {} in terminal: {error}",
                        link.url
                    );

                    println!("{}: {}", link.label, link.url);
                }
            }
        }

        #[cfg(not(feature = "terminal-docs"))]
        {
            for link in &suggestion.documentation {
                println!("{}: {}", link.label, link.url);
            }

            println!();
            println!("Enable the `terminal-docs` feature to render documentation here.");
        }
    }

    fn apply_suggestions(&self, suggestions: &[&Suggestion]) -> Result<FixReport, FixError> {
        let prepared = prepare_suggestions(suggestions)?;
        let mut changed_files = BTreeSet::new();

        for prepared in prepared {
            if prepared.original == prepared.updated {
                continue;
            }

            if self.backup {
                let backup = backup_path(&prepared.path, &self.backup_suffix);

                atomic_write(&backup, &prepared.original)?;
            }

            atomic_write(&prepared.path, &prepared.updated)?;
            changed_files.insert(prepared.path);
        }

        Ok(FixReport {
            applied_suggestions: suggestions.len(),
            changed_files: changed_files.into_iter().collect(),
        })
    }
}

fn applicable_suggestions(diagnostic: &Diagnostic) -> Vec<&Suggestion> {
    diagnostic
        .suggestions
        .iter()
        .filter(|suggestion| suggestion.is_machine_applicable() && suggestion.has_edits())
        .collect()
}

fn prepare_suggestions(suggestions: &[&Suggestion]) -> Result<Vec<PreparedFile>, FixError> {
    let mut by_file: BTreeMap<PathBuf, Vec<&Edit>> = BTreeMap::new();

    for suggestion in suggestions {
        for edit in &suggestion.edits {
            by_file
                .entry(edit.file().to_path_buf())
                .or_default()
                .push(edit);
        }
    }

    let mut prepared = Vec::new();

    for (file, edits) in by_file {
        let original = fs::read_to_string(&file)?;

        validate_edits(&file, &original, &edits)?;

        let updated = apply_edits(&file, original.clone(), &edits)?;

        prepared.push(PreparedFile {
            path: file,
            original,
            updated,
        });
    }

    Ok(prepared)
}

fn interactive_prompt(can_apply: bool, has_docs: bool) -> &'static str {
    match (can_apply, has_docs) {
        (true, true) => "[A]pply  [S]kip  [D]ocs  [Q]uit > ",
        (true, false) => "[A]pply  [S]kip  [Q]uit > ",
        (false, true) => "[S]kip  [D]ocs  [Q]uit > ",
        (false, false) => "[S]kip  [Q]uit > ",
    }
}

fn parse_interactive_action(
    input: &str,
    can_apply: bool,
    has_docs: bool,
) -> Option<InteractiveAction> {
    match input.trim().to_ascii_lowercase().as_str() {
        "a" | "apply" if can_apply => Some(InteractiveAction::Apply),
        "s" | "skip" => Some(InteractiveAction::Skip),
        "d" | "docs" if has_docs => Some(InteractiveAction::Docs),
        "q" | "quit" => Some(InteractiveAction::Quit),
        _ => None,
    }
}

fn validate_edits(file: &Path, content: &str, edits: &[&Edit]) -> Result<(), FixError> {
    let mut spans = Vec::new();

    for edit in edits {
        match edit {
            Edit::Replace {
                range, expected, ..
            }
            | Edit::Delete {
                range, expected, ..
            } => {
                validate_range(file, content, *range)?;

                let actual = &content[range.start..range.end];

                if actual != expected {
                    return Err(FixError::StaleEdit {
                        file: file.to_path_buf(),
                        expected: expected.clone(),
                        actual: actual.to_owned(),
                    });
                }

                spans.push((range.start, range.end));
            }

            Edit::Insert {
                offset,
                expected_before,
                ..
            } => {
                if *offset > content.len() {
                    return Err(FixError::InvalidRange {
                        file: file.to_path_buf(),
                        range: TextRange::new(*offset, *offset),
                    });
                }

                if !content.is_char_boundary(*offset) {
                    return Err(FixError::InvalidUtf8Boundary {
                        file: file.to_path_buf(),
                        offset: *offset,
                    });
                }

                if let Some(expected_before) = expected_before {
                    if !content[..*offset].ends_with(expected_before) {
                        return Err(FixError::StaleInsert {
                            file: file.to_path_buf(),
                            expected_before: expected_before.clone(),
                        });
                    }
                }

                spans.push((*offset, *offset));
            }
        }
    }

    spans.sort_unstable();

    for pair in spans.windows(2) {
        let (first_start, first_end) = pair[0];
        let (second_start, second_end) = pair[1];

        let overlaps = first_end > second_start;

        let duplicate_insert =
            first_start == first_end && second_start == second_end && first_start == second_start;

        if overlaps || duplicate_insert {
            return Err(FixError::OverlappingEdits {
                file: file.to_path_buf(),
            });
        }
    }

    Ok(())
}

fn validate_range(file: &Path, content: &str, range: TextRange) -> Result<(), FixError> {
    if range.start > range.end || range.end > content.len() {
        return Err(FixError::InvalidRange {
            file: file.to_path_buf(),
            range,
        });
    }

    for offset in [range.start, range.end] {
        if !content.is_char_boundary(offset) {
            return Err(FixError::InvalidUtf8Boundary {
                file: file.to_path_buf(),
                offset,
            });
        }
    }

    Ok(())
}

fn apply_edits(file: &Path, mut content: String, edits: &[&Edit]) -> Result<String, FixError> {
    let mut edits = edits.to_vec();

    edits.sort_by_key(|edit| std::cmp::Reverse(edit.start()));

    for edit in edits {
        match edit {
            Edit::Replace {
                range, replacement, ..
            } => {
                validate_range(file, &content, *range)?;
                content.replace_range(range.start..range.end, replacement);
            }

            Edit::Delete { range, .. } => {
                validate_range(file, &content, *range)?;
                content.replace_range(range.start..range.end, "");
            }

            Edit::Insert { offset, text, .. } => {
                if !content.is_char_boundary(*offset) {
                    return Err(FixError::InvalidUtf8Boundary {
                        file: file.to_path_buf(),
                        offset: *offset,
                    });
                }

                content.insert_str(*offset, text);
            }
        }
    }

    Ok(content)
}

fn backup_path(file: &Path, suffix: &str) -> PathBuf {
    PathBuf::from(format!("{}{}", file.display(), suffix))
}

fn atomic_write(file: &Path, contents: &str) -> Result<(), FixError> {
    let parent = file.parent().unwrap_or_else(|| Path::new("."));

    if !parent.as_os_str().is_empty() {
        fs::create_dir_all(parent)?;
    }

    let name = file
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("diagprint");

    let temporary = parent.join(format!(".{name}.diagprint-{}.tmp", Uuid::now_v7()));

    fs::write(&temporary, contents)?;

    if let Ok(metadata) = fs::metadata(file) {
        fs::set_permissions(&temporary, metadata.permissions())?;
    }

    if let Err(error) = fs::rename(&temporary, file) {
        let _ = fs::remove_file(&temporary);
        return Err(FixError::Io(error));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{InteractiveAction, interactive_prompt, parse_interactive_action};

    #[test]
    fn interactive_prompt_only_offers_valid_actions() {
        assert_eq!(
            interactive_prompt(true, true),
            "[A]pply  [S]kip  [D]ocs  [Q]uit > "
        );

        assert_eq!(interactive_prompt(false, true), "[S]kip  [D]ocs  [Q]uit > ");

        assert_eq!(
            interactive_prompt(true, false),
            "[A]pply  [S]kip  [Q]uit > "
        );

        assert_eq!(interactive_prompt(false, false), "[S]kip  [Q]uit > ");
    }

    #[test]
    fn parser_rejects_unavailable_actions() {
        assert_eq!(parse_interactive_action("a", false, true), None);

        assert_eq!(parse_interactive_action("d", true, false), None);

        assert_eq!(
            parse_interactive_action("apply", true, false),
            Some(InteractiveAction::Apply)
        );

        assert_eq!(
            parse_interactive_action("quit", false, false),
            Some(InteractiveAction::Quit)
        );
    }
}
