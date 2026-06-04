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
