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

    pub fn click_scroll(&mut self, frac: f64, visible: usize) {
        let max = self.content_len().saturating_sub(visible);
        let top = (frac.clamp(0.0, 1.0) * max as f64).round() as usize;
        self.scroll = if self.preview_done() { top } else { max - top };
    }

    fn scroll_to(&mut self, line: usize, visible: usize) {
        let max = self.content_len().saturating_sub(visible);
        let top = line.min(max);
        self.scroll = if self.preview_done() { top } else { max - top };
    }

    fn scan_matches(&self) -> Vec<usize> {
        let q = self.search.to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }
        let Some(job) = self.sel_job() else {
            return Vec::new();
        };
        if self.preview_done() {
            job.preview.as_ref().map_or(Vec::new(), |p| {
                p.changes
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| contains_ci(&c.path, &q))
                    .map(|(i, _)| i)
                    .collect()
            })
        } else {
            job.output
                .iter()
                .enumerate()
                .filter(|(_, l)| contains_ci(l, &q))
                .map(|(i, _)| i)
                .collect()
        }
    }

    pub fn searching(&self) -> bool {
        self.searching
    }

    pub fn search_on(&self) -> bool {
        self.search_on
    }

    pub fn scanning(&self) -> bool {
        self.search_handle.is_some()
    }

    pub fn search_query(&self) -> &str {
        &self.search
    }

    fn poll_search(&mut self) {
        let done = self
            .search_handle
            .as_ref()
            .and_then(|h| h.rx.try_recv().ok());
        if let Some(matches) = done {
            self.matches = matches;
            self.match_line = self.matches.first().copied();
            self.search_handle = None;
            if let Some(l) = self.match_line {
                self.scroll_to(l, self.search_visible);
            }
        }
    }

    pub fn match_pos(&self) -> (usize, usize) {
        let pos = self
            .match_line
            .and_then(|l| self.matches.iter().position(|&i| i == l))
            .map_or(0, |p| p + 1);
        (pos, self.matches.len())
    }

    pub fn start_search(&mut self) {
        self.searching = true;
        self.search.clear();
        self.search_on = false;
        self.match_line = None;
        self.matches.clear();
        self.search_handle = None;
    }

    pub fn exit_search(&mut self) {
        self.searching = false;
        self.search_on = false;
        self.search.clear();
        self.match_line = None;
        self.matches.clear();
        self.search_handle = None;
    }

    pub fn search_key(&mut self, key: KeyEvent, visible: usize) {
        match key.code {
            KeyCode::Esc => self.exit_search(),
            KeyCode::Enter => {
                self.searching = false;
                self.search_on = true;
                self.match_line = None;
                self.matches.clear();
                self.search_visible = visible;
                let q = self.search.to_lowercase();
                let changes = self
                    .preview_done()
                    .then(|| self.sel_job().and_then(|j| j.preview.as_ref()))
                    .flatten()
                    .map(|pv| pv.changes.clone());
                match changes {
                    Some(changes) if changes.len() > SYNC_SCAN_LIMIT => {
                        self.search_handle = Some(spawn_search(changes, q));
                    }
                    Some(changes) => {
                        self.matches = changes
                            .iter()
                            .enumerate()
                            .filter(|(_, c)| contains_ci(&c.path, &q))
                            .map(|(i, _)| i)
                            .collect();
                        self.match_line = self.matches.first().copied();
                        if let Some(l) = self.match_line {
                            self.scroll_to(l, visible);
                        }
                    }
                    None => {
                        self.matches = self.scan_matches();
                        self.match_line = self.matches.first().copied();
                        if let Some(l) = self.match_line {
                            self.scroll_to(l, visible);
                        }
                    }
                }
            }
            KeyCode::Backspace => {
                self.search.pop();
            }
            KeyCode::Char(c) => self.search.push(c),
            _ => {}
        }
    }

    pub fn next_match(&mut self, visible: usize, delta: i32) {
        if self.matches.is_empty() {
            return;
        }
        let cur = self
            .match_line
            .and_then(|l| self.matches.iter().position(|&i| i == l));
        let n = self.matches.len() as i32;
        let next = match cur {
            Some(p) => (p as i32 + delta).rem_euclid(n) as usize,
            None => 0,
        };
        self.match_line = Some(self.matches[next]);
        self.scroll_to(self.matches[next], visible);
    }

    fn sel_job(&self) -> Option<&Job> {
        self.jobs.get(self.sel)
    }

    pub fn cancel(&mut self) {
        if self.active.is_none() {
            return;
        }
        self.cancelling = true;
        match &self.handle {
            Some(Active::Run(h)) => h.cancel(),
            Some(Active::Preview(h)) => h.cancel(),
            None => {}
        }
    }

    pub fn sel_mode(&self) -> Option<(&'static str, ratatui::style::Color)> {
        let j = self.sel_job()?;
        match j.kind {
            JobKind::Preview => Some(("DRY-RUN", accent())),
            JobKind::Real => {
                let transferred = j
                    .progress
                    .as_ref()
                    .is_some_and(|p| p.percent > 0 || p.bytes > 0);
                if transferred {
                    Some(("TRANSFER", added()))
                } else {
                    Some(("CHECK", warn()))
                }
            }
        }
    }

    pub fn position(&self) -> Option<(usize, usize)> {
        if self.jobs.is_empty() {
            None
        } else {
            Some((self.sel + 1, self.jobs.len()))
        }
    }

    pub fn sel_label(&self) -> Option<String> {
        self.sel_job().map(|j| j.label.clone())
    }

    pub fn rail_line(&self, cx: &Ctx) -> Line<'static> {
        let mut line = self.rail_line_body(cx);
        line.spans.insert(0, " ".into());
        line
    }

    fn rail_line_body(&self, cx: &Ctx) -> Line<'static> {
        let Some(job) = self.sel_job() else {
            return Line::from("No runs yet".fg(Color::Reset));
        };
        let name = trunc(&job.label, 18);
        match &job.status {
            JobStatus::Cancelled => Line::from(vec![
                "✗ ".fg(warn()),
                name.into(),
                " [CANCELLED]".fg(warn()),
            ]),
            JobStatus::Done(c) if *c != 0 => Line::from(vec![
                "✗ ".fg(deleted()),
                name.into(),
                " [FAILED]".fg(deleted()),
            ]),
            JobStatus::Queued => Line::from(vec![
                "⏸ ".fg(Color::Reset),
                name.into(),
                " [QUEUED]".fg(Color::Reset),
            ]),
            JobStatus::Active => {
                let sp = SPINNER[cx.tick % SPINNER.len()];
                let tail = match job.active_pct() {
                    Some((pct, true)) => format!("{pct}%").fg(added()),
                    Some((pct, false)) => format!("~{pct}%").fg(added()),
                    None => "--%".fg(Color::Reset),
                };
                Line::from(vec![
                    format!("{sp} ").fg(accent()),
                    format!("{name} · ").into(),
                    tail,
                ])
            }
            JobStatus::Done(_) => match job.kind {
                JobKind::Real => {
                    Line::from(vec!["✓ ".fg(added()), name.into(), " [DONE]".fg(added())])
                }
                JobKind::Preview => {
                    let (a, m, d) = job.counts.unwrap_or((0, 0, 0));
                    Line::from(vec![
                        "✓ ".fg(added()),
                        format!("{name}  ").into(),
                        format!("+{}", compact(a)).fg(added()),
                        format!(" ~{}", compact(m)).fg(modified()),
                        format!(" -{}", compact(d)).fg(deleted()),
                    ])
                }
            },
        }
    }

    pub fn main_text(&self, cx: &Ctx, visible: usize) -> Text<'static> {
        let Some(job) = self.sel_job() else {
            return Text::from(vec![
                Line::from("No runs yet".fg(Color::Reset)),
                Line::from(""),
                Line::from("Press r to run · p to preview".fg(Color::Reset)),
            ]);
        };
        let q = self.search_on.then(|| self.search.to_lowercase());
        if self.preview_done() {
            return match &job.preview {
                Some(pv) => preview_text(pv, self.scroll, visible, q.as_deref(), self.match_line),
                None => Text::from(dropped_preview_summary(job)),
            };
        }
        if let JobStatus::Done(code) = &job.status {
            let code = *code;
            return Text::from(if code == 0 {
                let transferred = job
                    .progress
                    .as_ref()
                    .is_some_and(|p| p.percent > 0 || p.bytes > 0);
                done_summary(job, transferred)
            } else {
                failed_summary(job, code)
            });
        }
        let mut lines = Vec::new();
        match &job.status {
            JobStatus::Active => lines.push(active_line(job, cx)),
            JobStatus::Done(0) => lines.push(Line::from(
                format!("   ✓ done · {}", fmt_dur(job.elapsed)).fg(added()),
            )),
            JobStatus::Done(c) => {
                lines.push(Line::from(format!("   ✗ failed · exit {c}").fg(deleted())))
            }
            JobStatus::Cancelled => match progress_line(job, cx, job.elapsed, true) {
                Some(l) => lines.push(l),
                None => lines.push(Line::from("   Cancelled".fg(warn()))),
            },
            JobStatus::Queued => lines.push(Line::from(
                "   Queued — waiting for the active run".fg(Color::Reset),
            )),
        }
        let out = &job.output;
        if out.is_empty() && job.kind == JobKind::Preview && matches!(job.status, JobStatus::Active)
        {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                "   ⚑ ".fg(accent()),
                "No changes found yet".fg(Color::Reset),
            ]));
        }
        let room = visible.saturating_sub(lines.len()).max(1);
        let start = out.len().saturating_sub(room).saturating_sub(self.scroll);
        let end = (start + room).min(out.len());
        for (j, l) in out[start..end].iter().enumerate() {
            let i = start + j;
            let base = line_style(l);
            let mut spans = vec![Span::raw("   ")];
            match &q {
                Some(ql) => spans.extend(crate::ui::highlight_spans(
                    l,
                    base,
                    ql,
                    self.match_line == Some(i),
                )),
                None => spans.push(Span::styled(l.clone(), base)),
            }
            lines.push(Line::from(spans));
        }
        Text::from(lines)
    }
}

