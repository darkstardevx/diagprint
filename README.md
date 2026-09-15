# diagprint v0.3.0

v0.3 turns the prototype into a more serious diagnostic engine.

## New
- Width-aware boxed terminal renderer
- Source snippets and caret labels
- Hierarchical `Cause`
- `std::error::Error::source()` chain capture
- Size, hourly, and daily rotation
- Retention count cleanup
- Optional gzip/zstd compression (`--features compression`)
- Shared mutex for thread-safe file writes
- JSON, Markdown, plain, and terminal renderers
- Stable metadata: timestamp, app, PID, hostname, session UUIDv7, report UUIDv7

## Test
```bash
cargo test
cargo run --example basic
cargo run --example error_chain
cargo test --features compression
```

## Compression
```rust
use diagprint::Compression;
// .compression(Compression::Gzip)
// .compression(Compression::Zstd)
```
Enable with `cargo run --features compression ...`.

## Next
- dedicated `anyhow` feature/adapter
- thiserror examples (it works through std::error::Error already)
- line wrapping instead of truncation
- rotation collision hardening
- compressed archive retention awareness
- nonblocking/async writer
- themes and HTML renderer
