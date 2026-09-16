#![allow(dead_code)]

#[derive(diagprint_derive::Diagnostic)]
struct BadDiagnostic {
    #[diag(
        primary,
        message = "first",
        message = "second"
    )]
    location: usize,
}

fn main() {}
