#![allow(dead_code)]

#[derive(diagprint_derive::Diagnostic)]
#[diag(suggestion(applicability = manual))]
struct BadDiagnostic {}

fn main() {}
