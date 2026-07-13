use std::time::{Duration, Instant};

use chrono::Local;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent};
use ratatui::{layout::Rect, DefaultTerminal, Frame};

use crate::profile::{Profile, Task};
use crate::store::{Settings, Store};

use crate::popups::{
    AddTask, Alert, ConfirmClearFilters, ConfirmDelete, ConfirmRun, Help, Prompt, SectionEdit,
};
use crate::screens::browse::Browse;

#[derive(Clone, Copy)]
pub enum LogKind {
    Info,
    Active,
    Done,
    Warn,
    Error,
    Command,
}

pub struct LogEntry {
    pub(crate) at: String,
    pub(crate) kind: LogKind,
    pub(crate) text: String,
}

pub struct Ctx {
    pub store: Store,
    pub settings: Settings,
    pub log: Vec<LogEntry>,
    pub area: Rect,
    pub tick: usize,
    pub shake: u8,
    pub profile: usize,
    pub pcursor: usize,
    pub task: usize,
    pub subtab: usize,
}

impl Ctx {
    pub fn active_profile(&self) -> Option<&Profile> {
        self.store.profiles.get(self.profile)
    }

    pub fn active_task(&self) -> Option<&Task> {
        self.active_profile().and_then(|p| p.tasks.get(self.task))
    }

    pub fn clamp(&mut self) {
        let np = self.store.profiles.len();
        if np == 0 {
            self.profile = 0;
            self.pcursor = 0;
            self.task = 0;
            return;
        }
        self.profile = self.profile.min(np - 1);
        self.pcursor = self.pcursor.min(np - 1);
        let nt = self.store.profiles[self.profile].tasks.len();
        self.task = self.task.min(nt.saturating_sub(1));
    }

    pub fn push_log(&mut self, kind: LogKind, text: impl Into<String>) {
        let at = Local::now().format("%H:%M:%S").to_string();
        self.log.push(LogEntry {
            at,
            kind,
            text: text.into(),
        });
        let n = self.log.len();
        if n > 200 {
            self.log.drain(0..n - 200);
        }
    }

    pub fn reject(&mut self) {
        self.shake = 6;
    }

    pub fn shake_dx(&self) -> i16 {
        const WAVE: [i16; 7] = [0, 1, -1, 2, -2, 3, -3];
        WAVE[(self.shake as usize).min(WAVE.len() - 1)]
    }

    pub fn save(&mut self, ok_msg: &str) {
        match self.store.save() {
            Ok(()) => self.push_log(LogKind::Info, ok_msg.to_string()),
            Err(e) => self.push_log(LogKind::Error, format!("save failed: {e}")),
        }
    }
}

pub enum Cmd {
    None,
    Quit,
    Overlay(Overlay),
    Close,
    RequestRun(Vec<Task>),
    StartRun(Vec<Task>),
}

pub trait Component {
    fn draw(&mut self, frame: &mut Frame, area: Rect, cx: &Ctx);
    fn on_key(&mut self, key: KeyEvent, cx: &mut Ctx) -> Cmd;
    fn on_mouse(&mut self, _m: MouseEvent, _cx: &mut Ctx) -> Cmd {
        Cmd::None
    }
    fn busy(&self) -> bool {
        false
    }
    fn tick(&mut self, _cx: &mut Ctx) {}
}

pub enum Overlay {
    Prompt(Prompt),
    AddTask(AddTask),
    Edit(Box<SectionEdit>),
    ConfirmDelete(ConfirmDelete),
    ConfirmClearFilters(ConfirmClearFilters),
    ConfirmRun(ConfirmRun),
    Alert(Alert),
    Help(Help),
}

impl Overlay {
    fn is_text_input(&self) -> bool {
        matches!(
            self,
            Overlay::Prompt(_) | Overlay::AddTask(_) | Overlay::Edit(_)
        )
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect, cx: &Ctx) {
        match self {
            Overlay::Prompt(p) => p.draw(frame, area, cx),
            Overlay::AddTask(a) => a.draw(frame, area, cx),
            Overlay::Edit(e) => e.draw(frame, area, cx),
            Overlay::ConfirmDelete(c) => c.draw(frame, area, cx),
            Overlay::ConfirmClearFilters(c) => c.draw(frame, area, cx),
            Overlay::ConfirmRun(c) => c.draw(frame, area, cx),
            Overlay::Alert(a) => a.draw(frame, area, cx),
            Overlay::Help(h) => h.draw(frame, area),
        }
    }

    fn on_key(&mut self, key: KeyEvent, cx: &mut Ctx) -> Cmd {
        match self {
            Overlay::Prompt(p) => p.on_key(key, cx),
            Overlay::AddTask(a) => a.on_key(key, cx),
            Overlay::Edit(e) => e.on_key(key, cx),
            Overlay::ConfirmDelete(c) => c.on_key(key, cx),
            Overlay::ConfirmClearFilters(c) => c.on_key(key, cx),
            Overlay::ConfirmRun(c) => c.on_key(key, cx),
            Overlay::Alert(a) => a.on_key(key, cx),
            Overlay::Help(h) => h.on_key(key),
        }
    }

