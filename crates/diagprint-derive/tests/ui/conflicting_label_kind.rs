#![allow(dead_code)]

#[derive(diagprint_derive::Diagnostic)]
struct BadDiagnostic {
    #[diag(primary, secondary)]
    location: usize,
}

fn main() {}
