use std::io::{BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::profile::Task;
use crate::rsync;

#[derive(Debug, Clone, Default)]
pub struct Progress {
    pub percent: u8,
    pub speed: String,
    pub eta: String,
    pub files_done: u64,
    pub files_total: u64,
    pub files_final: bool,
    pub bytes: u64,
}

impl Progress {
    pub fn effective_percent(&self) -> u8 {
        if self.percent > 0 || !self.files_final || self.files_total == 0 {
            self.percent
        } else {
            (self.files_done * 100 / self.files_total).min(100) as u8
        }
    }
}

#[derive(Debug)]
pub enum RunMsg {
    Progress(Progress),

    Line(String),
    Done { code: i32 },
    Failed(String),
}

pub struct RunHandle {
    pub rx: Receiver<RunMsg>,
    child: Arc<Mutex<Option<Child>>>,
}

impl RunHandle {
    pub fn cancel(&self) {
        if let Ok(mut slot) = self.child.lock() {
            if let Some(child) = slot.as_mut() {
                let _ = child.kill();
            }
        }
    }
}

pub fn start(task: &Task) -> RunHandle {
    let args = rsync::build_args(task, false);
    let task = task.clone();
    let (tx, rx) = mpsc::channel();
    let child_slot: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));
    let slot = child_slot.clone();

    thread::spawn(move || {
        if let Err(e) = rsync::prepare_dest(&task) {
            let _ = tx.send(RunMsg::Failed(format!("could not create destination: {e}")));
            return;
        }
        let mut child = match Command::new("rsync")
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(RunMsg::Failed(format!(
                    "failed to launch rsync: {e} (is rsync installed?)"
                )));
                return;
            }
        };

        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        *slot.lock().unwrap() = Some(child);

        let tx_err = tx.clone();
        let stderr_thread = thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            read_segments(&mut reader, |seg| {
                let _ = tx_err.send(RunMsg::Line(seg));
            });
        });

        let mut reader = BufReader::new(stdout);
        read_segments(&mut reader, |seg| {
            let msg = match parse_progress(&seg) {
                Some(p) => RunMsg::Progress(p),
                None => RunMsg::Line(seg),
            };
            let _ = tx.send(msg);
        });

        let _ = stderr_thread.join();

        let code = match slot.lock().unwrap().take() {
            Some(mut child) => child.wait().ok().and_then(|s| s.code()).unwrap_or(-1),
            None => -1,
        };
        let _ = tx.send(RunMsg::Done { code });
    });

    RunHandle {
        rx,
        child: child_slot,
    }
}

pub fn read_segments<R: Read>(reader: &mut BufReader<R>, mut on_seg: impl FnMut(String)) {
    let mut buf: Vec<u8> = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match reader.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => {
                let b = byte[0];
                if b == b'\n' || b == b'\r' {
                    if !buf.is_empty() {
                        on_seg(String::from_utf8_lossy(&buf).trim_end().to_string());
                        buf.clear();
                    }
                } else {
                    buf.push(b);
                }
            }
            Err(_) => break,
        }
    }
    if !buf.is_empty() {
        on_seg(String::from_utf8_lossy(&buf).trim_end().to_string());
    }
}

pub fn parse_progress(line: &str) -> Option<Progress> {
    if !line.contains('%') {
        return None;
    }
    let toks: Vec<&str> = line.split_whitespace().collect();
    let mut p = Progress::default();

    if let Some(first) = toks.first() {
        let digits: String = first.chars().filter(|c| c.is_ascii_digit()).collect();
        p.bytes = digits.parse().unwrap_or(0);
    }
    for t in &toks {
        if let Some(pct) = t.strip_suffix('%') {
            if let Ok(v) = pct.parse::<u8>() {
                p.percent = v;
            }
        } else if t.contains("B/s") {
            p.speed = (*t).to_string();
        } else if t.matches(':').count() == 2 {
            p.eta = (*t).to_string();
        }
    }
    for key in ["to-chk=", "ir-chk="] {
        if let Some(idx) = line.find(key) {
            let rest = &line[idx + key.len()..];
            let nums: String = rest
