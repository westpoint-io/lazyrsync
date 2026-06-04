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
