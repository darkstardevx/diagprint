use diagprint::render::{PlainRenderer, Renderer};
use diagprint::{DiagnosticValue, Reporter};

#[test]
fn diagnostic_attributes_preserve_types() {
    let reporter = Reporter::builder()
        .application("attribute-test")
        .build()
        .unwrap();

    let diagnostic = reporter
        .info("typed fields")
        .attribute("enabled", true)
        .attribute("signed", -42_i64)
        .attribute("unsigned", 42_u64)
        .attribute("large_signed", -84_i128)
        .attribute("large_unsigned", 84_u128)
        .attribute("ratio", 1.5_f64)
        .attribute("name", "cache");

    assert!(matches!(
        diagnostic.attributes[0].value,
        DiagnosticValue::Bool(true)
    ));

    assert!(matches!(
        diagnostic.attributes[1].value,
        DiagnosticValue::I64(-42)
    ));

    assert!(matches!(
        diagnostic.attributes[2].value,
        DiagnosticValue::U64(42)
    ));

    assert!(matches!(
        diagnostic.attributes[5].value,
        DiagnosticValue::F64(value)
            if value == 1.5
    ));

    let value = serde_json::to_value(&diagnostic).unwrap();

    let attributes = value["attributes"].as_array().unwrap();

    assert_eq!(attributes[0]["value"], true);

    assert_eq!(attributes[1]["value"], -42);

    assert_eq!(attributes[2]["value"], 42);

    assert_eq!(attributes[6]["value"], "cache");
}

#[test]
fn empty_attributes_do_not_change_json_shape() {
    let reporter = Reporter::builder()
        .application("attribute-test")
        .build()
        .unwrap();

    let value = serde_json::to_value(reporter.info("plain")).unwrap();

    assert!(value.get("attributes").is_none());
}

#[test]
fn plain_renderer_exposes_structured_attributes() {
    let reporter = Reporter::builder()
        .application("attribute-test")
        .build()
        .unwrap();

    let diagnostic = reporter
        .warning("cache")
        .attribute("user_id", 42_u64)
        .attribute("retry", true);

    let rendered = PlainRenderer.render(&diagnostic);

    assert!(rendered.contains("attributes:"));

    assert!(rendered.contains("user_id=42"));

    assert!(rendered.contains("retry=true"));
}
