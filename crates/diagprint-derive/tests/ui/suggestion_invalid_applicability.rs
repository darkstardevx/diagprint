#![allow(dead_code)]

#[derive(diagprint_derive::Diagnostic)]
#[diag(suggestion(
    title = "fix it",
    applicability = definitely_safe
))]
struct BadDiagnostic {}

fn main() {}
