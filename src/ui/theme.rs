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
