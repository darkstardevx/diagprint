#![allow(dead_code)]

#[derive(diagprint_derive::Diagnostic)]
union BadDiagnostic {
    value: u32,
}

fn main() {}
