use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    style::{Style, Stylize},
    text::{Line, Text},
    widgets::{Block, BorderType, Paragraph},
    Frame,
};

use crate::app::{Cmd, Ctx};
use crate::ui::{deleted, hint_line, with_footer};

pub enum AlertAction {
    EnableFlag {
        field_index: usize,
        label: &'static str,
    },
}

pub struct Alert {
    action: AlertAction,
}

impl Alert {
    pub fn enable_flag(field_index: usize, label: &'static str) -> Self {
        Self {
            action: AlertAction::EnableFlag { field_index, label },
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, cx: &Ctx) {
        let footer = if cx.settings.hints {
            vec![hint_line(&[("<enter>", "Enable"), ("<esc>", "Cancel")])]
        } else {
            vec![]
        };
        let area = with_footer(frame, cx, area, 60, 8, footer);
        let AlertAction::EnableFlag { label, .. } = self.action;
        let text = Text::from(vec![
            Line::from(""),
            Line::from(vec![
                "Enable ".into(),
                label.fg(deleted()).bold(),
                " ?".into(),
            ]),
            Line::from(""),
            Line::from("It can remove files on the destination".dim()),
            Line::from("that no longer exist in the source.".dim()),
        ]);
        frame.render_widget(
            Paragraph::new(text).centered().block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .border_style(Style::new().fg(deleted()).bold())
                    .title(" Alert ".fg(deleted()).bold()),
            ),
            area,
        );
    }

    pub fn on_key(&mut self, key: KeyEvent, cx: &mut Ctx) -> Cmd {
        match key.code {
            KeyCode::Char('y') | KeyCode::Enter => {
                match self.action {
                    AlertAction::EnableFlag { field_index, label } => {
                        if let Some(t) = cx.active_task_mut() {
                            {
                                crate::editor::toggle_bool_flag(&mut t.flags, field_index);
                            }
                        }
                        cx.save(&format!("enabled {label}"));
                    }
                }
                Cmd::Close
            }
            KeyCode::Char('n') | KeyCode::Char('q') | KeyCode::Esc => Cmd::Close,
            _ => Cmd::None,
        }
    }
}
