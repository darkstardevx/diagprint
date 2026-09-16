use crate::{Label, SourceCache, SourceRevision, SourceSnapshot};
use std::{fs, sync::Arc};
use unicode_width::UnicodeWidthChar;

pub(super) const DEFAULT_SOURCE_CONTEXT_LINES: usize = 2;
const TAB_WIDTH: usize = 4;

#[derive(Clone, Copy)]
pub(super) enum SourceStore<'a> {
    Cache(&'a SourceCache),
    Snapshot(&'a SourceSnapshot),
}

impl SourceStore<'_> {
    fn get(self, name: &str) -> Option<Arc<str>> {
        match self {
            Self::Cache(cache) => cache.get(name),
            Self::Snapshot(snapshot) => snapshot.get(name),
        }
    }

    fn revision(self, name: &str) -> Option<SourceRevision> {
        match self {
            Self::Cache(cache) => cache.revision(name),
            Self::Snapshot(snapshot) => snapshot.revision(name),
        }
    }
}

pub(super) struct SourceContext {
    pub language: &'static str,
    pub start_line: usize,
    pub end_line: usize,
    pub target_line: usize,
    pub target_text: String,
    pub text: String,
}

pub(super) enum SourceResolution {
    Available(SourceContext),

    Stale {
        expected: SourceRevision,
        actual: SourceRevision,
    },

    RevisionUnavailable {
        expected: SourceRevision,
    },

    LineUnavailable {
        requested: u32,
        total_lines: usize,
    },

    Unavailable,
}

enum SourceText {
    Cached(Arc<str>),
    File(String),
}

impl SourceText {
    fn as_str(&self) -> &str {
        match self {
            Self::Cached(source) => source,
            Self::File(source) => source,
        }
    }
}

pub(super) fn resolve_source(
    label: &Label,
    sources: Option<SourceStore<'_>>,
    context_lines: usize,
    filesystem_fallback: bool,
) -> SourceResolution {
    let location = &label.location;

    if let Some(expected) = location.revision {
        match sources.and_then(|source| source.revision(&location.file)) {
            Some(actual) if actual == expected => {}

            Some(actual) => {
                return SourceResolution::Stale { expected, actual };
            }

            None => {
                return SourceResolution::RevisionUnavailable { expected };
            }
        }
    }

    let Some(source) = load_source_text(sources, &location.file, filesystem_fallback) else {
        return SourceResolution::Unavailable;
    };

    let lines = source.as_str().lines().collect::<Vec<_>>();

    let requested_line = location.line.max(1);
    let target = requested_line.saturating_sub(1) as usize;

    if target >= lines.len() {
        return SourceResolution::LineUnavailable {
            requested: location.line,
            total_lines: lines.len(),
        };
    }

    let start = target.saturating_sub(context_lines);
    let end = target
        .saturating_add(context_lines)
        .saturating_add(1)
        .min(lines.len());

    let text = lines[start..end].join("\n");

    SourceResolution::Available(SourceContext {
        language: language_hint(&location.file),
        start_line: start + 1,
        end_line: end,
        target_line: target + 1,
        target_text: lines[target].to_owned(),
        text,
    })
}

fn load_source_text(
    sources: Option<SourceStore<'_>>,
    name: &str,
    filesystem_fallback: bool,
) -> Option<SourceText> {
    if let Some(sources) = sources {
        if let Some(source) = sources.get(name) {
            return Some(SourceText::Cached(source));
        }
    }

    filesystem_fallback
        .then(|| fs::read_to_string(name).ok())
        .flatten()
        .map(SourceText::File)
}

pub(super) fn language_hint(path: &str) -> &'static str {
    let file_name = path.rsplit('/').next().unwrap_or(path);
    let file_name = file_name.rsplit('\\').next().unwrap_or(file_name);
    let lower_name = file_name.to_ascii_lowercase();

    match lower_name.as_str() {
        "dockerfile" => return "dockerfile",
        "makefile" | "gnumakefile" => return "makefile",
        "cmakelists.txt" => return "cmake",
        "justfile" => return "makefile",
        _ => {}
    }

    let extension = lower_name
        .rsplit_once('.')
        .map(|(_, extension)| extension)
        .unwrap_or("");

    match extension {
        "rs" => "rust",

        "py" | "pyw" => "python",

        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "jsx",

        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",

        "json" | "jsonc" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",

        "md" | "markdown" => "markdown",

        "sh" | "bash" => "bash",
        "zsh" => "zsh",
        "fish" => "fish",

        "c" | "h" => "c",
        "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => "cpp",

        "cs" => "csharp",
        "java" => "java",
        "kt" | "kts" => "kotlin",

        "go" => "go",
        "rb" => "ruby",
        "php" => "php",
        "swift" => "swift",
        "lua" => "lua",

        "sql" => "sql",

        "html" | "htm" => "html",
        "css" => "css",
        "scss" => "scss",
        "sass" => "sass",

        "xml" => "xml",

        "vue" => "vue",
        "svelte" => "svelte",

        "nix" => "nix",

        "proto" => "protobuf",
        "graphql" | "gql" => "graphql",

        "ini" | "conf" | "cfg" => "ini",

        "env" => "dotenv",

        "txt" | "log" => "text",

        _ => "text",
    }
}

pub(super) fn caret_line(source_line: &str, column: u32, length: usize, marker: char) -> String {
    let target_index = column.max(1).saturating_sub(1) as usize;

    let mut display_offset = 0;
    let mut highlight_width = 0;

    for (index, ch) in source_line.chars().enumerate() {
        let width = if ch == '\t' {
            TAB_WIDTH - (display_offset % TAB_WIDTH)
        } else {
            UnicodeWidthChar::width(ch).unwrap_or(0)
        };

        if index < target_index {
            display_offset += width;
            continue;
        }

        if index < target_index.saturating_add(length.max(1)) {
            highlight_width += width.max(1);
        }
    }

    if source_line.chars().count() < target_index {
        display_offset = expanded_width(source_line);
    }

    let highlight_width = highlight_width.max(1);

    format!(
        "{}{}",
        " ".repeat(display_offset),
        marker.to_string().repeat(highlight_width),
    )
}

fn expanded_width(source: &str) -> usize {
    let mut width = 0;

    for ch in source.chars() {
        if ch == '\t' {
            width += TAB_WIDTH - (width % TAB_WIDTH);
        } else {
            width += UnicodeWidthChar::width(ch).unwrap_or(0);
        }
    }

    width
}
