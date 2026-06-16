use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Clone, Copy)]
pub(crate) struct Theme {
    pub accent: Color,
    pub on_accent: Color,
    pub secondary: Color,
    pub border: Color,
    pub muted: Color,
    pub added: Color,
    pub modified: Color,
    pub deleted: Color,
    pub warn: Color,
    pub bytes: Color,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ThemeSpec {
    pub accent: String,
    pub on_accent: String,
    pub secondary: String,
    pub border: String,
    pub muted: String,
    pub added: String,
    pub modified: String,
    pub deleted: String,
    pub warn: String,
    pub bytes: String,
}

impl Default for ThemeSpec {
    fn default() -> Self {
        Self {
            accent: "lightblue".into(),
            on_accent: "black".into(),
            secondary: "lightcyan".into(),
            border: "gray".into(),
            muted: "darkgray".into(),
            added: "green".into(),
            modified: "yellow".into(),
            deleted: "red".into(),
            warn: "yellow".into(),
            bytes: "blue".into(),
        }
    }
}

static THEME: OnceLock<Theme> = OnceLock::new();

fn cur() -> &'static Theme {
    THEME.get_or_init(|| Theme::from_spec(&ThemeSpec::default()))
}

pub(crate) fn apply(spec: &ThemeSpec) {
    let _ = THEME.set(Theme::from_spec(spec));
}

impl Theme {
    fn from_spec(s: &ThemeSpec) -> Theme {
        let d = ThemeSpec::default();
        let c = |val: &str, fallback: &str| {
            parse_color(val)
                .or_else(|| parse_color(fallback))
                .unwrap_or(Color::Reset)
        };
        Theme {
            accent: c(&s.accent, &d.accent),
            on_accent: c(&s.on_accent, &d.on_accent),
            secondary: c(&s.secondary, &d.secondary),
            border: c(&s.border, &d.border),
            muted: c(&s.muted, &d.muted),
            added: c(&s.added, &d.added),
            modified: c(&s.modified, &d.modified),
            deleted: c(&s.deleted, &d.deleted),
            warn: c(&s.warn, &d.warn),
            bytes: c(&s.bytes, &d.bytes),
        }
    }
}

fn parse_color(s: &str) -> Option<Color> {
    let k: String = s
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|c| !matches!(c, ' ' | '-' | '_'))
        .collect();
    if let Some(hex) = k.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some(Color::Rgb(r, g, b));
        }
        return None;
    }
    Some(match k.as_str() {
        "reset" => Color::Reset,
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "gray" | "grey" => Color::Gray,
        "darkgray" | "darkgrey" => Color::DarkGray,
        "lightred" => Color::LightRed,
        "lightgreen" => Color::LightGreen,
        "lightyellow" => Color::LightYellow,
        "lightblue" => Color::LightBlue,
        "lightmagenta" => Color::LightMagenta,
        "lightcyan" => Color::LightCyan,
        "white" => Color::White,
        _ => return None,
    })
}

pub(crate) fn accent() -> Color {
    cur().accent
}
