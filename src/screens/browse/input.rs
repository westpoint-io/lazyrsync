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
        let [rail, right] =
            Layout::horizontal([Constraint::Length(34), Constraint::Min(30)]).areas(body);
        let rails = Layout::vertical(rail_constraints()).split(rail);
        let [main, log] =
            Layout::vertical([Constraint::Min(6), Constraint::Length(8)]).areas(right);
        ([rails[0], rails[1], rails[2], rails[3]], main, log)
    }

    fn click_subtab(&self, cx: &mut Ctx, area: Rect, col: u16) -> bool {
        let mut x = area.x + 1 + 4;
        for (j, name) in SUBTABS.iter().enumerate() {
            let w = name.chars().count() as u16;
            if col >= x && col < x + w {
                cx.subtab = j;
                return true;
            }
            x += w + 3;
        }
        false
    }

    fn click_task_list(&mut self, cx: &mut Ctx, area: Rect, row: u16) {
        let visible = (area.height as usize).saturating_sub(2);
        let vis = self.visible_rows(cx);
        let (cur, _) = self.panel_count(cx, 0);
        let offset = (cur + 1).saturating_sub(visible);
        let pos = row.saturating_sub(area.y + 1) as usize + offset;
        if let Some(&real) = vis.get(pos) {
            self.set_list_cursor(cx, real);
        }
    }

    fn click_flag(&mut self, area: Rect, row: u16, col: u16) {
        if row <= area.y {
            return;
        }
        let rr = (row - area.y - 1) as usize;
        let inner_w = (area.width as usize).saturating_sub(2);
        let colw = (inner_w / 2).max(15);
        let right = (col as usize) >= (area.x as usize + 1 + colw);
        let n = rr * 2 + usize::from(right);
        if n < editor::bool_flag_count() {
            self.flag = n;
        }
    }

    fn activate_profile(&mut self, cx: &mut Ctx) {
        if self.runs.running() && cx.pcursor != cx.profile {
            cx.push_log(
                LogKind::Warn,
                "can't switch profiles while a run is in progress",
            );
            cx.reject();
            return;
        }
        if cx.pcursor < cx.store.profiles.len() {
            let p = cx.store.profiles.remove(cx.pcursor);
            cx.store.profiles.insert(0, p);
            cx.store.sort_profiles_by_recency();
            let _ = cx.store.save();
        }
        cx.profile = 0;
        cx.pcursor = 0;
        cx.task = 0;
        self.tcursor = 0;
        self.flag = 0;
        self.filter = 0;
        self.visual = None;
    }

    fn select_task(&mut self, cx: &mut Ctx) {
        if cx.subtab != 0 || self.focus != 0 {
            return;
        }
        self.visual = None;
        if self.tcursor == 0 {
            return;
        }
        if let Some(p) = cx.store.profiles.get_mut(cx.profile) {
            if self.tcursor < p.tasks.len() {
                let t = p.tasks.remove(self.tcursor);
                p.tasks.insert(0, t);
                p.sort_tasks_by_recency();
            }
        }
        self.tcursor = 0;
        cx.task = 0;
        let _ = cx.store.save();
    }

    fn add(&self, cx: &mut Ctx) -> Cmd {
        if cx.subtab == 1 {
            Cmd::Overlay(Overlay::Prompt(Prompt::new_profile(String::new())))
        } else if cx.active_profile().is_some() {
            Cmd::Overlay(Overlay::AddTask(AddTask::new()))
        } else {
            cx.push_log(LogKind::Warn, "add a profile first");
            Cmd::None
        }
    }

    fn delete(&mut self, cx: &mut Ctx) -> Cmd {
        if cx.subtab == 1 {
            let Some(p) = cx.store.profiles.get(cx.pcursor) else {
                return Cmd::None;
            };
            return Cmd::Overlay(Overlay::ConfirmDelete(ConfirmDelete::profile(
                p.name.clone(),
            )));
        }
        let ids: Vec<String> = if let Some((lo, hi)) = self.visual_range() {
            match cx.active_profile() {
                Some(p) => (lo..=hi)
                    .filter_map(|i| p.tasks.get(i).map(|t| t.id.clone()))
                    .collect(),
                None => return Cmd::None,
            }
        } else {
            self.select_task(cx);
            match cx.active_task() {
                Some(t) => vec![t.id.clone()],
                None => return Cmd::None,
            }
        };
        if ids.is_empty() {
            return Cmd::None;
        }
        Cmd::Overlay(Overlay::ConfirmDelete(ConfirmDelete::tasks(ids)))
    }

    fn clear_filters(&mut self, cx: &mut Ctx) -> Cmd {
        match cx.active_task() {
            Some(t) if t.filters.is_empty() => Cmd::None,
            Some(t) => Cmd::Overlay(Overlay::ConfirmClearFilters(ConfirmClearFilters::new(
                t.id.clone(),
            ))),
            None => Cmd::None,
        }
    }

    fn open_section(&mut self, cx: &mut Ctx, section: editor::Section) -> Cmd {
        if cx.subtab != 0 {
            return Cmd::None;
        }
        self.select_task(cx);
        match cx.active_task().cloned() {
            Some(t) => {
                let taken = cx
                    .active_profile()
                    .map(|p| {
                        p.tasks
                            .iter()
                            .filter(|o| o.id != t.id)
                            .map(|o| o.id.clone())
                            .collect()
                    })
                    .unwrap_or_default();
                Cmd::Overlay(Overlay::Edit(Box::new(SectionEdit::new(t, section, taken))))
            }
            None => Cmd::None,
        }
    }

    fn run_task(&mut self, cx: &mut Ctx) -> Cmd {
        if let Some((lo, hi)) = self.visual_range() {
            let Some(p) = cx.active_profile() else {
                return Cmd::None;
            };
            let all: Vec<Task> = (lo..=hi).filter_map(|i| p.tasks.get(i).cloned()).collect();
            return Self::request_run(all);
        }
        self.select_task(cx);
        match cx.active_task().cloned() {
            Some(t) => Self::request_run(vec![t]),
            None => {
                cx.push_log(LogKind::Warn, "no task to run");
                Cmd::None
            }
        }
    }

    fn request_run(batch: Vec<Task>) -> Cmd {
        match batch.len() {
            0 => Cmd::None,
            1 => Cmd::RequestRun(batch),
            _ => Cmd::Overlay(Overlay::ConfirmRun(ConfirmRun::new(batch))),
        }
    }

    fn run_all(&self, cx: &mut Ctx) -> Cmd {
        let Some(p) = cx.active_profile() else {
            return Cmd::None;
        };
        if p.tasks.is_empty() {
            cx.push_log(LogKind::Warn, "profile has no tasks");
            return Cmd::None;
        }
        Self::request_run(p.tasks.clone())
    }

    fn start_preview(&mut self, cx: &mut Ctx) {
        self.select_task(cx);
        let Some(t) = cx.active_task().cloned() else {
            cx.push_log(LogKind::Warn, "no task to preview");
            return;
        };
        self.runs.preview(t, cx);
        self.focus = 3;
    }
}
