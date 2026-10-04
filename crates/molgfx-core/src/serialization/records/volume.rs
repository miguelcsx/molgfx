//! Scalar volume presentation serialization.

use super::super::types::{VolumeStyleDescription, VolumeTransferPointDescription};
use super::{region_description, rgba, volume_rendering};

pub(crate) fn volume_style_description(value: &crate::VolumeStyle) -> VolumeStyleDescription {
    VolumeStyleDescription {
        rendering: volume_rendering(value.rendering).to_owned(),
        transfer: value
            .transfer
            .points()
            .iter()
            .map(|point| VolumeTransferPointDescription {
                value: point.value,
                color: rgba(point.color),
                opacity: point.opacity,
            })
            .collect(),
        opacity_scale: value.opacity_scale,
        step_scale: value.step_scale,
        slice: value.slice.map(|slice| {
            [
                slice.plane.normal.x,
                slice.plane.normal.y,
                slice.plane.normal.z,
                slice.plane.offset,
            ]
        }),
        slice_ramp: value.slice_ramp.map(|ramp| {
            ramp.values()
                .iter()
                .zip(ramp.colors())
                .map(|(value, color)| VolumeTransferPointDescription {
                    value: *value,
                    color: rgba(*color),
                    opacity: 1.0,
                })
                .collect()
        }),
        region: value.region.map(region_description),
        iso_width_voxels: value.iso_width_voxels,
    }
}