fn active_line(job: &Job, cx: &Ctx) -> Line<'static> {
    progress_line(job, cx, job.started.elapsed().as_secs(), false).unwrap_or_else(|| {
        let sp = SPINNER[cx.tick % SPINNER.len()];
        Line::from(format!("   {sp} Scanning…").fg(Color::Reset))
    })
}

fn progress_line(job: &Job, cx: &Ctx, elapsed: u64, frozen: bool) -> Option<Line<'static>> {
    match job.active_pct() {
        Some((pct, exact)) => {
            let p = job.progress.as_ref().expect("active_pct implies progress");
            let mut spans = bar(pct as f64 / 100.0, 24);
            let mark = if exact { "" } else { "~" };
            spans.push(format!(" {mark}{pct}%").fg(added()));
            if (p.percent > 0 || p.bytes > 0) && !p.speed.is_empty() {
                spans.push(format!("  {}", p.speed).fg(bytes()));
            } else if p.files_final && p.files_total > 0 {
                spans.push(
                    format!(
                        "  checked {}/{}",
                        commas(p.files_done),
                        commas(p.files_total)
                    )
                    .fg(Color::Reset),
                );
            } else if p.files_done > 0 {
                spans.push(format!("  checked {}", commas(p.files_done)).fg(Color::Reset));
            }
            spans.push("  elapsed ".fg(Color::Reset));
            spans.push(fmt_dur(elapsed).fg(Color::Reset));
            if p.bytes > 0 {
                spans.push(format!("  {}", human_bytes(p.bytes)).fg(bytes()));
            }
            spans.insert(0, Span::raw("   "));
            Some(Line::from(spans))
        }
        None => {
            let p = job.progress.as_ref()?;
            if p.files_done == 0 {
                return None;
            }
            let mut spans = if frozen {
                bar(0.0, 24)
            } else {
                pulse(24, cx.tick)
            };
            spans.push(format!("  checked {} files", commas(p.files_done)).fg(Color::Reset));
            spans.push("  elapsed ".fg(Color::Reset));
            spans.push(fmt_dur(elapsed).fg(Color::Reset));
            spans.insert(0, Span::raw("   "));
            Some(Line::from(spans))
        }
    }
}

