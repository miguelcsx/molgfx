use super::OccupancyBoundsFormat;
use molgfx_gpu::{Capabilities, TextureFormatCapabilities};

fn storage_texture_capabilities() -> Capabilities {
    Capabilities {
        r32float: TextureFormatCapabilities {
            sampled: true,
            storage_write: true,
        },
        rg32float: TextureFormatCapabilities {
            sampled: true,
            storage_write: true,
        },
        rgba32float: TextureFormatCapabilities {
            sampled: true,
            storage_write: true,
        },
        ..Capabilities::default()
    }
}

#[test]
fn occupancy_selects_native_bounds_when_rg32_storage_is_supported() {
    assert_eq!(
        OccupancyBoundsFormat::resolve(&storage_texture_capabilities()).ok(),
        Some(OccupancyBoundsFormat::Rg32Float)
    );
}

#[test]
fn occupancy_falls_back_to_rgba32_bounds_when_rg32_storage_is_unavailable() {
    let mut capabilities = storage_texture_capabilities();
    capabilities.rg32float.storage_write = false;

    assert_eq!(
        OccupancyBoundsFormat::resolve(&capabilities).ok(),
        Some(OccupancyBoundsFormat::Rgba32Float)
    );
}

#[test]
fn occupancy_rejects_only_the_requested_feature_without_bounds_storage() {
    let mut capabilities = storage_texture_capabilities();
    capabilities.rg32float.storage_write = false;
    capabilities.rgba32float.storage_write = false;

    let error = OccupancyBoundsFormat::resolve(&capabilities).unwrap_err();
    assert_eq!(
        error.to_string(),
        "capability \"temporal occupancy bounds sampled storage texture\" unavailable"
    );
}

#[test]
fn occupancy_requires_the_density_storage_texture_before_creating_resources() {
    let mut capabilities = storage_texture_capabilities();
    capabilities.r32float.storage_write = false;

    let error = OccupancyBoundsFormat::resolve(&capabilities).unwrap_err();
    assert_eq!(
        error.to_string(),
        "capability \"temporal occupancy r32float sampled storage texture\" unavailable"
    );
}
