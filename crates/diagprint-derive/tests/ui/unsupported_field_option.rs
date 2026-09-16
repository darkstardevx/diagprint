#![allow(dead_code)]

#[derive(diagprint_derive::Diagnostic)]
struct BadDiagnostic {
    #[diag(primary, unsupported = "value")]
    location: usize,
}

fn main() {}
