use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
};

use crate::app::{Ctx, LogKind};
use crate::preview::{self, Change, ChangeKind, Preview, PreviewHandle, PreviewMsg};
use crate::profile::Task;
use crate::run::{self, Progress, RunHandle, RunMsg};
use crate::ui::{
    accent, added, bytes, deleted, human_bytes, modified, muted, preview_text, secondary, warn,
};

const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

const SYNC_SCAN_LIMIT: usize = 200_000;

struct SearchHandle {
    rx: Receiver<Vec<usize>>,
    cancel: Arc<AtomicBool>,
}

impl Drop for SearchHandle {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

fn spawn_search(changes: Arc<Vec<Change>>, q_lower: String) -> SearchHandle {
    let (tx, rx) = std::sync::mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    thread::spawn(move || {
        let mut matches = Vec::new();
        for (i, c) in changes.iter().enumerate() {
            if i % 65536 == 0 && flag.load(Ordering::Relaxed) {
                return;
            }
            if contains_ci(&c.path, &q_lower) {
                matches.push(i);
            }
        }
        let _ = tx.send(matches);
    });
    SearchHandle { rx, cancel }
}

#[derive(Clone, Copy, PartialEq)]
enum JobKind {
    Real,
    Preview,
}

enum JobStatus {
    Queued,
    Active,
    Done(i32),
    Cancelled,
}

enum Active {
    Run(RunHandle),
    Preview(PreviewHandle),
}

struct Job {
    task: Task,
    label: String,
    kind: JobKind,
    status: JobStatus,
    progress: Option<Progress>,
    output: Vec<String>,
    preview: Option<Preview>,
    counts: Option<(usize, usize, usize)>,
    started: Instant,
    elapsed: u64,
}

impl Job {
    fn active_pct(&self) -> Option<(u8, bool)> {
        let p = self.progress.as_ref()?;
        let real = p.effective_percent();
        if real > 0 {
            return Some((real, true));
        }
        if p.files_done > 0 {
            if let Some(total) = self.task.last_files.filter(|t| *t > 0) {
                return Some(((p.files_done * 100 / total).clamp(1, 99) as u8, false));
            }
        }
        None
    }
}

pub struct Runs {
    jobs: Vec<Job>,
    handle: Option<Active>,
    active: Option<usize>,
    sel: usize,
    scroll: usize,
    cancelling: bool,
    search: String,
    searching: bool,
    search_on: bool,
    match_line: Option<usize>,
    matches: Vec<usize>,
    search_handle: Option<SearchHandle>,
    search_visible: usize,
}

impl Runs {
    pub fn new() -> Self {
        Runs {
            jobs: Vec::new(),
            handle: None,
            active: None,
            sel: 0,
            scroll: 0,
            cancelling: false,
            search: String::new(),
            searching: false,
            search_on: false,
            match_line: None,
            matches: Vec::new(),
            search_handle: None,
            search_visible: 0,
        }
    }

    pub fn running(&self) -> bool {
        self.active.is_some()
    }

    fn push_job(&mut self, task: Task, kind: JobKind) {
        self.jobs.push(Job {
            label: task.label.clone(),
            task,
            kind,
            status: JobStatus::Queued,
            progress: None,
            output: Vec::new(),
            preview: None,
            counts: None,
            started: Instant::now(),
            elapsed: 0,
        });
    }

    pub fn enqueue(&mut self, batch: Vec<Task>, cx: &mut Ctx) {
        self.drop_old_previews();
        for task in batch {
            self.push_job(task, JobKind::Real);
        }
        if self.active.is_none() {
            self.start_next(cx);
        }
        if let Some(a) = self.active {
            self.sel = a;
        }
    }

    pub fn preview(&mut self, task: Task, cx: &mut Ctx) {
        self.drop_old_previews();
        self.push_job(task, JobKind::Preview);
        let idx = self.jobs.len() - 1;
        if self.active.is_none() {
            self.start_next(cx);
        }
        self.sel = idx;
        self.scroll = 0;
    }

    fn drop_old_previews(&mut self) {
        const HUGE: usize = 100_000;
        for j in &mut self.jobs {
            if j.kind == JobKind::Preview {
                let huge = j.preview.as_ref().is_some_and(|p| p.changes.len() > HUGE);
                if huge {
                    j.preview = None;
                }
            }
        }
    }

    fn start_next(&mut self, cx: &mut Ctx) {
        self.cancelling = false;
        match self
            .jobs
            .iter()
            .position(|j| matches!(j.status, JobStatus::Queued))
        {
            Some(i) => {
                self.jobs[i].status = JobStatus::Active;
                self.jobs[i].started = Instant::now();
                self.active = Some(i);
                match self.jobs[i].kind {
                    JobKind::Real => {
                        self.handle = Some(Active::Run(run::start(&self.jobs[i].task)));
                        cx.push_log(LogKind::Active, format!("Run - {}", self.jobs[i].label));
                        cx.push_log(
                            LogKind::Command,
                            crate::rsync::resolved_command(&self.jobs[i].task, false),
                        );
                    }
                    JobKind::Preview => {
                        self.handle = Some(Active::Preview(preview::spawn(&self.jobs[i].task)));
                        cx.push_log(LogKind::Active, format!("Dry-run - {}", self.jobs[i].label));
                        cx.push_log(
                            LogKind::Command,
                            crate::rsync::resolved_command(&self.jobs[i].task, true),
                        );
                    }
