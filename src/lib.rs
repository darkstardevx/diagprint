//! # diagprint
//!
//! Pretty, structured diagnostics and reports for Rust applications and
//! command-line tools.
//!
//! `diagprint` separates diagnostic data from presentation so the same
//! [`Diagnostic`] can be rendered to a rich terminal interface, plain text,
//! JSON, or Markdown.
//!
//! ## Quick start
//!
//! ```
//! use diagprint::{Reporter, Severity};
//!
//! # fn main() -> diagprint::Result<()> {
//! let reporter = Reporter::builder()
//!     .application("myapp")
//!     .min_severity(Severity::Info)
//!     .build()?;
//!
//! let diagnostic = reporter
//!     .error("Network initialization failed")
//!     .code("NET-001")
//!     .cause("failed to open network interface")
//!     .help("Check interface permissions and driver state");
//!
//! reporter.emit(&diagnostic)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Highlights
//!
//! - Rich terminal diagnostics
//! - Unicode-aware display width
//! - Automatic terminal sizing
//! - Source snippets and highlighted ranges
//! - Intelligent clipping around source locations
//! - Hierarchical error chains
//! - JSON, Markdown, plain-text, and terminal renderers
//! - File output with rotation and retention
//! - Optional gzip and Zstandard compression
//! - Custom terminal themes
//! - Optional Cybercore theme integration
//! - UUIDv7 session and report identifiers
//!
//! ## Feature flags
//!
//! `compression` enables gzip and Zstandard support.
//!
//! `cybercore` enables integration with the Cybercore color schema and named
//! theme system.

mod diagnostic;
mod reporter;
mod rotation;
mod severity;

pub mod render;

pub use diagnostic::{Cause, Diagnostic, Label, SourceLocation};
pub use render::{SeverityTheme, Style, Theme};
pub use reporter::{Compression, Reporter, ReporterBuilder};
pub use rotation::{RotationCadence, RotationPolicy, RotationState};
pub use severity::Severity;

pub type Result<T> = std::io::Result<T>;
