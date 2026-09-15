use crate::{
    Applicability, Diagnostic, Edit, Suggestion,
    remediation::{affected_files, apply_transaction, prepare_edits},
};
use std::{
    io::{self, Write},
    path::PathBuf,
};

pub use crate::remediation::{FixError, RollbackFailure};

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

    /// Validates every machine-applicable edit against the current
    /// filesystem without changing any files.
    pub fn check(&self, diagnostic: &Diagnostic) -> Result<FixCheck, FixError> {
        let suggestions = applicable_suggestions(diagnostic);

        let edits = suggestion_edits(&suggestions);

        let prepared = prepare_edits(&edits)?;

        Ok(FixCheck {
            applicable_suggestions: suggestions.len(),

            affected_files: affected_files(&prepared),
        })
    }

    /// Applies all machine-applicable suggestions as one
    /// rollback-on-error transaction.
    ///
    /// All edits are prepared and validated before the first target
    /// file is changed.
    ///
    /// If a later target write fails, files already written by the
    /// transaction are restored from their in-memory originals.
    pub fn apply(&self, diagnostic: &Diagnostic) -> Result<FixReport, FixError> {
        let suggestions = applicable_suggestions(diagnostic);

        self.apply_suggestions(&suggestions)
    }

    pub fn apply_interactive(&self, diagnostic: &Diagnostic) -> Result<FixReport, FixError> {
        let mut report = FixReport::default();

        for suggestion in &diagnostic.suggestions {
            println!();

            print!("{}", Self::preview_suggestion(suggestion,).render());

            let mut can_apply = suggestion.is_machine_applicable() && suggestion.has_edits();

            if can_apply {
                let edits: Vec<&Edit> = suggestion.edits.iter().collect();

                match prepare_edits(&edits) {
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

    pub(crate) fn check_edits(&self, edits: &[Edit]) -> Result<Vec<PathBuf>, FixError> {
        let edit_refs: Vec<&Edit> = edits.iter().collect();

        let prepared = prepare_edits(&edit_refs)?;

        Ok(affected_files(&prepared))
    }

    pub(crate) fn apply_edits_transaction(
        &self,
        edits: &[Edit],
    ) -> Result<crate::remediation::AppliedTransaction, FixError> {
        let edit_refs: Vec<&Edit> = edits.iter().collect();

        let prepared = prepare_edits(&edit_refs)?;

        apply_transaction(prepared, self.backup, &self.backup_suffix)
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
        let edits = suggestion_edits(suggestions);

        let prepared = prepare_edits(&edits)?;

        let transaction = apply_transaction(prepared, self.backup, &self.backup_suffix)?;

        Ok(FixReport {
            applied_suggestions: suggestions.len(),

            changed_files: transaction.changed_files(),
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

fn suggestion_edits<'a>(suggestions: &[&'a Suggestion]) -> Vec<&'a Edit> {
    suggestions
        .iter()
        .flat_map(|suggestion| suggestion.edits.iter())
        .collect()
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

#[cfg(test)]
mod tests {
    use super::{InteractiveAction, interactive_prompt, parse_interactive_action};

    #[test]
    fn interactive_prompt_only_offers_valid_actions() {
        assert_eq!(
            interactive_prompt(true, true,),
            "[A]pply  [S]kip  [D]ocs  [Q]uit > "
        );

        assert_eq!(
            interactive_prompt(false, true,),
            "[S]kip  [D]ocs  [Q]uit > "
        );

        assert_eq!(
            interactive_prompt(true, false,),
            "[A]pply  [S]kip  [Q]uit > "
        );

        assert_eq!(interactive_prompt(false, false,), "[S]kip  [Q]uit > ");
    }

    #[test]
    fn parser_rejects_unavailable_actions() {
        assert_eq!(parse_interactive_action("a", false, true,), None);

        assert_eq!(parse_interactive_action("d", true, false,), None);

        assert_eq!(
            parse_interactive_action("apply", true, false,),
            Some(InteractiveAction::Apply)
        );

        assert_eq!(
            parse_interactive_action("quit", false, false,),
            Some(InteractiveAction::Quit)
        );
    }
}
