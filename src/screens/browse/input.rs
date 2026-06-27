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
