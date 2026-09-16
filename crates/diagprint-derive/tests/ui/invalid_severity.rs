#![allow(dead_code)]

#[derive(diagprint_derive::Diagnostic)]
#[diag(severity = catastrophic)]
struct BadDiagnostic {}

fn main() {}
