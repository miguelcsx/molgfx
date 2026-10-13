//! A structure's interaction words plus the rows each recent edit touched.

use std::ops::Range;

use crate::Column;

/// Edits remembered per column; older ones fall back to a full upload.
const LOG_SLOTS: u64 = 8;

#[derive(Clone, Copy, Debug, Default)]
struct Change {
    revision: u64,
    start: u32,
    end: u32,
}

/// Dense state words with a short ring of per-revision dirty row spans, so a
/// consumer that last saw revision `r` can upload only the rows edited since.
#[derive(Clone, Debug, Default)]
pub(crate) struct InteractionColumn {
    words: Column<u32>,
    log: [Change; 8],
}

impl InteractionColumn {
    pub(crate) fn new(words: Vec<u32>) -> Self {
        Self {
            words: Column::new(words),
            log: [Change::default(); 8],
        }
    }

    pub(crate) const fn column(&self) -> &Column<u32> {
        &self.words
    }

    /// Mutable words; the caller must follow with [`Self::record`].
    pub(crate) fn words_mut(&mut self) -> &mut [u32] {
        self.words.values_mut()
    }

    /// Logs the rows edited by the most recent `words_mut` call.
    pub(crate) fn record(&mut self, rows: Range<u32>) {
        let revision = self.words.revision().get();
        self.log[slot(revision)] = Change {
            revision,
            start: rows.start,
            end: rows.end.max(rows.start),
        };
    }

    /// Rows edited after `since`, or `None` when the log cannot prove it.
    pub(crate) fn dirty_since(&self, since: u64) -> Option<Range<u32>> {
        let current = self.words.revision().get();
        if since > current || current - since > LOG_SLOTS {
            return None;
        }
        let mut hull: Option<Range<u32>> = None;
        for revision in since + 1..=current {
            let change = self.log[slot(revision)];
            if change.revision != revision {
                return None;
            }
            if change.start < change.end {
                hull = Some(match hull {
                    Some(h) => h.start.min(change.start)..h.end.max(change.end),
                    None => change.start..change.end,
                });
            }
        }
        Some(match hull {
            Some(hull) => hull,
            None => 0..0,
        })
    }
}

fn slot(revision: u64) -> usize {
    usize::from(revision.to_le_bytes()[0] & 7)
}

/// The smallest row range covering every row added to it.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RowSpan(Option<(u32, u32)>);

impl RowSpan {
    pub(crate) fn add(&mut self, row: u32) {
        self.0 = Some(match self.0 {
            Some((start, end)) => (start.min(row), end.max(row.saturating_add(1))),
            None => (row, row.saturating_add(1)),
        });
    }

    pub(crate) const fn is_empty(&self) -> bool {
        self.0.is_none()
    }

    pub(crate) fn rows(&self) -> Range<u32> {
        self.0.map_or(0..0, |(start, end)| start..end)
    }
}
