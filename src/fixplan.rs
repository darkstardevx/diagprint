use crate::{Applicability, Edit, FixError, Fixer, RollbackFailure, Suggestion};
use std::{
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
};

/// A deterministic filesystem condition used as either a
/// precondition or a post-apply verification step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileCheck {
    Exists { file: PathBuf },

    Missing { file: PathBuf },

    Contains { file: PathBuf, text: String },

    Equals { file: PathBuf, expected: String },
}

impl FileCheck {
    pub fn exists(file: impl Into<PathBuf>) -> Self {
        Self::Exists { file: file.into() }
    }

    pub fn missing(file: impl Into<PathBuf>) -> Self {
        Self::Missing { file: file.into() }
    }

    pub fn contains(file: impl Into<PathBuf>, text: impl Into<String>) -> Self {
        Self::Contains {
            file: file.into(),
            text: text.into(),
        }
    }

    pub fn equals(file: impl Into<PathBuf>, expected: impl Into<String>) -> Self {
        Self::Equals {
            file: file.into(),
            expected: expected.into(),
        }
    }

    pub fn file(&self) -> &Path {
        match self {
            Self::Exists { file }
            | Self::Missing { file }
            | Self::Contains { file, .. }
            | Self::Equals { file, .. } => file,
        }
    }

    pub fn description(&self) -> String {
        match self {
            Self::Exists { file } => {
                format!("{} exists", file.display())
            }

            Self::Missing { file } => {
                format!("{} is missing", file.display())
            }

            Self::Contains { file, text } => {
                format!("{} contains {:?}", file.display(), text)
            }

            Self::Equals { file, expected } => {
                format!("{} exactly matches {:?}", file.display(), expected)
            }
        }
    }

    fn evaluate(&self) -> Result<(), String> {
        match self {
            Self::Exists { file } => match file.try_exists() {
                Ok(true) => Ok(()),

                Ok(false) => Err("file does not exist".into()),

                Err(error) => Err(error.to_string()),
            },

            Self::Missing { file } => match file.try_exists() {
                Ok(false) => Ok(()),

                Ok(true) => Err("file exists".into()),

                Err(error) => Err(error.to_string()),
            },

            Self::Contains { file, text } => {
                let content = fs::read_to_string(file).map_err(|error| error.to_string())?;

                if content.contains(text.as_str()) {
                    Ok(())
                } else {
                    Err(format!("expected text {:?} was not found", text))
                }
            }

            Self::Equals { file, expected } => {
                let content = fs::read_to_string(file).map_err(|error| error.to_string())?;

                if content == expected.as_str() {
                    Ok(())
                } else {
                    Err("file contents do not exactly match the expected state".into())
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCheckFailure {
    pub check: FileCheck,
    pub reason: String,
}

impl fmt::Display for FileCheckFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.check.description(), self.reason)
    }
}

#[derive(Debug)]
pub enum FixPlanError {
    NotMachineApplicable {
        applicability: Applicability,
    },

    NoEdits,

    UnguardedInsert {
        file: PathBuf,
        offset: usize,
    },

    PreconditionsFailed {
        failures: Vec<FileCheckFailure>,
    },

    Fix(FixError),

    VerificationFailed {
        failures: Vec<FileCheckFailure>,

        rollback_failures: Vec<RollbackFailure>,
    },
}

impl fmt::Display for FixPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotMachineApplicable { applicability } => {
                write!(
                    f,
                    "fix plan is {}, not machine-applicable",
                    applicability.as_str()
                )
            }

            Self::NoEdits => {
                write!(f, "fix plan contains no edits")
            }

            Self::UnguardedInsert { file, offset } => {
                write!(
                    f,
                    "machine-applicable fix plan contains an unguarded insert at {}:{}; use Edit::insert_after or downgrade applicability",
                    file.display(),
                    offset
                )
            }

            Self::PreconditionsFailed { failures } => {
                write!(f, "{} fix-plan precondition(s) failed", failures.len())
            }

            Self::Fix(error) => {
                write!(f, "fix plan could not be applied: {error}")
            }

            Self::VerificationFailed {
                failures,
                rollback_failures,
            } => {
                write!(
                    f,
                    "{} post-apply verification check(s) failed",
                    failures.len()
                )?;

                if rollback_failures.is_empty() {
                    write!(f, "; transaction rolled back")
                } else {
                    write!(
                        f,
                        "; rollback was incomplete ({} restoration failure(s))",
                        rollback_failures.len()
                    )
                }
            }
        }
    }
}

impl Error for FixPlanError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Fix(error) => Some(error),

            _ => None,
        }
    }
}

impl From<FixError> for FixPlanError {
    fn from(value: FixError) -> Self {
        Self::Fix(value)
    }
}

#[derive(Debug, Clone)]
pub struct FixPlanPreview {
    pub title: String,
    pub applicability: Applicability,
    pub lines: Vec<String>,
}

impl FixPlanPreview {
    pub fn render(&self) -> String {
        let mut output = format!(
            "FIX PLAN  {}\nAPPLICABILITY  {}\n",
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

#[derive(Debug, Clone)]
pub struct FixPlanCheck {
    pub affected_files: Vec<PathBuf>,

    pub preconditions_checked: usize,

    pub verifications_planned: usize,
}

#[derive(Debug, Clone)]
pub struct FixPlanReport {
    pub changed_files: Vec<PathBuf>,

    pub verification_checks: usize,

    pub verification_passed: bool,
}

impl FixPlanReport {
    pub fn verified(&self) -> bool {
        self.verification_passed
    }
}

/// A transactional remediation plan composed of explicit
/// filesystem edits, preconditions, and deterministic
/// post-apply verification checks.
///
/// `FixPlan` never executes shell commands.
///
/// All verification in this version is expressed through
/// [`FileCheck`] values.
///
/// A plan must be [`Applicability::MachineApplicable`] before
/// [`FixPlan::apply`] will mutate files.
#[derive(Debug, Clone)]
pub struct FixPlan {
    title: String,

    explanation: Option<String>,

    applicability: Applicability,

    preconditions: Vec<FileCheck>,

    edits: Vec<Edit>,

    verifications: Vec<FileCheck>,

    backups: bool,

    backup_suffix: String,
}

impl FixPlan {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),

            explanation: None,

            applicability: Applicability::Manual,

            preconditions: Vec::new(),

            edits: Vec::new(),

            verifications: Vec::new(),

            backups: false,

            backup_suffix: ".diagprint.bak".into(),
        }
    }

