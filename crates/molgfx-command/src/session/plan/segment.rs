//! Categorical-grid additions through the scene's identity allocator.

use super::{Planner, scene_error};
use crate::error::CommandError;
use molgfx_scene::SegmentationSpec;

impl Planner<'_> {
    pub(super) fn segment(&mut self, segmentation: SegmentationSpec) -> Result<(), CommandError> {
        let id = self
            .transaction
            .add_segmentation(segmentation)
            .map_err(|error| scene_error(&error))?;
        self.messages
            .push(format!("segmentation {}:{} added", id.index, id.generation));
        Ok(())
    }
}
