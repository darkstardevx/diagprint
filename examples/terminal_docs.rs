use diagprint::{DocumentationLink, TerminalDocViewer};
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let requested = env::args().nth(1);

    let link = match requested {
        Some(url) => DocumentationLink::new("Requested documentation", url).language("rust"),

        None => DocumentationLink::rust_error("E0277"),
    };

    TerminalDocViewer::new()
        .width(96)
        .max_code_blocks(6)
        .open_and_print(&link)?;

    Ok(())
}