fn rule_head(text: &str, color: Color, right: &str) -> Line<'static> {
    Line::from(vec![
        " ".into(),
        text.to_string().fg(color).bold(),
        "   ".into(),
        right.to_string().fg(secondary()),
    ])
}

fn divider() -> Line<'static> {
    Line::from(format!(" {}", "─".repeat(56)).fg(muted()))
}

#[derive(Default)]
struct Trailer {
    sent: String,
    received: String,
    rate: String,
    total: String,
}

fn parse_trailer(output: &[String]) -> Trailer {
    let mut t = Trailer::default();
    for l in output {
        let w: Vec<&str> = l.split_whitespace().collect();
        if l.starts_with("sent ") && l.contains("bytes/sec") {
            if let Some(i) = w.iter().position(|x| *x == "sent") {
                t.sent = w.get(i + 1).unwrap_or(&"").to_string();
            }
            if let Some(i) = w.iter().position(|x| *x == "received") {
                t.received = w.get(i + 1).unwrap_or(&"").to_string();
            }
            if let Some(i) = w.iter().position(|x| *x == "bytes/sec") {
                if i > 0 {
                    t.rate = w[i - 1].to_string();
                }
            }
        } else if l.starts_with("total size is") {
            if let Some(i) = w.iter().position(|x| *x == "is") {
                t.total = w.get(i + 1).unwrap_or(&"").to_string();
            }
        }
    }
    t
}

