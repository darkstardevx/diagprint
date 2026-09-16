use axum::http::StatusCode;
use diagprint::Reporter;
use diagprint_axum::{
    DiagnosticResponse, MAX_REQUEST_ID_LEN, ProblemDetailsResponse, ProblemDetailsResponseExt,
    RequestId,
};
use proptest::prelude::*;
use serde_json::Value;

const RESERVED_EXTENSION_NAMES: &[&str] = &[
    "type",
    "title",
    "status",
    "detail",
    "instance",
    "report_id",
    "request_id",
    "code",
];

fn expected_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_REQUEST_ID_LEN
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

fn expected_extension_name(name: &str) -> bool {
    let bytes = name.as_bytes();

    bytes.len() >= 3
        && bytes.first().is_some_and(u8::is_ascii_alphabetic)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
        && !RESERVED_EXTENSION_NAMES.contains(&name)
}

fn problem() -> ProblemDetailsResponse {
    let reporter = Reporter::builder()
        .application("diagprint-axum-property-tests")
        .build()
        .expect("property-test reporter should build");

    DiagnosticResponse::new(
        StatusCode::BAD_REQUEST,
        reporter.warning("private property-test diagnostic"),
    )
    .into_problem_details()
}

fn secret_problem(secret: &str) -> ProblemDetailsResponse {
    let reporter = Reporter::builder()
        .application(format!("private-application-{secret}"))
        .build()
        .expect("property-test reporter should build");

    let diagnostic = reporter
        .error(format!("private-message-{secret}"))
        .code(format!("private_code_{secret}"))
        .attribute("private.attribute", format!("private-attribute-{secret}"))
        .note(format!("private-note-{secret}"))
        .help(format!("private-help-{secret}"))
        .cause(format!("private-cause-{secret}"))
        .label(
            format!("/private/{secret}/source.rs"),
            42,
            Some(7),
            Some(11),
            Some(format!("private-label-{secret}")),
        );

    DiagnosticResponse::new(StatusCode::INTERNAL_SERVER_ERROR, diagnostic).into_problem_details()
}

fn arbitrary_text(max_chars: usize) -> impl Strategy<Value = String> {
    prop::collection::vec(any::<char>(), 0..=max_chars)
        .prop_map(|characters| characters.into_iter().collect())
}

fn generated_secret() -> impl Strategy<Value = String> {
    let alphabet =
        ("abcdefghijklmnopqrstuvwxyz".to_owned() + "ABCDEFGHIJKLMNOPQRSTUVWXYZ" + "0123456789")
            .chars()
            .collect::<Vec<_>>();

    prop::collection::vec(prop::sample::select(alphabet), 16..=48)
        .prop_map(|characters| characters.into_iter().collect())
}

proptest! {
    #![
        proptest_config(
            ProptestConfig {
                cases: 256,
                max_shrink_iters: 4096,
                ..ProptestConfig::default()
            }
        )
    ]

    #[test]
    fn request_id_acceptance_matches_public_contract(
        value in arbitrary_text(160),
    ) {
        let expected =
            expected_request_id(&value);

        let result =
            RequestId::new(value.clone());

        prop_assert_eq!(
            result.is_ok(),
            expected,
            "request ID contract mismatch for {:?}",
            value,
        );

        if let Ok(request_id) = result {
            prop_assert_eq!(
                request_id.as_str(),
                value.as_str(),
            );
        }
    }

    #[test]
    fn extension_name_acceptance_matches_contract(
        name in arbitrary_text(80),
    ) {
        let expected =
            expected_extension_name(&name);

        let result =
            problem().with_extension(
                name.clone(),
                Value::Bool(true),
            );

        prop_assert_eq!(
            result.is_ok(),
            expected,
            "Problem Details extension contract mismatch for {:?}",
            name,
        );
    }

    #[test]
    fn default_problem_details_never_expose_generated_secret(
        secret in generated_secret(),
    ) {
        let response =
            secret_problem(&secret);

        let document =
            response.problem_details();

        let serialized =
            serde_json::to_string(
                &document,
            )
            .expect(
                "Problem Details should serialize",
            );

        prop_assert!(
            !serialized.contains(
                &secret
            ),
            "private generated secret crossed the default HTTP boundary"
        );

        prop_assert!(
            document.code.is_none(),
            "private diagnostic code became public"
        );
    }
}
