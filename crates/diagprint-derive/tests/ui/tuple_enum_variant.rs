#![allow(dead_code)]

#[derive(diagprint_derive::Diagnostic)]
enum BadDiagnostic {
    Broken(u32),
}

fn main() {}