fn done_summary(job: &Job, transferred: bool) -> Vec<Line<'static>> {
    let elapsed = fmt_dur(job.elapsed);
    if !transferred {
        let checked = job
            .progress
            .as_ref()
            .filter(|p| p.files_final && p.files_total > 0)
            .map(|p| format!(" · {} files checked", commas(p.files_total)))
            .unwrap_or_default();
        return vec![
            rule_head("✓ ALREADY IN SYNC", added(), &elapsed),
            divider(),
            Line::from(format!("   Nothing to transfer{checked}").fg(Color::Reset)),
        ];
    }
    let t = parse_trailer(&job.output);
    let mut parts: Vec<String> = Vec::new();
    if !t.sent.is_empty() {
        parts.push(format!("{} sent", t.sent));
    }
    if !t.received.is_empty() {
        parts.push(format!("{} recv", t.received));
    }
    if !t.rate.is_empty() {
        parts.push(format!("{}/s", t.rate));
    }
    if !t.total.is_empty() {
        parts.push(format!("{} total", t.total));
    }
    let mut stats: Vec<Span<'static>> = vec!["   ".into()];
    for (i, p) in parts.into_iter().enumerate() {
        if i > 0 {
            stats.push("   ".fg(muted()));
        }
        stats.push(p.fg(bytes()));
    }
    vec![
        rule_head("✓ TRANSFER COMPLETE", added(), &elapsed),
        divider(),
        Line::from(stats),
    ]
}

fn failed_summary(job: &Job, code: i32) -> Vec<Line<'static>> {
    let mut lines = vec![
        rule_head(
            "✗ FAILED",
            deleted(),
            &format!("exit {code} · {}", fmt_dur(job.elapsed)),
        ),
        divider(),
    ];
    if let Some(err) = job
        .output
        .iter()
        .rev()
        .find(|l| l.contains("rsync:") || l.starts_with("error"))
    {
        lines.push(Line::from(format!("   {}", err.trim()).fg(Color::Reset)));
    }
    lines
}

