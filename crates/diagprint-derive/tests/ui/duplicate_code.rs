#![allow(dead_code)]

#[derive(diagprint_derive::Diagnostic)]
#[diag(code = "E001", code = "E002")]
struct BadDiagnostic {}

fn main() {}
