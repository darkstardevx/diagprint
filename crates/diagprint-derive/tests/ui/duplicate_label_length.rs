#![allow(dead_code)]

#[derive(diagprint_derive::Diagnostic)]
struct BadDiagnostic {
    #[diag(
        primary,
        length = length,
        length = length
    )]
    location: usize,

    length: usize,
}

fn main() {}
