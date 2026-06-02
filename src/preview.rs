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

    thread::spawn(move || {
        let mut child = match Command::new("rsync")
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(PreviewMsg::Failed(
                    -1,
                    format!("failed to launch rsync: {e} (is rsync installed?)"),
                ));
                return;
            }
        };

        let mut stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        *slot.lock().unwrap() = Some(child);

        let err_thread = thread::spawn(move || {
            let mut e = String::new();
            let _ = BufReader::new(stderr).read_to_string(&mut e);
            e
        });

        const TAIL: usize = 64;
        let mut changes: Vec<Change> = Vec::new();
        let mut tail: std::collections::VecDeque<String> = std::collections::VecDeque::new();
        let mut reader = BufReader::new(&mut stdout);
        crate::run::read_segments(&mut reader, |seg| match crate::run::parse_progress(&seg) {
            Some(p) => {
                let _ = tx.send(PreviewMsg::Progress(p));
            }
            None => {
                if let Some(c) = parse_itemized(&seg).into_iter().next() {
