#![allow(dead_code)]

#[derive(diagprint_derive::Diagnostic)]
#[diag(unsupported = "value")]
struct BadDiagnostic {}

fn main() {}