fn dropped_preview_summary(job: &Job) -> Vec<Line<'static>> {
    let (a, m, d) = job.counts.unwrap_or((0, 0, 0));
    vec![
        rule_head("✓ DRY-RUN COMPLETE", accent(), &fmt_dur(job.elapsed)),
        divider(),
        Line::from(vec![
            "   ".into(),
            format!("+{} new", commas(a as u64)).fg(added()),
            "   ".into(),
            format!("~{} changed", commas(m as u64)).fg(modified()),
            "   ".into(),
            format!("-{} deleted", commas(d as u64)).fg(deleted()),
        ]),
        Line::from(""),
        Line::from("   Diff freed to save memory — press p to re-run it.".fg(muted())),
    ]
}

fn change_counts(pv: &Preview) -> (usize, usize, usize) {
    let mut a = 0;
    let mut m = 0;
    let mut d = 0;
    for c in pv.changes.iter() {
        match c.kind {
            ChangeKind::Added => a += 1,
            ChangeKind::Modified => m += 1,
            ChangeKind::Deleted => d += 1,
        }
    }
    (a, m, d)
}

fn trunc(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max - 1).collect();
        out.push('…');
        out
    }
}

fn fmt_dur(secs: u64) -> String {
    format!("{}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
}

fn contains_ci(haystack: &str, needle_lower: &str) -> bool {
    if needle_lower.is_empty() {
        return true;
    }
    let (h, n) = (haystack.as_bytes(), needle_lower.as_bytes());
    if n.len() > h.len() {
        return false;
    }
    (0..=h.len() - n.len()).any(|i| {
        h[i..i + n.len()]
            .iter()
            .zip(n)
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
    })
}

fn compact(n: usize) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => format!("{:.1}K", n as f64 / 1_000.0),
        _ => format!("{:.1}M", n as f64 / 1_000_000.0),
    }
}

fn commas(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn bar(ratio: f64, width: usize) -> Vec<Span<'static>> {
    let ticks = [' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉'];
    let t8 = (ratio.clamp(0.0, 1.0) * width as f64 * 8.0).round() as usize;
    let whole = (t8 / 8).min(width);
    let rem = t8 % 8;
    let part = if rem > 0 && whole < width {
        ticks[rem].to_string()
    } else {
        String::new()
    };
    let filled = format!("{}{}", "█".repeat(whole), part);
    let fw = whole + part.chars().count();
    let unfilled = "░".repeat(width.saturating_sub(fw));
    vec![filled.fg(accent()), unfilled.fg(muted())]
}

fn pulse(width: usize, tick: usize) -> Vec<Span<'static>> {
    let seg = 3.min(width);
    let span = width.saturating_sub(seg);
    let pos = if span == 0 {
        0
    } else {
        let p = tick % (span * 2);
        if p <= span {
            p
        } else {
            span * 2 - p
        }
    };
    let left = "░".repeat(pos);
    let mid = "█".repeat(seg);
    let right = "░".repeat(width - pos - seg);
    vec![left.fg(muted()), mid.fg(accent()), right.fg(muted())]
}

fn line_style(l: &str) -> Style {
    if l.starts_with("+ ") {
        return Style::new().fg(added());
    }
    if l.starts_with("~ ") {
        return Style::new().fg(modified());
    }
    if l.starts_with("- ") {
        return Style::new().fg(deleted());
    }
    let low = l.to_lowercase();
    if l.starts_with('✗')
        || low.contains("rsync error")
        || low.starts_with("rsync:")
        || low.starts_with("error")
    {
        Style::new().fg(deleted())
    } else {
        Style::new()
    }
}

#[cfg(test)]
mod tests {
    use super::contains_ci;

    #[test]
    fn contains_ci_matches_case_insensitively() {
        assert!(contains_ci("photos/.git/HEAD", "photos"));
        assert!(contains_ci("Photos/README", "photos"));
        assert!(contains_ci("path/to/PHOTOS", "photos"));
        assert!(contains_ci("anything", ""));
        assert!(!contains_ci("node_modules", "photos"));
        assert!(!contains_ci("cook", "photos"));
    }
}
