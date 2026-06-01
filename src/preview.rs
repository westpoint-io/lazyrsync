use crate::profile::Task;
use crate::rsync;
use std::io::{BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;

pub enum PreviewMsg {
    Progress(crate::run::Progress),
    Found(Change),
    Done(Box<Preview>),
    Failed(i32, String),
}

pub struct PreviewHandle {
    pub rx: Receiver<PreviewMsg>,
    child: Arc<Mutex<Option<Child>>>,
}

impl PreviewHandle {
    pub fn cancel(&self) {
        if let Ok(mut slot) = self.child.lock() {
            if let Some(child) = slot.as_mut() {
                let _ = child.kill();
            }
        }
    }
}

impl Drop for PreviewHandle {
    fn drop(&mut self) {
        self.cancel();
    }
}

pub fn spawn(task: &Task) -> PreviewHandle {
    let args = rsync::build_args(task, true);
    let (tx, rx) = mpsc::channel();
    let child_slot: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));
    let slot = child_slot.clone();
