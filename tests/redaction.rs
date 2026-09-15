use diagprint::{REDACTED, RedactionPolicy, Sensitive};

#[test]
fn sensitive_values_redact_display_and_debug() {
    let secret = Sensitive::new("super-secret-token");

    assert_eq!(secret.to_string(), REDACTED);

    let debug = format!("{secret:?}");

    assert!(!debug.contains("super-secret-token"));
}

#[test]
fn sensitive_values_serialize_redacted() {
    let secret = Sensitive::new("super-secret-token");

    let json = serde_json::to_string(&secret).unwrap();

    assert_eq!(json, "\"[REDACTED]\"");
}

#[test]
fn revealing_requires_explicit_policy() {
    let secret = Sensitive::new("super-secret-token");

    assert_eq!(RedactionPolicy::Redact.format(&secret), REDACTED);

    assert_eq!(
        RedactionPolicy::Reveal.format(&secret),
        "super-secret-token"
    );

    assert_eq!(secret.expose(), &"super-secret-token");
}
