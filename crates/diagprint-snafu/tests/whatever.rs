use diagprint_snafu::{SnafuCode, SnafuIdentity, WhateverDiagnosticContext};
#[test]
fn whatever_context_keeps_identity_separate_from_message() {
    let a = WhateverDiagnosticContext::new(
        SnafuIdentity::new("config.read").unwrap(),
        "Could not read /home/alice/config.toml",
    )
    .code(SnafuCode::new("CONFIG_READ").unwrap());
    let b = WhateverDiagnosticContext::new(
        SnafuIdentity::new("config.read").unwrap(),
        "Could not read /srv/app/config.toml",
    )
    .code(SnafuCode::new("CONFIG_READ").unwrap());
    assert_eq!(a.identity(), b.identity());
    assert_eq!(a.code_value(), b.code_value());
    assert_ne!(a.message(), b.message());
}
#[test]
fn whatever_context_rejects_message_shaped_identity() {
    assert!(SnafuIdentity::new("Could not read /tmp/config").is_err());
}