    fn on_mouse(&mut self, m: MouseEvent, cx: &mut Ctx) -> Cmd {
        match self {
            Overlay::AddTask(a) => a.on_mouse(m, cx),
            Overlay::Edit(e) => e.on_mouse(m, cx),
            Overlay::Help(h) => h.on_mouse(m, cx),
            _ => Cmd::None,
        }
    }
}

pub struct App {
    ctx: Ctx,
    browse: Browse,
    overlay: Option<Overlay>,
    running: bool,
}

impl App {
    pub fn new() -> anyhow::Result<Self> {
        let settings = Settings::load();
        crate::ui::apply_theme(&settings.theme);
        let store = Store::load()?;
        let profile = store
            .profiles
            .iter()
            .position(|p| p.name == settings.last_profile)
            .unwrap_or(0);
        Ok(Self {
            ctx: Ctx {
                store,
                settings,
                log: Vec::new(),
                area: Rect::new(0, 0, 0, 0),
                tick: 0,
                shake: 0,
                profile,
                pcursor: profile,
                task: 0,
                subtab: 0,
            },
            browse: Browse::new(),
            overlay: None,
            running: true,
        })
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
        let mut last_tick = Instant::now();
        while self.running {
            terminal.draw(|frame| self.draw(frame))?;
            if self.busy() {
                let wait = if self.ctx.shake > 0 { 40 } else { 100 };
                if event::poll(Duration::from_millis(wait))? {
                    let ev = event::read()?;
                    self.dispatch(ev);
                }
                self.tick_async();
                if self.ctx.shake > 0 {
                    self.ctx.shake -= 1;
                }
                if last_tick.elapsed() >= Duration::from_millis(100) {
                    self.ctx.tick = self.ctx.tick.wrapping_add(1);
                    last_tick = Instant::now();
                }
            } else {
                let ev = event::read()?;
                self.dispatch(ev);
            }
        }
        Ok(())
    }

    fn busy(&self) -> bool {
        self.ctx.shake > 0 || self.browse.busy()
    }

    fn tick_async(&mut self) {
        self.browse.tick(&mut self.ctx);
    }

    fn draw(&mut self, frame: &mut Frame) {
        self.ctx.area = frame.area();
        let area = self.ctx.area;
        self.browse.draw(frame, area, &self.ctx);
        if let Some(overlay) = &mut self.overlay {
            overlay.draw(frame, area, &self.ctx);
        }
    }

    fn dispatch(&mut self, ev: Event) {
        match ev {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                let cmd = self.on_key(key);
                self.apply(cmd);
            }
            Event::Mouse(m) => {
                let cmd = self.on_mouse(m);
                self.apply(cmd);
            }
            Event::Paste(text) => self.on_paste(text),
            _ => {}
        }
    }

    fn on_paste(&mut self, text: String) {
        if !self.text_entry_active() {
            return;
        }
        for c in text.chars().filter(|c| !c.is_control()) {
            let cmd = self.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
            self.apply(cmd);
        }
    }

    fn text_entry_active(&self) -> bool {
        match &self.overlay {
            Some(o) => o.is_text_input(),
            None => self.browse.text_entry(),
        }
    }

    fn on_key(&mut self, key: KeyEvent) -> Cmd {
        if key.code == KeyCode::Char('g') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.ctx.settings.hints = !self.ctx.settings.hints;
            let _ = self.ctx.settings.save();
            return Cmd::None;
        }
        if let Some(overlay) = &mut self.overlay {
            return overlay.on_key(key, &mut self.ctx);
        }
        self.browse.on_key(key, &mut self.ctx)
    }

    fn on_mouse(&mut self, m: MouseEvent) -> Cmd {
        if let Some(overlay) = &mut self.overlay {
            return overlay.on_mouse(m, &mut self.ctx);
        }
        self.browse.on_mouse(m, &mut self.ctx)
    }

    fn apply(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::None => {}
            Cmd::Quit => {
                if let Some(name) = self.ctx.active_profile().map(|p| p.name.clone()) {
                    self.ctx.settings.last_profile = name;
                }
                let _ = self.ctx.settings.save();
                self.running = false;
            }
            Cmd::Overlay(overlay) => self.overlay = Some(overlay),
            Cmd::Close => {
                self.overlay = None;
                self.browse.on_resume(&self.ctx);
            }
            Cmd::RequestRun(batch) => self.apply(Cmd::StartRun(batch)),
            Cmd::StartRun(batch) => {
                self.overlay = None;
                self.browse.start_run(batch, &mut self.ctx);
            }
        }
    }
}
