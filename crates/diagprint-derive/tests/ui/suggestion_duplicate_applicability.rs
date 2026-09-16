#![allow(dead_code)]

#[derive(diagprint_derive::Diagnostic)]
#[diag(suggestion(
    title = "fix it",
    applicability = manual,
    applicability = maybe_incorrect
))]
struct BadDiagnostic {}

fn main() {}
