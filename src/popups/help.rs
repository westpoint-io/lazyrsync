use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Clear, Paragraph, Wrap},
    Frame,
};

use crate::app::Cmd;
use crate::ui::{accent, centered, on_accent, rounded, secondary};

struct Binding {
    key: &'static str,
    desc: &'static str,
    long: &'static str,
}

const LOCAL_TASKS: &[Binding] = &[
    Binding {
        key: "r",
        desc: "run task",
        long: "Run the selected task (*), or a V visual range in sequence.",
    },
    Binding {
        key: "R",
        desc: "run all",
        long: "Run every task in the active profile, one after another. Tasks with a pre-run error (missing source, same source/dest) are skipped, not blocking the rest.",
    },
    Binding {
        key: "p",
        desc: "dry-run preview",
        long: "Preview the transfer as a diff without changing anything.",
    },
    Binding {
        key: "<space>",
        desc: "select / toggle",
        long: "Select the task under the cursor (*), or toggle a flag in the Flags panel.",
    },
    Binding {
        key: "V",
        desc: "visual select",
        long: "Start a range selection; j/k extends the block, r runs them all, d deletes them all, <esc> dismisses.",
    },
    Binding {
        key: "e",
        desc: "edit paths",
        long: "On the Tasks panel: edit source and destination (either can be a user@host:/path remote).",
    },
    Binding {
        key: "s",
        desc: "edit ssh",
        long: "On the Tasks panel: edit SSH port, key file, and extra options.",
    },
    Binding {
        key: "i",
        desc: "edit filters",
        long: "On the Filters panel (4): edit include / exclude patterns and the *-from files.",
    },
    Binding {
        key: "<ctrl+r>",
        desc: "clear filters",
        long: "On the Filters panel (4): clear every filter rule on the active task (asks to confirm).",
    },
    Binding {
        key: "x",
        desc: "edit advanced",
        long: "On the Flags panel (3): edit the bandwidth limit and raw extra arguments.",
    },
    Binding {
        key: "a",
        desc: "add task",
        long: "On the Tasks/Profiles panel: add a new task (or profile).",
    },
    Binding {
        key: "d",
        desc: "delete task",
        long: "On the Tasks/Profiles panel: delete the selected task or profile (asks to confirm). With a V range active, deletes all selected tasks at once.",
    },
    Binding {
        key: "c",
        desc: "cancel scan",
        long: "Cancel a dry-run that is currently running.",
    },
];

const LOCAL_PROFILES: &[Binding] = &[
    Binding {
        key: "<enter>",
        desc: "switch to profile",
        long: "Make the highlighted profile active (marked *). j/k only move the cursor.",
    },
    Binding {
        key: "a",
        desc: "add profile",
        long: "Create a new profile.",
    },
    Binding {
        key: "r",
        desc: "rename profile",
        long: "Rename the selected profile.",
    },
    Binding {
        key: "d",
        desc: "delete profile",
        long: "Delete the selected profile and its tasks (asks to confirm).",
    },
];

const GLOBAL: &[Binding] = &[
    Binding {
        key: "<tab>",
        desc: "switch panel",
        long: "Move focus between Status, Flags, and Filters (also 1-3).",
    },
    Binding {
        key: "]",
        desc: "next tab",
        long: "Switch the left list to the next tab (Tasks / Profiles).",
    },
    Binding {
        key: "[",
        desc: "previous tab",
        long: "Switch the left list to the previous tab (Tasks / Profiles).",
    },
    Binding {
        key: "/",
        desc: "search / filter",
        long: "On the Tasks/Profiles list: filter rows by name as you type (<esc> clears). On the Runs pane: search the output — matches highlight, n/N jump between them, <esc> exits.",
    },
    Binding {
        key: "<pgup>",
        desc: "scroll up",
        long: "Scroll the dry-run preview up (K and the wheel also work).",
    },
    Binding {
        key: "<pgdown>",
        desc: "scroll down",
        long: "Scroll the dry-run preview down (J and the wheel also work).",
    },
    Binding {
        key: "+",
        desc: "zoom",
        long: "Toggle full-width zoom of the Status panel.",
    },
    Binding {
        key: "<ctrl+g>",
        desc: "toggle hints",
        long: "Show or hide the keybinding hints shown in popups.",
    },
    Binding {
        key: "?",
        desc: "help",
        long: "Toggle this keybindings list.",
    },
];

