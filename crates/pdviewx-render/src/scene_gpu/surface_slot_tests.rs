use super::*;
use crate::testing::MockDevice;

const DIMENSIONS: [u32; 3] = [17, 11, 7];
const BYTES_PER_TEXEL: u64 = 4;

fn allocated_bytes(device: &MockDevice) -> u64 {
    let extents = device
        .log
        .texture_extents
        .lock()
        .unwrap_or_else(|error| panic!("texture log locks: {error}"));
    extents
        .iter()
        .map(|(_, extent)| {
            extent.iter().map(|axis| u64::from(*axis)).product::<u64>() * BYTES_PER_TEXEL
        })
        .sum()
}

#[test]
fn field_texture_residency_matches_each_grid_surface_kind() {
    let one_texture_bytes = DIMENSIONS
        .iter()
        .map(|axis| u64::from(*axis))
        .product::<u64>()
        * BYTES_PER_TEXEL;
    for (kind, expected_textures) in [
        (SurfaceKind::SolventAccessible, 2_u64),
        (SurfaceKind::SolventExcluded, 3),
        (SurfaceKind::Gaussian, 2),
    ] {
        let device = MockDevice::default();
        let mut slot = SurfaceSlot::<MockDevice>::new();
        slot.kind = kind;
        slot.ensure_field(&device, DIMENSIONS)
            .unwrap_or_else(|error| panic!("field allocates: {error}"));
        let extents = device
            .log
            .texture_extents
            .lock()
            .unwrap_or_else(|error| panic!("texture log locks: {error}"));
        assert_eq!(extents.len() as u64, expected_textures, "{kind:?}");
        assert!(extents.iter().all(|(_, extent)| *extent == DIMENSIONS));
        drop(extents);
        assert_eq!(
            allocated_bytes(&device),
            one_texture_bytes * expected_textures,
            "{kind:?} retains only its full-resolution field textures"
        );
    }
}

#[test]
fn leaving_ses_releases_the_eroded_field_and_probe_buffer() {
    let device = MockDevice::default();
    let mut slot = SurfaceSlot::<MockDevice>::new();
    slot.kind = SurfaceKind::SolventExcluded;
    slot.ensure_field(&device, DIMENSIONS)
        .unwrap_or_else(|error| panic!("SES field allocates: {error}"));
    assert!(slot.field.is_some());

    slot.kind = SurfaceKind::Gaussian;
    slot.ensure_field(&device, DIMENSIONS)
        .unwrap_or_else(|error| panic!("Gaussian field reuses primary textures: {error}"));
    assert!(slot.inflated.is_some());
    assert!(slot.field.is_none());
    assert!(slot.erosion.is_none());
    assert!(slot.erosion_offsets.is_none());
    assert_eq!(slot.erosion_offsets_capacity, 0);
}

#[test]
fn scalar_fields_and_compact_normals_use_their_shader_formats() {
    let device = MockDevice::default();
    let mut slot = SurfaceSlot::<MockDevice>::new();
    slot.kind = SurfaceKind::SolventExcluded;
    slot.ensure_field(&device, DIMENSIONS)
        .unwrap_or_else(|error| panic!("SES field allocates: {error}"));

    let formats = device
        .log
        .texture_formats
        .lock()
        .unwrap_or_else(|error| panic!("texture format log locks: {error}"));
    assert!(formats.contains(&("probe-inflated surface field", TextureFormat::R32Float)));
    assert!(formats.contains(&("solvent-excluded surface field", TextureFormat::R32Float)));
    assert!(formats.contains(&("continuous surface normals", TextureFormat::Rgba8Snorm)));
}
