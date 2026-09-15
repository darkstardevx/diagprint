use crate::Severity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Style {
    pub open: String,
    pub close: String,
}

impl Style {
    pub fn plain() -> Self {
        Self {
            open: String::new(),
            close: String::new(),
        }
    }

    pub fn ansi(code: impl AsRef<str>) -> Self {
        Self {
            open: format!("\x1b[{}m", code.as_ref()),
            close: "\x1b[0m".into(),
        }
    }

    pub fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self::ansi(format!("38;2;{red};{green};{blue}"))
    }

    pub fn ansi256(index: u8) -> Self {
        Self::ansi(format!("38;5;{index}"))
    }

    pub fn background_rgb(red: u8, green: u8, blue: u8) -> Self {
        Self::ansi(format!("48;2;{red};{green};{blue}"))
    }

    pub fn background_ansi256(index: u8) -> Self {
        Self::ansi(format!("48;5;{index}"))
    }

    pub fn from_hex(value: &str) -> Option<Self> {
        let (red, green, blue) = parse_hex(value)?;
        Some(Self::rgb(red, green, blue))
    }

    pub fn background_from_hex(value: &str) -> Option<Self> {
        let (red, green, blue) = parse_hex(value)?;
        Some(Self::background_rgb(red, green, blue))
    }

    pub fn on_ansi(self, code: impl AsRef<str>) -> Self {
        self.append_code(code.as_ref())
    }

    pub fn on_rgb(self, red: u8, green: u8, blue: u8) -> Self {
        self.append_code(&format!("48;2;{red};{green};{blue}"))
    }

    pub fn on_ansi256(self, index: u8) -> Self {
        self.append_code(&format!("48;5;{index}"))
    }

    pub fn on_hex(self, value: &str) -> Self {
        match parse_hex(value) {
            Some((red, green, blue)) => self.on_rgb(red, green, blue),
            None => self,
        }
    }

    pub fn bold(self) -> Self {
        self.append_code("1")
    }

    pub fn dim(self) -> Self {
        self.append_code("2")
    }

    pub fn italic(self) -> Self {
        self.append_code("3")
    }

    pub fn underline(self) -> Self {
        self.append_code("4")
    }

    fn append_code(mut self, code: &str) -> Self {
        if code.is_empty() {
            return self;
        }

        if self.open.is_empty() {
            self.open = format!("\x1b[{code}m");
            self.close = "\x1b[0m".into();
            return self;
        }

        let mut open = self.open.strip_suffix('m').unwrap_or(&self.open).to_owned();

        open.push(';');
        open.push_str(code);
        open.push('m');

        self.open = open;

        if self.close.is_empty() {
            self.close = "\x1b[0m".into();
        }

        self
    }

    pub(crate) fn paint(&self, enabled: bool, text: &str) -> String {
        if !enabled || self.open.is_empty() {
            return text.to_owned();
        }

        format!("{}{}{}", self.open, text, self.close)
    }
}

impl Default for Style {
    fn default() -> Self {
        Self::plain()
    }
}

