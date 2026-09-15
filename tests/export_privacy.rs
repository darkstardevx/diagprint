use diagprint::render::{JsonRenderer, Renderer};
use diagprint::{
    Applicability, DiagnosticSink, DocumentationLink, Edit, JsonLinesSink, Reporter,
    SuggestedCommand, Suggestion, TextRange,
};

fn private_diagnostic() -> diagprint::Diagnostic {
    let reporter = Reporter::builder()
        .application("export-test")
        .build()
        .unwrap();

    reporter
        .error("configuration failed")
        .attribute("authorization", "Bearer super-secret")
        .label(
            "/home/alice/private/config.rs",
            4,
            Some(2),
            Some(6),
            Some("bad configuration"),
        )
        .suggestion(
            Suggestion::new("repair configuration")
                .explanation("replace the private value")
                .applicability(Applicability::MachineApplicable)
                .documentation(DocumentationLink::new(
                    "private docs",
                    "https://alice:password@example.com/docs?token=secret#private",
                ))
                .edit(Edit::replace(
                    "/home/alice/private/config.rs",
                    TextRange::new(10, 20),
                    "EXPECTED-SOURCE-SECRET",
                    "REPLACEMENT-SOURCE-SECRET",
                ))
                .command(SuggestedCommand::new("tool --token COMMAND-SECRET")),
        )
}

#[test]
fn json_renderer_never_exports_remediation_payloads_by_default() {
    let diagnostic = private_diagnostic();

    let rendered = JsonRenderer.render(&diagnostic);

    assert!(!rendered.contains("EXPECTED-SOURCE-SECRET"));

    assert!(!rendered.contains("REPLACEMENT-SOURCE-SECRET"));

    assert!(!rendered.contains("COMMAND-SECRET"));

    assert!(!rendered.contains("Bearer super-secret"));

    assert!(!rendered.contains("/home/alice/private"));

    assert!(!rendered.contains("alice:password"));

    assert!(!rendered.contains("token=secret"));

    assert!(rendered.contains("\"file\": \"config.rs\""));

    assert!(rendered.contains("https://example.com/docs"));

    assert!(rendered.contains("\"attribute_count\": 1"));

    assert!(rendered.contains("\"edit_count\": 1"));

    assert!(rendered.contains("\"command_count\": 1"));
}

#[test]
fn json_lines_sink_uses_the_same_safe_export_boundary() {
    let diagnostic = private_diagnostic();

    let sink = JsonLinesSink::new(Vec::<u8>::new());

    sink.emit(&diagnostic).unwrap();

    let bytes = sink.into_inner().unwrap();

    let rendered = String::from_utf8(bytes).unwrap();

    assert!(!rendered.contains("EXPECTED-SOURCE-SECRET"));

    assert!(!rendered.contains("REPLACEMENT-SOURCE-SECRET"));

    assert!(!rendered.contains("COMMAND-SECRET"));

    assert!(!rendered.contains("Bearer super-secret"));

    assert!(rendered.contains("\"edit_count\":1"));

    assert!(rendered.contains("\"command_count\":1"));
}

#[test]
fn sanitizer_recognizes_common_secret_keys() {
    for key in [
        "password",
        "PASSWORD",
        "api_key",
        "client-secret",
        "http.authorization",
        "auth.token",
        "cookie",
    ] {
        assert!(diagprint::is_sensitive_key(key), "{key} was not recognized",);
    }

    for key in ["user_id", "request_id", "cache_hit", "region"] {
        assert!(
            !diagprint::is_sensitive_key(key),
            "{key} was incorrectly sensitive",
        );
    }
}
