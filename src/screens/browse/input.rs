use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Layout, Position, Rect};

use super::{Browse, SUBTABS};
use crate::app::{Cmd, Ctx, LogKind, Overlay};
use crate::editor;
use crate::popups::{
    AddTask, Alert, ConfirmClearFilters, ConfirmDelete, ConfirmRun, Help, Prompt, SectionEdit,
};
use crate::profile::Task;
use crate::ui::rail_constraints;

impl Browse {
    pub(super) fn handle_key(&mut self, key: KeyEvent, cx: &mut Ctx) -> Cmd {
        if self.focus == 0 && self.is_filtering() {
            self.handle_filter_key(cx, key);
            return Cmd::None;
        }
        if self.focus == 3 && self.runs.searching() {
            let v = self.main_visible(cx);
            self.runs.search_key(key, v);
            return Cmd::None;
        }
        match key.code {
            KeyCode::Char('c') if self.runs.running() => {
                self.runs.cancel();
                Cmd::None
            }
            KeyCode::Esc if self.visual.is_some() => {
                self.visual = None;
                Cmd::None
            }
            KeyCode::Esc if self.focus == 3 && self.runs.search_on() => {
                self.runs.exit_search();
                Cmd::None
            }
            KeyCode::Esc if self.focus == 0 && self.filter_active() => {
                self.clear_filter(cx);
                Cmd::None
            }
            KeyCode::Char('n') if self.focus == 3 && self.runs.search_on() => {
                let v = self.main_visible(cx);
                self.runs.next_match(v, 1);
                Cmd::None
            }
            KeyCode::Char('N') if self.focus == 3 && self.runs.search_on() => {
                let v = self.main_visible(cx);
                self.runs.next_match(v, -1);
                Cmd::None
            }
            KeyCode::Char('q') | KeyCode::Esc => Cmd::Quit,
            KeyCode::Char('/') if self.focus == 0 => {
                self.filtering = true;
                self.list_filter.clear();
                Cmd::None
            }
            KeyCode::Char('/') if self.focus == 3 && !self.runs.running() => {
                self.runs.start_search();
                Cmd::None
            }
            KeyCode::Tab => {
                self.focus = Self::focus_at(self.rail_pos() + 1);
                Cmd::None
            }
            KeyCode::BackTab => {
                self.focus = Self::focus_at(self.rail_pos() + 3);
                Cmd::None
            }
            KeyCode::Char(c @ '1'..='4') => {
                self.focus = Self::focus_at(c as usize - '1' as usize);
                Cmd::None
            }
            KeyCode::Char('[') | KeyCode::Char(']') if self.focus == 0 => {
                cx.subtab = (cx.subtab + 1) % 2;
                if cx.subtab == 1 {
                    cx.pcursor = cx.profile;
                } else {
                    self.tcursor = cx.task;
                }
                self.visual = None;
                self.snap_cursor(cx);
                Cmd::None
            }
            KeyCode::Char(' ') | KeyCode::Enter if self.focus == 0 && cx.subtab == 1 => {
                self.activate_profile(cx);
                Cmd::None
            }
            KeyCode::Char(' ') | KeyCode::Enter if self.focus == 0 && cx.subtab == 0 => {
                self.select_task(cx);
                Cmd::None
            }
            KeyCode::Char('V') if self.focus == 0 && cx.subtab == 0 => {
                if self.visual.is_some() {
                    self.visual = None;
                } else {
                    self.visual = Some(self.tcursor);
                    self.visual_len = cx.active_profile().map_or(0, |p| p.tasks.len());
                }
                Cmd::None
            }
            KeyCode::Char(' ') if self.focus == 1 => {
                let enabling_delete = self.flag == editor::delete_flag_index()
                    && cx.active_task().is_some_and(|t| !t.flags.delete);
                if enabling_delete && !cx.settings.skip_delete_warning {
                    return Cmd::Overlay(Overlay::Alert(Alert::enable_delete()));
                }
                if let Some(p) = cx.store.profiles.get_mut(cx.profile) {
                    if let Some(t) = p.tasks.get_mut(cx.task) {
                        editor::toggle_bool_flag(&mut t.flags, self.flag);
                    }
                }
                cx.save("toggled flag");
                Cmd::None
            }
            KeyCode::PageDown => {
                self.scroll_preview(10);
                Cmd::None
            }
            KeyCode::PageUp => {
                self.scroll_preview(-10);
                Cmd::None
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.scroll_preview(10);
                Cmd::None
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.scroll_preview(-10);
                Cmd::None
            }
            KeyCode::Char('J') => {
                self.scroll_preview(10);
                Cmd::None
            }
            KeyCode::Char('K') => {
                self.scroll_preview(-10);
                Cmd::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_sel(cx, 1);
                Cmd::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_sel(cx, -1);
                Cmd::None
            }
            KeyCode::Right | KeyCode::Char('l') if self.focus == 3 => {
                self.runs.select(1);
                Cmd::None
            }
            KeyCode::Left | KeyCode::Char('h') if self.focus == 3 => {
                self.runs.select(-1);
                Cmd::None
            }
            KeyCode::Right | KeyCode::Char('l') if self.focus == 1 => {
                self.move_flag(0, 1);
                Cmd::None
            }
            KeyCode::Left | KeyCode::Char('h') if self.focus == 1 => {
                self.move_flag(0, -1);
                Cmd::None
            }
            KeyCode::Char('r')
                if key.modifiers.contains(KeyModifiers::CONTROL) && self.focus == 2 =>
            {
                self.clear_filters(cx)
            }
            KeyCode::Char('a') if self.focus == 0 => self.add(cx),
            KeyCode::Char('r') if self.focus == 0 && cx.subtab == 1 => {
                match cx.store.profiles.get(cx.pcursor) {
                    Some(p) => {
                        Cmd::Overlay(Overlay::Prompt(Prompt::rename_profile(p.name.clone())))
                    }
                    None => Cmd::None,
                }
            }
            KeyCode::Char('r') => self.run_task(cx),
            KeyCode::Char('d') if self.focus == 0 => self.delete(cx),
            KeyCode::Char('e') if self.focus == 0 && cx.subtab == 0 => {
                self.open_section(cx, editor::Section::Basics)
            }
            KeyCode::Char('s') if self.focus == 0 && cx.subtab == 0 => {
                self.open_section(cx, editor::Section::Ssh)
            }
            KeyCode::Char('i') if self.focus == 2 => {
                self.open_section(cx, editor::Section::Filters)
            }
            KeyCode::Char('x') if self.focus == 1 => {
                self.open_section(cx, editor::Section::Advanced)
            }
            KeyCode::Char('p') => {
                self.start_preview(cx);
                Cmd::None
            }
            KeyCode::Char('?') => Cmd::Overlay(Overlay::Help(Help::new(cx.subtab == 1))),
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.zoom = !self.zoom;
                Cmd::None
            }
            KeyCode::Char('R') => self.run_all(cx),
            _ => Cmd::None,
        }
    }

    pub(super) fn handle_mouse(&mut self, m: MouseEvent, cx: &mut Ctx) -> Cmd {
        let (rail, main, log) = self.geometry(cx);
        let at = Position::new(m.column, m.row);
        match m.kind {
            MouseEventKind::Up(MouseButton::Left) => self.scroll_drag = false,
            MouseEventKind::Drag(MouseButton::Left) => {
                if self.scroll_drag && self.focus == 3 {
                    self.scroll_to_row(main, m.row);
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let track = m.column + 1 >= main.x + main.width && main.contains(at);
                if self.focus == 3 && track && main.height > 2 {
                    self.scroll_drag = true;
                    self.scroll_to_row(main, m.row);
                } else if rail[0].contains(at) {
                    self.focus = 3;
                } else if rail[1].contains(at) {
                    self.focus = 0;
                    if m.row == rail[1].y {
                        self.click_subtab(cx, rail[1], m.column);
                    } else {
                        self.click_task_list(cx, rail[1], m.row);
                    }
                } else if rail[2].contains(at) {
                    self.focus = 1;
                    self.click_flag(rail[2], m.row, m.column);
                } else if rail[3].contains(at) {
                    self.focus = 2;
                }
            }
            MouseEventKind::ScrollDown => {
                if rail[0].contains(at) {
                    self.runs.select(1);
                } else if rail[1].contains(at) {
                    self.move_sel(cx, 1);
                } else if main.contains(at) {
                    self.runs.scroll(3);
                } else if log.contains(at) {
                    self.log_scroll = self.log_scroll.saturating_sub(3);
                }
            }
            MouseEventKind::ScrollUp => {
                if rail[0].contains(at) {
                    self.runs.select(-1);
                } else if rail[1].contains(at) {
                    self.move_sel(cx, -1);
                } else if main.contains(at) {
                    self.runs.scroll(-3);
                } else if log.contains(at) {
                    self.log_scroll = (self.log_scroll + 3).min(self.log_max_scroll);
                }
            }
            _ => {}
        }
        Cmd::None
    }

    fn handle_filter_key(&mut self, cx: &mut Ctx, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.clear_filter(cx),
            KeyCode::Enter => self.filtering = false,
            KeyCode::Backspace => {
                self.list_filter.pop();
                self.snap_cursor(cx);
            }
            KeyCode::Char(c) => {
                self.list_filter.push(c);
                self.snap_cursor(cx);
            }
            _ => {}
        }
    }

    fn clear_filter(&mut self, cx: &mut Ctx) {
        self.filtering = false;
        self.list_filter.clear();
        self.snap_cursor(cx);
    }

    fn main_visible(&self, cx: &Ctx) -> usize {
        (self.geometry(cx).1.height as usize).saturating_sub(2)
    }

    fn scroll_to_row(&mut self, main: Rect, row: u16) {
        if main.height <= 2 {
            return;
        }
        let visible = (main.height as usize).saturating_sub(2);
        let top = main.y + 1;
        let bottom = main.y + main.height - 2;
        let rel = row.clamp(top, bottom).saturating_sub(top) as f64;
        let frac = rel / (main.height - 2) as f64;
        self.runs.click_scroll(frac, visible);
    }

    fn geometry(&self, cx: &Ctx) -> ([Rect; 4], Rect, Rect) {
        let [body, _status] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(cx.area);
        if self.zoom {
            return ([Rect::new(0, 0, 0, 0); 4], body, Rect::new(0, 0, 0, 0));
        }
