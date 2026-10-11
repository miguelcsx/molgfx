use super::{LOD_ATOM_THRESHOLD, RepresentationKind, lod_mode};

const ABOVE: u32 = LOD_ATOM_THRESHOLD + 1;

#[test]
fn a_cartoon_above_the_threshold_is_never_tile_culled() {
    // Tile-depth LOD keeps one nearest atom per screen tile. A ribbon is built
    // from neighbouring atoms, so culling them erased it: cartoons of 130k+
    // atoms drew nothing at all in the adaptive realtime path.
    assert_eq!(lod_mode(ABOVE, 0, RepresentationKind::Cartoon, true), 0);
    assert_eq!(lod_mode(ABOVE, ABOVE, RepresentationKind::Cartoon, true), 0);
}

#[test]
fn a_surface_above_the_threshold_is_never_tile_culled() {
    assert_eq!(lod_mode(ABOVE, 0, RepresentationKind::Surface, true), 0);
}

#[test]
fn atom_forms_still_use_lod_above_the_threshold() {
    assert_eq!(lod_mode(ABOVE, 0, RepresentationKind::Spacefill, true), 1);
    assert_eq!(
        lod_mode(ABOVE, 10, RepresentationKind::BallAndStick, true),
        1
    );
}

#[test]
fn point_clouds_without_bonds_keep_the_wide_index_path() {
    assert_eq!(lod_mode(ABOVE, 0, RepresentationKind::Points, true), 2);
}

#[test]
fn nothing_uses_lod_below_the_threshold_or_when_disabled() {
    assert_eq!(
        lod_mode(
            LOD_ATOM_THRESHOLD - 1,
            0,
            RepresentationKind::Spacefill,
            true
        ),
        0
    );
    assert_eq!(lod_mode(ABOVE, 0, RepresentationKind::Spacefill, false), 0);
}
