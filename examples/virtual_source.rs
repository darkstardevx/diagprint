use diagprint::{Reporter, SourceCache, render::TerminalRenderer};

fn main() {
    let sources = SourceCache::new();

    sources.insert(
        "memory://example/generated.rs",
        concat!("fn generated() {\n", "    let answer = mystery();\n", "}\n",),
    );

    let reporter = Reporter::builder()
        .application("virtual-source-example")
        .build()
        .unwrap();

    let diagnostic = reporter
        .error("Generated source contains an unresolved value")
        .code("E-VIRTUAL")
        .label(
            "memory://example/generated.rs",
            2,
            Some(9),
            Some(6),
            Some("this source exists only in memory"),
        );

    let renderer = TerminalRenderer {
        color: false,
        width: 72,
        ..Default::default()
    };

    print!("{}", renderer.render_with_sources(&diagnostic, &sources));
}
