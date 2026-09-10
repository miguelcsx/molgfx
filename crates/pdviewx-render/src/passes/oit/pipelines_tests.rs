use super::{segmentation_pipeline_constants, volume_pipeline_constants};
use crate::scene_gpu::SegmentationPipelineKey;
use pdviewx_core::VolumeRendering;

#[test]
fn every_volume_algorithm_has_its_own_specialization_value() {
    let modes = [
        VolumeRendering::Direct,
        VolumeRendering::Isosurface,
        VolumeRendering::Medium,
        VolumeRendering::Slice,
        VolumeRendering::LiquidSurface,
    ];
    for (mode, expected) in modes.into_iter().zip([0.0, 1.0, 2.0, 3.0, 4.0]) {
        assert_eq!(
            volume_pipeline_constants(mode),
            [("VOLUME_RENDER_MODE", expected)]
        );
    }
}

#[test]
fn categorical_slice_and_lookup_modes_specialize_independently() {
    let cases = [
        (SegmentationPipelineKey::Direct, [0.0, 0.0]),
        (SegmentationPipelineKey::DirectSlice, [1.0, 0.0]),
        (SegmentationPipelineKey::Hash, [0.0, 1.0]),
        (SegmentationPipelineKey::HashSlice, [1.0, 1.0]),
    ];
    for (key, expected) in cases {
        assert_eq!(
            segmentation_pipeline_constants(key),
            [
                ("SEGMENTATION_SLICE_MODE", expected[0]),
                ("SEGMENTATION_HASH_LOOKUP", expected[1]),
            ]
        );
    }
}
