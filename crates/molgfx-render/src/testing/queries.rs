//! Questions tests ask of the mock log, answered once here.

use super::{Locked, MockLog};

impl MockLog {
    /// The id of the first buffer created under `label`, or `u32::MAX`.
    pub(crate) fn buffer_id(&self, label: &str) -> u32 {
        let buffers = self.buffers.locked();
        let found = buffers.iter().find(|(_, candidate, _)| *candidate == label);
        found.map_or(u32::MAX, |(id, _, _)| *id)
    }

    /// Whether any buffer was created under `label`.
    pub(crate) fn has_buffer(&self, label: &str) -> bool {
        self.buffer_id(label) != u32::MAX
    }

    /// How many physical buffers were created.
    pub(crate) fn buffer_count(&self) -> usize {
        self.buffers.locked().len()
    }

    /// How many indirect draws read arguments from `buffer`.
    pub(crate) fn indirect_count(&self, buffer: u32) -> usize {
        let draws = self.indirect_draws.locked();
        draws.iter().filter(|(id, _)| *id == buffer).count()
    }

    /// How many writes landed in buffers created under `label`.
    pub(crate) fn writes_to(&self, label: &str) -> usize {
        let buffers = self.buffers.locked();
        let writes = self.writes.locked();
        let in_label = |id: u32| {
            buffers
                .iter()
                .any(|(buffer, candidate, _)| *buffer == id && *candidate == label)
        };
        writes.iter().filter(|write| in_label(write.0)).count()
    }
}
