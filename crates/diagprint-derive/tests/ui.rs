#[test]
fn diagnostic_derive_rejects_invalid_metadata() {
    let tests = trybuild::TestCases::new();

    tests.compile_fail("tests/ui/*.rs");
}