fn parse_hex(value: &str) -> Option<(u8, u8, u8)> {
    let value = value.trim().trim_start_matches('#');

    if value.len() != 6 || !value.is_ascii() {
        return None;
    }

    let red = u8::from_str_radix(&value[0..2], 16).ok()?;
    let green = u8::from_str_radix(&value[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&value[4..6], 16).ok()?;

    Some((red, green, blue))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeverityTheme {
    pub trace: Style,
    pub debug: Style,
    pub info: Style,
    pub warning: Style,
    pub error: Style,
    pub fatal: Style,
}

impl SeverityTheme {
    pub fn minimal() -> Self {
        Self {
            trace: Style::plain(),
            debug: Style::plain(),
            info: Style::plain(),
            warning: Style::plain(),
            error: Style::plain(),
            fatal: Style::plain(),
        }
    }

    pub fn style(&self, severity: Severity) -> &Style {
        match severity {
            Severity::Trace => &self.trace,
            Severity::Debug => &self.debug,
            Severity::Info => &self.info,
            Severity::Warning => &self.warning,
            Severity::Error => &self.error,
            Severity::Fatal => &self.fatal,
        }
    }
}

impl Default for SeverityTheme {
    fn default() -> Self {
        Self {
            trace: Style::ansi("90").dim(),
            debug: Style::ansi("36"),
            info: Style::ansi("34"),
            warning: Style::ansi("33").bold(),
            error: Style::ansi("31").bold(),
            fatal: Style::ansi("35").bold().underline(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    pub border: Style,
    pub message: Style,

    pub source_path: Style,
    pub source_gutter: Style,
    pub source_target: Style,
    pub source_caret: Style,
    pub source_label: Style,

    pub cause: Style,
    pub note: Style,
    pub help: Style,

    pub metadata_label: Style,
    pub metadata_value: Style,

    pub severity: SeverityTheme,
}

impl Theme {
    pub fn minimal() -> Self {
        Self {
            border: Style::plain(),
            message: Style::plain(),

            source_path: Style::plain(),
            source_gutter: Style::plain(),
            source_target: Style::plain(),
            source_caret: Style::plain(),
            source_label: Style::plain(),

            cause: Style::plain(),
            note: Style::plain(),
            help: Style::plain(),

            metadata_label: Style::plain(),
            metadata_value: Style::plain(),

            severity: SeverityTheme::minimal(),
        }
    }

    #[cfg(feature = "cybercore")]
    pub fn cybercore() -> Self {
        let schema = cybercore::schema::load();
        Self::from_cybercore_palette(schema.active_theme())
    }

    #[cfg(feature = "cybercore")]
    pub fn cybercore_named(name: &str) -> Option<Self> {
        let schema = cybercore::schema::load();

        schema.theme(name).map(Self::from_cybercore_palette)
    }

    #[cfg(feature = "cybercore")]
    pub fn cybercore_or_default(name: &str) -> Self {
        Self::cybercore_named(name).unwrap_or_else(Self::cybercore)
    }

    #[cfg(feature = "cybercore")]
    pub fn cybercore_theme_names() -> Vec<String> {
        cybercore::schema::load()
            .theme_names()
            .map(str::to_owned)
            .collect()
    }

    #[cfg(feature = "cybercore")]
    pub fn cybercore_active_theme_name() -> String {
        cybercore::schema::load().active.clone()
    }

    #[cfg(feature = "cybercore")]
    pub fn cybercore_theme_exists(name: &str) -> bool {
        cybercore::schema::load().theme(name).is_some()
    }

    #[cfg(feature = "cybercore")]
    fn from_cybercore_palette(palette: &cybercore::schema::Palette) -> Self {
        fn color(hex: &str) -> Style {
            Style::from_hex(hex).unwrap_or_default()
        }

        Self {
            border: color(&palette.line).dim(),
            message: color(&palette.white),

            source_path: color(&palette.cyan).bold(),
            source_gutter: color(&palette.muted).dim(),

            source_target: color(&palette.acid_green).on_hex(&palette.panel).bold(),

            source_caret: color(&palette.hot_pink).bold(),
            source_label: color(&palette.hot_pink),

            cause: color(&palette.red).bold(),
            note: color(&palette.orange).bold(),
            help: color(&palette.acid_green).bold(),

            metadata_label: color(&palette.purple).on_hex(&palette.panel).bold(),

            metadata_value: color(&palette.muted),

            severity: SeverityTheme {
                trace: color(&palette.muted).on_hex(&palette.bg).dim(),

                debug: color(&palette.purple).on_hex(&palette.bg),

                info: color(&palette.cyan).on_hex(&palette.bg).bold(),

                warning: color(&palette.orange).on_hex(&palette.bg).bold(),

                error: color(&palette.red).on_hex(&palette.bg).bold(),

                fatal: color(&palette.hot_pink)
                    .on_hex(&palette.bg)
                    .bold()
                    .underline(),
            },
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            border: Style::ansi("90").dim(),
            message: Style::plain(),

            source_path: Style::ansi("36").bold(),
            source_gutter: Style::ansi("90").dim(),
            source_target: Style::ansi("33").bold(),
            source_caret: Style::ansi("31").bold(),
            source_label: Style::ansi("31"),

            cause: Style::ansi("31").bold(),
            note: Style::ansi("33").bold(),
            help: Style::ansi("36").bold(),

            metadata_label: Style::ansi("90").bold(),
            metadata_value: Style::plain(),

            severity: SeverityTheme::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Style;

    #[test]
    fn parses_hex_rgb() {
        let style = Style::from_hex("14B9B5").expect("valid RGB hex");

        assert_eq!(style.open, "\x1b[38;2;20;185;181m");
        assert_eq!(style.close, "\x1b[0m");
    }

    #[test]
    fn parses_hash_prefixed_hex_rgb() {
        let style = Style::from_hex("#FD3E6A").expect("valid RGB hex");

        assert_eq!(style.open, "\x1b[38;2;253;62;106m");
    }

    #[test]
    fn rejects_invalid_hex() {
        assert!(Style::from_hex("nope").is_none());
        assert!(Style::from_hex("12345").is_none());
        assert!(Style::from_hex("GG0000").is_none());
    }

    #[test]
    fn supports_true_color_backgrounds() {
        let style = Style::rgb(255, 255, 255).on_rgb(14, 9, 29);

        assert_eq!(style.open, "\x1b[38;2;255;255;255;48;2;14;9;29m");
    }

    #[test]
    fn supports_ansi256_backgrounds() {
        let style = Style::ansi256(51).on_ansi256(17);

        assert_eq!(style.open, "\x1b[38;5;51;48;5;17m");
    }

    #[test]
    fn supports_text_modifiers() {
        let style = Style::rgb(20, 185, 181).bold().dim().italic().underline();

        assert_eq!(style.open, "\x1b[38;2;20;185;181;1;2;3;4m");
    }

    #[test]
    fn invalid_background_hex_is_a_noop() {
        let style = Style::rgb(1, 2, 3).on_hex("not-a-color");

        assert_eq!(style.open, "\x1b[38;2;1;2;3m");
    }

    #[cfg(feature = "cybercore")]
    #[test]
    fn loads_active_cybercore_theme() {
        let theme = super::Theme::cybercore();

        assert!(
            theme.border.open.starts_with("\x1b[38;2;"),
            "Cybercore border should resolve to true-color ANSI"
        );

        assert!(
            theme.severity.error.open.contains("48;2;"),
            "Cybercore severity styling should use its background role"
        );

        assert!(
            theme.metadata_label.open.contains("48;2;"),
            "Cybercore metadata styling should use its panel role"
        );
    }

    #[cfg(feature = "cybercore")]
    #[test]
    fn exposes_cybercore_theme_names() {
        let names = super::Theme::cybercore_theme_names();

        assert!(
            !names.is_empty(),
            "Cybercore should expose at least one theme"
        );

        let active = super::Theme::cybercore_active_theme_name();

        assert!(
            names.contains(&active),
            "active Cybercore theme should appear in theme list"
        );
    }

    #[cfg(feature = "cybercore")]
    #[test]
    fn cybercore_theme_names_are_sorted() {
        let names = super::Theme::cybercore_theme_names();

        let mut sorted = names.clone();
        sorted.sort();

        assert_eq!(
            names, sorted,
            "Cybercore theme names should have stable sorted ordering"
        );
    }

    #[cfg(feature = "cybercore")]
    #[test]
    fn cybercore_theme_exists_reports_known_and_unknown_names() {
        let active = super::Theme::cybercore_active_theme_name();

        assert!(super::Theme::cybercore_theme_exists(&active));

        assert!(!super::Theme::cybercore_theme_exists(
            "__diagprint_theme_that_does_not_exist__"
        ));
    }

    #[cfg(feature = "cybercore")]
    #[test]
    fn cybercore_or_default_falls_back_to_active_theme() {
        let fallback =
            super::Theme::cybercore_or_default("__diagprint_theme_that_does_not_exist__");

        assert_eq!(fallback, super::Theme::cybercore());
    }

    #[cfg(feature = "cybercore")]
    #[test]
    fn unknown_cybercore_theme_returns_none() {
        assert!(super::Theme::cybercore_named("__diagprint_theme_that_does_not_exist__").is_none());
    }
}
