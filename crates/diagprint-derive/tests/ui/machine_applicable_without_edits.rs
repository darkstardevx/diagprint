#![allow(dead_code)]

#[derive(diagprint_derive::Diagnostic)]
#[diag(suggestion(
    title = "apply automatically",
    applicability = machine_applicable
))]
struct BadDiagnostic {}

fn main() {}
