use diagprint::{
    Reporter, SourceCache,
    render::{Renderer, SarifRenderer},
};
use serde_json::Value;

fn parse(rendered: &str) -> Value {
    serde_json::from_str(rendered).expect("valid SARIF JSON")
}

#[test]
fn renders_sarif_2_1_0_document() {
    let reporter = Reporter::builder()
        .application("sarif-test")
        .build()
        .unwrap();

    let diagnostic = reporter.error("invalid value").code("E100").label(
        "src/main.rs",
        7,
        Some(5),
        Some(4),
        Some("bad value"),
    );

    let sarif = parse(&SarifRenderer.render(&diagnostic));

    assert_eq!(sarif["version"].as_str(), Some("2.1.0"));

    assert_eq!(
        sarif["runs"][0]["tool"]["driver"]["name"].as_str(),
        Some("diagprint")
    );

    assert_eq!(
        sarif["runs"][0]["results"][0]["ruleId"].as_str(),
        Some("E100")
    );

    assert_eq!(
        sarif["runs"][0]["results"][0]["level"].as_str(),
        Some("error")
    );
}

#[test]
fn sarif_end_column_is_exclusive() {
    let reporter = Reporter::builder()
        .application("sarif-test")
        .build()
        .unwrap();

    let diagnostic =
        reporter
            .error("range test")
            .label("src/main.rs", 3, Some(5), Some(4), None::<String>);

    let sarif = parse(&SarifRenderer.render(&diagnostic));

    let region = &sarif["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["region"];

    assert_eq!(region["startLine"].as_u64(), Some(3));

    assert_eq!(region["startColumn"].as_u64(), Some(5));

    // Columns 5, 6, 7, 8 are selected.
    // SARIF endColumn is the exclusive column 9.
    assert_eq!(region["endColumn"].as_u64(), Some(9));
}

#[test]
fn zero_length_is_an_insertion_point() {
    let reporter = Reporter::builder()
        .application("sarif-test")
        .build()
        .unwrap();

    let diagnostic =
        reporter
            .error("insert here")
            .label("src/main.rs", 1, Some(8), Some(0), None::<String>);

    let sarif = parse(&SarifRenderer.render(&diagnostic));

    let region = &sarif["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["region"];

    assert_eq!(region["startColumn"].as_u64(), Some(8));

    assert_eq!(region["endColumn"].as_u64(), Some(8));
}

#[test]
fn secondary_labels_become_related_locations() {
    let reporter = Reporter::builder()
        .application("sarif-test")
        .build()
        .unwrap();

    let diagnostic = reporter
        .error("type mismatch")
        .code("E200")
        .label("src/main.rs", 5, Some(9), Some(4), Some("use"))
        .secondary_label("src/lib.rs", 2, Some(5), Some(7), Some("declared here"));

    let sarif = parse(&SarifRenderer.render(&diagnostic));

    let result = &sarif["runs"][0]["results"][0];

    assert_eq!(
        result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"].as_str(),
        Some("src/main.rs")
    );

    assert_eq!(result["relatedLocations"][0]["id"].as_u64(), Some(1));

    assert_eq!(
        result["relatedLocations"][0]["physicalLocation"]["artifactLocation"]["uri"].as_str(),
        Some("src/lib.rs")
    );

    assert_eq!(
        result["relatedLocations"][0]["message"]["text"].as_str(),
        Some("declared here")
    );
}

#[test]
fn uncoded_diagnostics_get_stable_fallback_rule_ids() {
    let reporter = Reporter::builder()
        .application("sarif-test")
        .build()
        .unwrap();

    let diagnostic = reporter.warning("uncoded warning");

    let sarif = parse(&SarifRenderer.render(&diagnostic));

    assert_eq!(
        sarif["runs"][0]["results"][0]["ruleId"].as_str(),
        Some("diagprint/uncoded/warning")
    );
}

#[test]
fn render_many_deduplicates_rules() {
    let reporter = Reporter::builder()
        .application("sarif-test")
        .build()
        .unwrap();

    let first = reporter.error("first").code("E100");

    let second = reporter.error("second").code("E100");

    let third = reporter.warning("third").code("W200");

    let rendered = SarifRenderer.render_many([&first, &second, &third]);

    let sarif = parse(&rendered);

    let rules = sarif["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .unwrap();

    let results = sarif["runs"][0]["results"].as_array().unwrap();

    assert_eq!(rules.len(), 2);
    assert_eq!(results.len(), 3);

    assert_eq!(rules[0]["id"].as_str(), Some("E100"));

    assert_eq!(rules[1]["id"].as_str(), Some("W200"));

    assert_eq!(results[0]["ruleIndex"].as_u64(), Some(0));

    assert_eq!(results[1]["ruleIndex"].as_u64(), Some(0));

    assert_eq!(results[2]["ruleIndex"].as_u64(), Some(1));
}

#[test]
fn preserves_source_revision_as_sarif_property() {
    const NAME: &str = "src/revisioned.rs";

    let cache = SourceCache::new();

    cache.insert(NAME, "let value = old();\n");

    let snapshot = cache.snapshot();

    let reporter = Reporter::builder()
        .application("sarif-test")
        .build()
        .unwrap();

    let diagnostic = reporter
        .error("revision test")
        .label(NAME, 1, Some(5), Some(5), None::<String>)
        .bind_source_revisions(&snapshot);

    let sarif = parse(&SarifRenderer.render(&diagnostic));

    assert_eq!(
        sarif["runs"][0]["results"][0]["locations"][0]["properties"]["diagprintSourceRevision"]
            .as_u64(),
        Some(1)
    );
}

#[test]
fn notes_help_and_causes_are_preserved_in_message() {
    let reporter = Reporter::builder()
        .application("sarif-test")
        .build()
        .unwrap();

    let diagnostic = reporter
        .error("top-level failure")
        .note("important context")
        .help("try another value")
        .cause("inner failure");

    let sarif = parse(&SarifRenderer.render(&diagnostic));

    let message = sarif["runs"][0]["results"][0]["message"]["text"]
        .as_str()
        .unwrap();

    assert!(message.contains("top-level failure"));

    assert!(message.contains("Note: important context"));

    assert!(message.contains("Help: try another value"));

    assert!(message.contains("Caused by: inner failure"));
}
