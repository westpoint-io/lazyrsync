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
                    let _ = tx.send(PreviewMsg::Found(c.clone()));
                    changes.push(c);
                }
                tail.push_back(seg);
                if tail.len() > TAIL {
                    tail.pop_front();
                }
            }
        });
        let err = err_thread.join().unwrap_or_default();

        let code = match slot.lock().unwrap().take() {
            Some(mut child) => child.wait().ok().and_then(|s| s.code()).unwrap_or(-1),
            None => -1,
        };
        if code != 0 && code != 24 {
            let msg = err
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .unwrap_or("rsync failed");
            let _ = tx.send(PreviewMsg::Failed(
                code,
                format!("rsync exited {code}: {msg}"),
            ));
            return;
        }

        let trailer: Vec<&str> = tail.iter().map(String::as_str).collect();
        let stats = parse_stats(&trailer.join("\n"));
        let _ = tx.send(PreviewMsg::Done(Box::new(Preview {
            changes: Arc::new(changes),
            stats,
        })));
    });

    PreviewHandle {
        rx,
        child: child_slot,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone)]
pub struct Change {
    pub kind: ChangeKind,
    pub path: Box<str>,
}

#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub files: u64,
    pub dirs: u64,
    pub created: u64,
    pub deleted: u64,
    pub transferred: u64,
    pub total_size: u64,
    pub transferred_size: u64,
}

#[derive(Debug, Clone)]
pub struct Preview {
    pub changes: Arc<Vec<Change>>,
    pub stats: Stats,
}

pub fn parse_itemized(output: &str) -> Vec<Change> {
    let mut changes = Vec::new();
    for line in output.lines() {
        if let Some(rest) = line.strip_prefix("*deleting") {
            changes.push(Change {
                kind: ChangeKind::Deleted,
                path: rest.trim().into(),
            });
            continue;
        }

        let bytes = line.as_bytes();
        if bytes.len() < 12 || bytes[11] != b' ' {
            continue;
        }
        let update = bytes[0] as char;

        if !matches!(update, '<' | '>' | 'c' | 'h') {
            continue;
        }
        let flags = &line[2..11];
        let path = line[12..].trim();
        if path.is_empty() {
            continue;
        }
        let path: Box<str> = path.into();
        let kind = if flags.chars().all(|c| c == '+') {
            ChangeKind::Added
        } else {
            ChangeKind::Modified
        };
        changes.push(Change { kind, path });
    }
    changes
}

fn stat_num(output: &str, prefix: &str) -> u64 {
    for line in output.lines() {
        if let Some(rest) = line.trim().strip_prefix(prefix) {
            let digits: String = rest
                .chars()
                .take_while(|c| *c != '(')
                .filter(|c| c.is_ascii_digit())
                .collect();
            if let Ok(n) = digits.parse() {
                return n;
            }
        }
    }
    0
}

fn dir_count(output: &str) -> u64 {
    for line in output.lines() {
        if let Some(rest) = line.trim().strip_prefix("Number of files:") {
            if let Some(idx) = rest.find("dir:") {
                let digits: String = rest[idx + 4..]
                    .chars()
                    .take_while(|c| *c != ')')
                    .filter(|c| c.is_ascii_digit())
                    .collect();
                return digits.parse().unwrap_or(0);
            }
        }
    }
    0
}

pub fn parse_stats(output: &str) -> Stats {
    Stats {
        files: stat_num(output, "Number of files:"),
        dirs: dir_count(output),
        created: stat_num(output, "Number of created files:"),
        deleted: stat_num(output, "Number of deleted files:"),
        transferred: stat_num(output, "Number of regular files transferred:"),
        total_size: stat_num(output, "Total file size:"),
        transferred_size: stat_num(output, "Total transferred file size:"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "sending incremental file list
>f+++++++++ new_file.txt
cd+++++++++ subdir/
>f+++++++++ subdir/nested.txt
>f..t...... unchanged_time_only.txt
>f.st...... changed.txt
*deleting   will_be_deleted.txt
.d..t...... ./
