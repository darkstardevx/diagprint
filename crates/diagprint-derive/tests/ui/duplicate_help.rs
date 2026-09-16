#![allow(dead_code)]

#[derive(diagprint_derive::Diagnostic)]
#[diag(help = "first", help = "second")]
struct BadDiagnostic {}

fn main() {}
