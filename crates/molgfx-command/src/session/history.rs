//! Undo and redo of scene edits together with the session state.

use super::state::SessionSpec;
use molgfx_api::PatchOperation;
use std::collections::VecDeque;

/// One executed program: how to replay it and how to reverse it.
#[derive(Clone, Debug)]
pub(crate) struct Entry {
    pub(crate) label: String,
    pub(crate) forward: Vec<PatchOperation>,
    pub(crate) inverse: Vec<PatchOperation>,
    pub(crate) before: SessionSpec,
    pub(crate) after: SessionSpec,
}

/// Executed and undone programs, oldest first, bounded in length.
#[derive(Clone, Debug, Default)]
pub(crate) struct History {
    done: VecDeque<Entry>,
    undone: Vec<Entry>,
}

impl History {
    /// The most programs kept for undo.
    pub(crate) const LIMIT: usize = 256;

    /// Records a new program; anything undone can no longer be redone.
    pub(crate) fn record(&mut self, entry: Entry) {
        self.undone.clear();
        if self.done.len() == Self::LIMIT {
            let _ = self.done.pop_front();
        }
        self.done.push_back(entry);
    }

    pub(crate) fn take_undo(&mut self) -> Option<Entry> {
        self.done.pop_back()
    }

    pub(crate) fn undone(&mut self, entry: Entry) {
        self.undone.push(entry);
    }

    pub(crate) fn take_redo(&mut self) -> Option<Entry> {
        self.undone.pop()
    }

    pub(crate) fn redone(&mut self, entry: Entry) {
        self.done.push_back(entry);
    }

    pub(crate) fn clear(&mut self) {
        self.done.clear();
        self.undone.clear();
    }

    pub(crate) fn labels(&self) -> Vec<String> {
        self.done.iter().map(|entry| entry.label.clone()).collect()
    }

    pub(crate) fn can_undo(&self) -> bool {
        !self.done.is_empty()
    }

    pub(crate) fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }
}