    pub fn from_suggestion(suggestion: &Suggestion) -> Self {
        Self {
            title: suggestion.title.clone(),

            explanation: suggestion.explanation.clone(),

            applicability: suggestion.applicability,

            preconditions: Vec::new(),

            edits: suggestion.edits.clone(),

            verifications: Vec::new(),

            backups: false,

            backup_suffix: ".diagprint.bak".into(),
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

    pub fn precondition(mut self, check: FileCheck) -> Self {
        self.preconditions.push(check);

        self
    }

    pub fn edit(mut self, edit: Edit) -> Self {
        self.edits.push(edit);
        self
    }

    pub fn edits(mut self, edits: impl IntoIterator<Item = Edit>) -> Self {
        self.edits.extend(edits);
        self
    }

    pub fn verify(mut self, check: FileCheck) -> Self {
        self.verifications.push(check);

        self
    }

    pub fn backups(mut self, enabled: bool) -> Self {
        self.backups = enabled;
        self
    }

    pub fn backup_suffix(mut self, suffix: impl Into<String>) -> Self {
        self.backup_suffix = suffix.into();

        self
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn applicability_value(&self) -> Applicability {
        self.applicability
    }

    pub fn preview(&self) -> FixPlanPreview {
        let mut lines = Vec::new();

        if let Some(explanation) = &self.explanation {
            lines.push(format!("WHY    {explanation}"));
        }

        for check in &self.preconditions {
            lines.push(format!("PRECONDITION  {}", check.description()));
        }

        for edit in &self.edits {
            lines.extend(edit.preview_lines());
        }

        for check in &self.verifications {
            lines.push(format!("VERIFY  {}", check.description()));
        }

        lines.push(format!(
            "TRANSACTION  {}",
            if self.applicability.can_apply_automatically() && !self.edits.is_empty() {
                "rollback-on-error enabled"
            } else {
                "manual review required"
            }
        ));

        FixPlanPreview {
            title: self.title.clone(),

            applicability: self.applicability,

            lines,
        }
    }

    pub fn check(&self) -> Result<FixPlanCheck, FixPlanError> {
        self.ensure_automatic()?;
        self.check_preconditions()?;

        let fixer = Fixer::new();

        let affected_files = fixer.check_edits(&self.edits)?;

        Ok(FixPlanCheck {
            affected_files,

            preconditions_checked: self.preconditions.len(),

            verifications_planned: self.verifications.len(),
        })
    }

    pub fn apply(&self) -> Result<FixPlanReport, FixPlanError> {
        self.ensure_automatic()?;
        self.check_preconditions()?;

        let fixer = Fixer::new()
            .backups(self.backups)
            .backup_suffix(self.backup_suffix.clone());

        /*
         * apply_edits_transaction() re-reads and re-validates
         * every edit immediately before mutation.
         */
        let transaction = fixer.apply_edits_transaction(&self.edits)?;

        let changed_files = transaction.changed_files();

        let verification_failures = evaluate_checks(&self.verifications);

        if !verification_failures.is_empty() {
            /*
             * Verification failed after a successful write transaction.
             * Restore every changed file from the exact original contents
             * captured by the transaction.
             */
            let rollback_failures = transaction.rollback();

            return Err(FixPlanError::VerificationFailed {
                failures: verification_failures,

                rollback_failures,
            });
        }

        Ok(FixPlanReport {
            changed_files,

            verification_checks: self.verifications.len(),

            verification_passed: true,
        })
    }

    fn ensure_automatic(&self) -> Result<(), FixPlanError> {
        if self.edits.is_empty() {
            return Err(FixPlanError::NoEdits);
        }

        if !self.applicability.can_apply_automatically() {
            return Err(FixPlanError::NotMachineApplicable {
                applicability: self.applicability,
            });
        }

        /*
         * A machine-applicable insertion must carry an exact
         * expected-before guard.
         *
         * Plain Edit::insert() is fine for manual suggestions, but is too
         * weak for FixPlan automatic mutation.
         */
        for edit in &self.edits {
            if let Edit::Insert {
                file,
                offset,
                expected_before,
                ..
            } = edit
            {
                if !expected_before
                    .as_ref()
                    .is_some_and(|expected| !expected.is_empty())
                {
                    return Err(FixPlanError::UnguardedInsert {
                        file: file.clone(),

                        offset: *offset,
                    });
                }
            }
        }

        Ok(())
    }

    fn check_preconditions(&self) -> Result<(), FixPlanError> {
        let failures = evaluate_checks(&self.preconditions);

        if failures.is_empty() {
            Ok(())
        } else {
            Err(FixPlanError::PreconditionsFailed { failures })
        }
    }
}

fn evaluate_checks(checks: &[FileCheck]) -> Vec<FileCheckFailure> {
    checks
        .iter()
        .filter_map(|check| {
            check.evaluate().err().map(|reason| FileCheckFailure {
                check: check.clone(),

                reason,
            })
        })
        .collect()
}
