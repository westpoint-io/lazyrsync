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