pub struct Help {
    sel: usize,
    profiles: bool,
}

impl Help {
    pub fn new(profiles: bool) -> Self {
        Self { sel: 0, profiles }
    }

    fn local(&self) -> &'static [Binding] {
        if self.profiles {
            LOCAL_PROFILES
        } else {
            LOCAL_TASKS
        }
    }

    fn total(&self) -> usize {
        self.local().len() + GLOBAL.len()
    }

    fn binding(&self, i: usize) -> &'static Binding {
        let local = self.local();
        if i < local.len() {
            &local[i]
        } else {
            &GLOBAL[i - local.len()]
        }
    }

    fn key_width(&self) -> usize {
        self.local()
            .iter()
            .chain(GLOBAL)
            .map(|b| b.key.chars().count())
            .max()
            .unwrap_or(1)
    }

    pub fn on_key(&mut self, key: KeyEvent) -> Cmd {
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.sel = (self.sel + 1) % self.total();
                Cmd::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.sel = (self.sel + self.total() - 1) % self.total();
                Cmd::None
            }
            _ => Cmd::Close,
        }
    }

    pub fn on_mouse(&mut self, m: MouseEvent, _cx: &mut crate::app::Ctx) -> Cmd {
        match m.kind {
            MouseEventKind::ScrollDown => {
                self.sel = (self.sel + 1) % self.total();
                Cmd::None
            }
            MouseEventKind::ScrollUp => {
                self.sel = (self.sel + self.total() - 1) % self.total();
                Cmd::None
            }
            _ => Cmd::None,
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let width = 60u16;
        let iw = (width - 2) as usize;

        let kw = self.key_width();
        let mut lines: Vec<Line> = Vec::new();
        let mut sel_line = 0usize;
        let header = |t: &str| Line::from(format!("--- {t} ---").fg(secondary()).bold()).centered();

        lines.push(header("Local"));
        for (i, b) in self.local().iter().enumerate() {
            if i == self.sel {
                sel_line = lines.len();
            }
            lines.push(row(b, i == self.sel, iw, kw));
        }
        lines.push(Line::from(""));
        lines.push(header("Global"));
        for (j, b) in GLOBAL.iter().enumerate() {
            let i = self.local().len() + j;
            if i == self.sel {
                sel_line = lines.len();
            }
            lines.push(row(b, i == self.sel, iw, kw));
        }

        let footer_h = 3u16;
        let max_menu_inner = (area.height as usize).saturating_sub(2 + footer_h as usize + 2);
        let menu_inner = lines.len().min(max_menu_inner.max(1));
        let menu_h = menu_inner as u16 + 2;
        let region = centered(area, width, menu_h + footer_h);
        let [menu_a, footer_a] =
            Layout::vertical([Constraint::Length(menu_h), Constraint::Length(footer_h)])
                .areas(region);

        let offset = sel_line
            .saturating_sub(menu_inner.saturating_sub(1))
            .min(lines.len().saturating_sub(menu_inner)) as u16;

        frame.render_widget(Clear, menu_a);
        frame.render_widget(
            Paragraph::new(lines).scroll((offset, 0)).block(
                rounded(true)
                    .title(" Keybindings ".fg(accent()).bold())
                    .title_bottom(
                        Line::from(format!("{} of {} ", self.sel + 1, self.total()).dim())
                            .right_aligned(),
                    ),
            ),
            menu_a,
        );

        frame.render_widget(Clear, footer_a);
        frame.render_widget(
            Paragraph::new(Text::from(Line::from(format!(
                " {}",
                self.binding(self.sel).long
            ))))
            .wrap(Wrap { trim: false })
            .block(rounded(false)),
            footer_a,
        );
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn row(b: &Binding, selected: bool, w: usize, kw: usize) -> Line<'static> {
    let desc = capitalize(b.desc);
    if selected {
        let mut s = format!("{:>kw$} {desc}", b.key);
        let pad = w.saturating_sub(s.chars().count());
        s.push_str(&" ".repeat(pad));
        Line::from(s).style(Style::new().fg(on_accent()).bg(accent()).bold())
    } else {
        Line::from(vec![
            Span::styled(format!("{:>kw$} ", b.key), Style::new().fg(accent())),
            Span::raw(desc),
        ])
    }
}
