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
