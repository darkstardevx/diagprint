#![allow(dead_code)]

#[derive(diagprint_derive::Diagnostic)]
#[diag(suggestion(title = "first", title = "second"))]
struct BadDiagnostic {}

fn main() {}
