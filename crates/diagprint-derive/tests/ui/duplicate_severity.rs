#![allow(dead_code)]

#[derive(diagprint_derive::Diagnostic)]
#[diag(severity = warning, severity = error)]
struct BadDiagnostic {}

fn main() {}
