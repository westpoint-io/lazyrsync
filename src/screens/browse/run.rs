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
                }
            }
            None => {
                self.active = None;
                self.handle = None;
            }
        }
    }

    pub fn tick(&mut self, cx: &mut Ctx) {
        self.poll_search();
        let Some(active) = self.active else {
            return;
        };
        let mut run_msgs: Vec<RunMsg> = Vec::new();
        let mut prev_msgs: Vec<PreviewMsg> = Vec::new();
        match &self.handle {
            Some(Active::Run(h)) => {
                while let Ok(m) = h.rx.try_recv() {
                    run_msgs.push(m);
                }
            }
            Some(Active::Preview(h)) => {
                while let Ok(m) = h.rx.try_recv() {
                    prev_msgs.push(m);
                }
            }
            None => return,
        }
        let mut finished: Option<i32> = None;
        for msg in run_msgs {
            match msg {
                RunMsg::Progress(p) => self.jobs[active].progress = Some(p),
                RunMsg::Line(l) => {
                    let out = &mut self.jobs[active].output;
                    out.push(l);
                    if out.len() > 2000 {
                        let n = out.len() - 2000;
                        out.drain(0..n);
                    }
                }
                RunMsg::Failed(e) => {
                    self.jobs[active].output.push(format!("error: {e}"));
                    finished = Some(-1);
                }
                RunMsg::Done { code } => finished = Some(code),
            }
        }
        for msg in prev_msgs {
            match msg {
                PreviewMsg::Progress(mut p) => {
                    p.percent = 0;
                    p.bytes = 0;
                    p.speed.clear();
                    self.jobs[active].progress = Some(p);
                }
                PreviewMsg::Found(c) => {
                    let sym = match c.kind {
                        ChangeKind::Added => '+',
                        ChangeKind::Modified => '~',
                        ChangeKind::Deleted => '-',
                    };
                    let out = &mut self.jobs[active].output;
                    out.push(format!("{sym} {}", c.path));
                    if out.len() > 2000 {
                        let n = out.len() - 2000;
                        out.drain(0..n);
                    }
                }
                PreviewMsg::Done(pv) => {
                    self.jobs[active].counts = Some(change_counts(&pv));
                    self.jobs[active].preview = Some(*pv);
                    finished = Some(0);
                }
                PreviewMsg::Failed(code, e) => {
                    self.jobs[active].output.push(format!("error: {e}"));
                    finished = Some(code);
                }
            }
        }
        if let Some(code) = finished {
            self.finish(active, code, cx);
        }
    }

    fn finish(&mut self, active: usize, code: i32, cx: &mut Ctx) {
        self.jobs[active].elapsed = self.jobs[active].started.elapsed().as_secs();
        let label = self.jobs[active].label.clone();
        let is_preview = self.jobs[active].kind == JobKind::Preview;
        if self.cancelling {
            self.jobs[active].status = JobStatus::Cancelled;
            let verb = if is_preview { "Dry-run" } else { "Run" };
            cx.push_log(LogKind::Warn, format!("{verb} Cancelled - {label}"));
            self.drop_queued();
            self.active = None;
            self.handle = None;
        } else if code == 0 {
            self.jobs[active].status = JobStatus::Done(0);
            if is_preview {
                if let Some(pv) = &self.jobs[active].preview {
                    let s = &pv.stats;
                    cx.push_log(
                        LogKind::Done,
                        format!(
                            "Dry-run Done - {label} - {} to transfer, {} new, {} deleted",
                            s.transferred, s.created, s.deleted
                        ),
                    );
                }
            } else {
                cx.push_log(LogKind::Done, format!("Run Done - {label}"));
            }
            let total = if is_preview {
                self.jobs[active].preview.as_ref().map(|pv| pv.stats.files)
            } else {
                self.jobs[active]
                    .progress
                    .as_ref()
                    .filter(|p| p.files_final)
                    .map(|p| p.files_total)
            };
            if let Some(total) = total.filter(|t| *t > 0) {
                self.remember_total(active, total, cx);
            }
            self.start_next(cx);
        } else if is_preview {
            self.jobs[active].status = JobStatus::Done(code);
            cx.push_log(LogKind::Error, format!("Dry-run Failed - {label}"));
            self.start_next(cx);
        } else {
            self.jobs[active].status = JobStatus::Done(code);
            self.jobs[active]
                .output
                .push(format!("✗ rsync exited with code {code}"));
            cx.push_log(
                LogKind::Error,
                format!("Run Failed - {label} - exit {code}"),
            );
            self.start_next(cx);
        }
        if let Some(a) = self.active {
            self.sel = a;
        }
    }

    fn remember_total(&mut self, job_idx: usize, total: u64, cx: &mut Ctx) {
        let (id, source, dest) = {
            let t = &self.jobs[job_idx].task;
            (t.id.clone(), t.source.clone(), t.dest.clone())
        };
        let mut changed = false;
        for p in &mut cx.store.profiles {
            for t in &mut p.tasks {
                if t.id == id && t.source == source && t.dest == dest {
                    changed |= t.last_files != Some(total);
                    t.last_files = Some(total);
                }
            }
        }
        for j in &mut self.jobs {
            if j.task.id == id && j.task.source == source && j.task.dest == dest {
                j.task.last_files = Some(total);
            }
        }
        if changed {
            let _ = cx.store.save();
        }
    }

    fn drop_queued(&mut self) {
        for j in &mut self.jobs {
            if matches!(j.status, JobStatus::Queued) {
                j.status = JobStatus::Cancelled;
            }
        }
    }

    pub fn select(&mut self, delta: i32) {
        if self.jobs.is_empty() {
            return;
        }
        let n = self.jobs.len() as i32;
        self.sel = (self.sel as i32 + delta).rem_euclid(n) as usize;
        self.scroll = 0;
        self.exit_search();
    }

    pub fn scroll(&mut self, delta: i32) {
        let max = self.content_len() as i32;
        self.scroll = (self.scroll as i32 + delta).clamp(0, max) as usize;
    }

    fn preview_done(&self) -> bool {
        self.sel_job()
            .is_some_and(|j| j.kind == JobKind::Preview && matches!(j.status, JobStatus::Done(0)))
    }

    fn content_len(&self) -> usize {
        let Some(job) = self.sel_job() else { return 0 };
        if self.preview_done() {
            job.preview.as_ref().map_or(0, |p| p.changes.len())
        } else {
            job.output.len()
        }
    }

    pub fn overflow(&self, visible: usize) -> Option<(usize, usize)> {
        let total = self.content_len();
        if total <= visible {
            return None;
        }
        let top = if self.preview_done() {
            self.scroll.min(total - visible)
        } else {
            total.saturating_sub(visible + self.scroll)
        };
        Some((total, top))
    }
