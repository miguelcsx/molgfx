//! Deterministic frame sequences driven by a multi-track timeline.

use super::timeline_sequence::{FrameSchedule, SequenceManifest};
use super::{Image, Renderer};
use crate::{Camera, Error, Scene};
use molgfx_core::Timeline;

impl Renderer {
    /// Renders the frames of `schedule`, each at its exact playback time.
    ///
    /// Frame `i` is shown at `i / fps` seconds, computed from the index in
    /// `f64`. For each frame the timeline is applied to the scene, then one
    /// image of the schedule's exposure is rendered with `camera` and handed to
    /// `on_frame` with its frame index, so a long sequence streams to disk
    /// instead of accumulating in memory. The same timeline and schedule give
    /// the same frames. The returned manifest records every frame's playback
    /// time and each track's physical time; write it beside the frames with
    /// [`SequenceManifest::write_json`].
    ///
    /// Encoding is deliberately not here: encode the frames with whatever tool
    /// the caller already uses, for example `ffmpeg -framerate FPS -i
    /// frame_%06d.png`.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error for an empty target, a frame
    /// rate that is not finite and positive, or a range with no frame; the
    /// error of `on_frame`; and a typed timeline, renderer or device error
    /// otherwise.
    pub fn render_timeline(
        &mut self,
        scene: &mut Scene,
        timeline: &mut Timeline,
        camera: &Camera,
        size: (u32, u32),
        schedule: &FrameSchedule,
        mut on_frame: impl FnMut(u64, Image) -> Result<(), Error>,
    ) -> Result<SequenceManifest, Error> {
        if size.0 == 0 || size.1 == 0 {
            return Err(Error::InvalidSpec(
                "frame width and height must be non-zero".to_owned(),
            ));
        }
        let mut manifest = SequenceManifest::new(schedule, timeline)?;
        for index in schedule.range.clone() {
            scene.apply_timeline(timeline, schedule.playback_seconds(index)?)?;
            manifest.record(timeline)?;
            let image = self.render_output_with_camera(scene, camera, size, schedule.quality)?;
            on_frame(index, image)?;
        }
        Ok(manifest)
    }
}
