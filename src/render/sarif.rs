use super::Renderer;
use crate::{Diagnostic, ExportPolicy, Label, LabelKind, Severity};
use serde_json::{Map, Value, json};
use std::{collections::BTreeMap, fs, io, path::Path};

/// Renders diagnostics as SARIF 2.1.0.
///
/// A single [`struct@Diagnostic`] becomes one SARIF result.
///
/// For source locations:
///
/// - the first primary label becomes the result's main location;
/// - if there is no primary label, the first available label is used;
/// - all remaining labels become `relatedLocations`.
///
/// SARIF regions use exclusive `endColumn` semantics.
///
/// Existing [`Renderer`] behavior preserves source paths exactly as supplied.
/// Use [`SarifRenderer::render_many_with_policy`] when an explicit external
/// privacy policy is required.
#[derive(Debug, Default, Clone, Copy)]
pub struct SarifRenderer;

impl SarifRenderer {
    const SCHEMA: &'static str = "https://json.schemastore.org/sarif-2.1.0.json";

    /// Renders multiple diagnostics into one complete SARIF document.
    ///
    /// This preserves the historical path and text behavior.
    pub fn render_many<'a>(&self, diagnostics: impl IntoIterator<Item = &'a Diagnostic>) -> String {
        self.render_many_internal(diagnostics, None)
    }

    /// Renders multiple diagnostics into one complete SARIF document using an
    /// explicit external-export policy.
    pub fn render_many_with_policy<'a>(
        &self,
        diagnostics: impl IntoIterator<Item = &'a Diagnostic>,
        policy: &ExportPolicy,
    ) -> String {
        self.render_many_internal(diagnostics, Some(policy))
    }

    fn render_many_internal<'a>(
        &self,
        diagnostics: impl IntoIterator<Item = &'a Diagnostic>,
        policy: Option<&ExportPolicy>,
    ) -> String {
        let diagnostics: Vec<&Diagnostic> = diagnostics.into_iter().collect();

        let mut rule_sources: BTreeMap<String, &Diagnostic> = BTreeMap::new();

        for &diagnostic in &diagnostics {
            let rule_id = Self::rule_id(diagnostic);

            rule_sources.entry(rule_id).or_insert(diagnostic);
        }

        let rule_indices: BTreeMap<String, usize> = rule_sources
            .keys()
            .enumerate()
            .map(|(index, rule_id)| (rule_id.clone(), index))
            .collect();

        let rules: Vec<Value> = rule_sources
            .iter()
            .map(|(rule_id, diagnostic)| Self::rule_descriptor(rule_id, diagnostic, policy))
            .collect();

        let results: Vec<Value> = diagnostics
            .iter()
            .map(|diagnostic| Self::result(diagnostic, &rule_indices, policy))
            .collect();

        let log = json!({
            "$schema": Self::SCHEMA,
            "version": "2.1.0",
            "runs": [
                {
                    "tool": {
                        "driver": {
                            "name": "diagprint",
                            "semanticVersion": env!(
                                "CARGO_PKG_VERSION"
                            ),
                            "informationUri": env!(
                                "CARGO_PKG_REPOSITORY"
                            ),
                            "rules": rules
                        }
                    },
                    "results": results
                }
            ]
        });

        serde_json::to_string_pretty(&log).expect("SARIF serialization failed")
    }

    /// Writes one complete SARIF document.
    ///
    /// Unlike `Reporter`'s normal file output, this overwrites the destination
    /// instead of appending independent rendered documents.
    pub fn write_many<'a>(
        &self,
        path: impl AsRef<Path>,
        diagnostics: impl IntoIterator<Item = &'a Diagnostic>,
    ) -> io::Result<()> {
        fs::write(path, self.render_many(diagnostics))
    }

    /// Writes one complete policy-controlled SARIF document.
    pub fn write_many_with_policy<'a>(
        &self,
        path: impl AsRef<Path>,
        diagnostics: impl IntoIterator<Item = &'a Diagnostic>,
        policy: &ExportPolicy,
    ) -> io::Result<()> {
        fs::write(path, self.render_many_with_policy(diagnostics, policy))
    }

    fn rule_id(diagnostic: &Diagnostic) -> String {
        if let Some(code) = diagnostic.code.as_deref().filter(|code| !code.is_empty()) {
            return code.to_owned();
        }

        format!(
            "diagprint/uncoded/{}",
            Self::severity_slug(diagnostic.severity)
        )
    }

    fn severity_slug(severity: Severity) -> &'static str {
        match severity {
            Severity::Trace => "trace",
            Severity::Debug => "debug",
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
            Severity::Fatal => "fatal",
        }
    }

    fn level(severity: Severity) -> &'static str {
        match severity {
            Severity::Trace | Severity::Debug => "none",

            Severity::Info => "note",

            Severity::Warning => "warning",

            Severity::Error | Severity::Fatal => "error",
        }
    }

    fn apply_text(value: &str, policy: Option<&ExportPolicy>) -> String {
        match policy {
            Some(policy) => policy.apply_text(value),

            None => value.to_owned(),
        }
    }

    fn rule_descriptor(
        rule_id: &str,
        diagnostic: &Diagnostic,
        policy: Option<&ExportPolicy>,
    ) -> Value {
        let mut rule = Map::new();

        rule.insert("id".into(), json!(rule_id));

        rule.insert(
            "shortDescription".into(),
            json!({
                "text":
                    Self::apply_text(
                        &diagnostic.message,
                        policy,
                    )
            }),
        );

        rule.insert(
            "defaultConfiguration".into(),
            json!({
                "level":
                    Self::level(
                        diagnostic.severity
                    )
            }),
        );

        if let Some(help) = diagnostic.help.as_deref().filter(|help| !help.is_empty()) {
            rule.insert(
                "help".into(),
                json!({
                    "text":
                        Self::apply_text(
                            help,
                            policy,
                        )
                }),
            );
        }

        Value::Object(rule)
    }

    fn result(
        diagnostic: &Diagnostic,
        rule_indices: &BTreeMap<String, usize>,
        policy: Option<&ExportPolicy>,
    ) -> Value {
        let rule_id = Self::rule_id(diagnostic);

        let rule_index = *rule_indices
            .get(&rule_id)
            .expect("SARIF rule index must exist");

        let mut result = Map::new();

        result.insert("ruleId".into(), json!(rule_id));

        result.insert("ruleIndex".into(), json!(rule_index));

        result.insert("level".into(), json!(Self::level(diagnostic.severity)));

        result.insert(
            "message".into(),
            json!({
                "text":
                    Self::result_message(
                        diagnostic,
                        policy,
                    )
            }),
        );

        if let Some(primary_index) = Self::primary_label_index(diagnostic) {
            let primary = &diagnostic.labels[primary_index];

            result.insert(
                "locations".into(),
                Value::Array(vec![Self::location(primary, None, policy)]),
            );

            let mut related = Vec::new();

            for (index, label) in diagnostic.labels.iter().enumerate() {
                if index == primary_index {
                    continue;
                }

                let id = related.len() + 1;

                related.push(Self::location(label, Some(id), policy));
            }

            if !related.is_empty() {
                result.insert("relatedLocations".into(), Value::Array(related));
            }
        }

        Value::Object(result)
    }

    fn primary_label_index(diagnostic: &Diagnostic) -> Option<usize> {
        diagnostic
            .labels
            .iter()
            .position(|label| label.kind == LabelKind::Primary)
            .or((!diagnostic.labels.is_empty()).then_some(0))
    }

    fn location(label: &Label, id: Option<usize>, policy: Option<&ExportPolicy>) -> Value {
        let mut region = Map::new();

        region.insert("startLine".into(), json!(label.location.line.max(1)));

        if let Some(column) = label.location.column {
            let column = column.max(1);

            region.insert("startColumn".into(), json!(column));

            if let Some(length) = label.length {
                let length = u32::try_from(length).unwrap_or(u32::MAX);

                let end_column = column.saturating_add(length);

                region.insert("endColumn".into(), json!(end_column));
            }
        }

        let mut physical_location = Map::new();

        let exported_path = match policy {
            Some(policy) => policy.apply_path(&label.location.file),

            None => Some(label.location.file.clone()),
        };

        if let Some(file) = exported_path {
            physical_location.insert(
                "artifactLocation".into(),
                json!({
                    "uri": file
                }),
            );
        }

        physical_location.insert("region".into(), Value::Object(region));

        let mut location = Map::new();

        if let Some(id) = id {
            location.insert("id".into(), json!(id));
        }

        location.insert("physicalLocation".into(), Value::Object(physical_location));

        if let Some(message) = label
            .message
            .as_deref()
            .filter(|message| !message.is_empty())
        {
            location.insert(
                "message".into(),
                json!({
                    "text":
                        Self::apply_text(
                            message,
                            policy,
                        )
                }),
            );
        }

        if let Some(revision) = label.location.revision {
            location.insert(
                "properties".into(),
                json!({
                    "diagprintSourceRevision":
                        revision.get()
                }),
            );
        }

        Value::Object(location)
    }

    fn result_message(diagnostic: &Diagnostic, policy: Option<&ExportPolicy>) -> String {
        let mut message = diagnostic.message.clone();

        for note in &diagnostic.notes {
            message.push_str("\n\nNote: ");

            message.push_str(note);
        }

        if let Some(help) = &diagnostic.help {
            message.push_str("\n\nHelp: ");

            message.push_str(help);
        }

        if let Some(cause) = &diagnostic.cause {
            for cause in cause.iter() {
                message.push_str("\n\nCaused by: ");

                message.push_str(&cause.message);
            }
        }

        Self::apply_text(&message, policy)
    }
}

impl Renderer for SarifRenderer {
    fn render(&self, diagnostic: &Diagnostic) -> String {
        self.render_many([diagnostic])
    }
}
