use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    style::{Style, Stylize},
    text::{Line, Text},
    widgets::{Block, BorderType, Paragraph},
    Frame,
};

use crate::app::{Cmd, Ctx};
use crate::profile::Filters;
use crate::ui::{deleted, hint_line, truncate, with_footer};

pub struct ConfirmClearFilters {
    task: String,
}

impl ConfirmClearFilters {
    pub fn new(task: String) -> Self {
        Self { task }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, cx: &Ctx) {
        let footer = if cx.settings.hints {
            vec![hint_line(&[("<enter>", "Clear"), ("<esc>", "Cancel")])]
        } else {
            vec![]
        };
        let area = with_footer(frame, cx, area, 60, 8, footer);
        let name = truncate(&self.task, 48);
        let text = Text::from(vec![
            Line::from(""),
            Line::from(vec![
                "Clear all filters on ".into(),
                name.fg(deleted()).bold(),
                " ?".into(),
            ]),
            Line::from(""),
            Line::from("Removes every include/exclude/filter rule.".dim()),
        ]);
        frame.render_widget(
            Paragraph::new(text).centered().block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .border_style(Style::new().fg(deleted()).bold())
                    .title(" Clear filters ".fg(deleted()).bold()),
            ),
            area,
        );
    }

    pub fn on_key(&mut self, key: KeyEvent, cx: &mut Ctx) -> Cmd {
        match key.code {
            KeyCode::Char('y') | KeyCode::Enter => {
                if let Some(p) = cx.store.profiles.get_mut(cx.profile) {
                    if let Some(t) = p.tasks.get_mut(cx.task) {
                        t.filters = Filters::default();
                    }
                }
                cx.save("cleared filters");
                Cmd::Close
            }
            KeyCode::Char('n') | KeyCode::Char('q') | KeyCode::Esc => Cmd::Close,
            _ => Cmd::None,
        }
    }
}
