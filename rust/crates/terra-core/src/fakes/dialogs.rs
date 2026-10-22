use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::ports::{FileDialogs, FileFilter};

/// Scripted dialog answers: each call pops the next queued path (or cancels).
#[derive(Debug, Default)]
pub struct FakeFileDialogs {
    open_answers: Mutex<VecDeque<Option<PathBuf>>>,
    save_answers: Mutex<VecDeque<Option<PathBuf>>>,
}

impl FakeFileDialogs {
    pub fn queue_open(&self, answer: Option<PathBuf>) {
        self.open_answers
            .lock()
            .expect("poisoned")
            .push_back(answer);
    }

    pub fn queue_save(&self, answer: Option<PathBuf>) {
        self.save_answers
            .lock()
            .expect("poisoned")
            .push_back(answer);
    }
}

impl FileDialogs for FakeFileDialogs {
    fn pick_open(&self, _filters: &[FileFilter]) -> Option<PathBuf> {
        self.open_answers
            .lock()
            .expect("poisoned")
            .pop_front()
            .flatten()
    }

    fn pick_save(&self, _filters: &[FileFilter], _default_name: &str) -> Option<PathBuf> {
        self.save_answers
            .lock()
            .expect("poisoned")
            .pop_front()
            .flatten()
    }
}
