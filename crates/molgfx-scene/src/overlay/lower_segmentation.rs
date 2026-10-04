//! Categorical-grid lowering retains labels independently of display edits.

use super::{OverlayBindings, SegmentationSpec};
use crate::Error;
use molgfx_core::{Representation, Scene, SegmentationHandle, SegmentationStyle};

pub(super) fn lower(
    scene: &mut Scene,
    spec: &SegmentationSpec,
    bindings: &OverlayBindings,
) -> Result<Option<SegmentationHandle>, Error> {
    let Some(binding) = bindings.segmentation(&spec.source.content_hash) else {
        return Ok(None);
    };
    binding.matches(spec)?;
    let grid = scene.add_segmented_volume(binding.native());
    let styles = super::segmentation_spec::native_styles(&spec.styles)?;
    let visible = spec
        .styles
        .iter()
        .any(|style| style.visible && style.opacity > 0.0 && style.color.0[3] > 0);
    let representation = scene.represent(
        grid,
        Representation::segmentation().segmentation_style(SegmentationStyle {
            presentation: spec.presentation,
            styles,
            ..SegmentationStyle::default()
        }),
    )?;
    scene
        .representation_mut(representation)
        .ok_or(Error::MissingId)?
        .visible = visible;
    Ok(Some(grid))
}
