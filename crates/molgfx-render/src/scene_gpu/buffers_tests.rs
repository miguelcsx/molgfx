use super::*;

#[test]
fn growth_uses_the_device_ceiling_instead_of_crossing_it() {
    let needed = (256 << 20) + 1;
    let limit = 300 << 20;

    assert!(matches!(
        grow_capacity(needed, limit, "test buffer"),
        Ok(value) if value == limit
    ));
}

#[test]
fn growth_rejects_records_that_cannot_fit_one_storage_binding() {
    let limit = 128 << 20;

    assert!(matches!(
        grow_capacity(limit + 1, limit, "test buffer"),
        Err(RenderError::Gpu(molgfx_gpu::GpuError::LimitExceeded {
            resource: "test buffer",
            limit: value
        })) if value == limit
    ));
}

#[test]
fn the_cull_buffer_holds_a_depth_plane_only_for_scenes_that_use_one() {
    let extent: [u32; 2] = [1920, 1080];
    let tiles = extent[0].div_ceil(8) * extent[1].div_ceil(8);
    assert_eq!(cull_tiles_bytes(extent, tiles, false), u64::from(tiles) * 4);
    let shadow = u64::from(SHADOW_MAP_EXTENT) * u64::from(SHADOW_MAP_EXTENT);
    assert_eq!(
        cull_tiles_bytes(extent, tiles, true),
        (1920 * 1080 + shadow) * 4
    );
}

#[test]
fn the_cull_depth_plane_stays_within_one_storage_binding() {
    let bytes = cull_tiles_bytes([16_384, 16_384], 2048 * 2048, true);
    assert_eq!(bytes, 128 << 20);
    assert_eq!(cull_tiles_bytes([0, 0], 0, false), 256);
}
