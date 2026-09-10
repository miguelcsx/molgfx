use super::*;
use crate::fixture;
use crate::representation::RepresentationKind;
use crate::selection::AtomSelection;
use crate::{ScalarVolume, SecondaryStructure};
use pdviewx_math::Mat4;
use std::sync::Arc;

fn scene() -> Scene {
    match Scene::from_structure(&fixture::structure()) {
        Ok(scene) => scene,
        Err(e) => panic!("fixture scene must build: {e}"),
    }
}

#[test]
fn a_scene_built_from_a_structure_exposes_its_atoms() {
    let s = scene();
    let Some(atoms) = s.first_atoms() else {
        panic!("scene has a structure")
    };
    assert_eq!(atoms.len(), 8);
}

#[test]
fn ordinary_scene_bounds_do_not_materialize_the_spatial_hierarchy() {
    let s = scene();
    let Some((_, placed)) = s.structures().next() else {
        panic!("scene has a structure")
    };
    assert!(!placed.spatial_bvh_is_ready());
    assert!(!s.world_aabb().is_empty());
    assert!(!placed.spatial_bvh_is_ready());
    let hierarchy = placed
        .spatial_bvh()
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(!hierarchy.nodes.is_empty());
    assert!(placed.spatial_bvh_is_ready());
}

#[test]
fn representing_a_selection_succeeds_for_supported_kinds_only() {
    let mut s = scene();
    let sel = s.add_selection(AtomSelection::All);
    assert!(s.represent(sel, RepresentationKind::Spacefill).is_ok());
    assert!(s.represent(sel, RepresentationKind::BallAndStick).is_ok());
    assert!(s.represent(sel, RepresentationKind::Lines).is_ok());
    assert!(s.represent(sel, RepresentationKind::Cartoon).is_ok());
    assert!(s.represent(sel, RepresentationKind::Trace).is_ok());
    assert!(s.represent(sel, RepresentationKind::Tube).is_ok());
    assert!(s.represent(sel, RepresentationKind::Surface).is_ok());
    assert!(s.represent(sel, RepresentationKind::Points).is_ok());
    let Err(err) = s.represent(sel, RepresentationKind::Volume) else {
        panic!("volume is not drawable yet")
    };
    assert_eq!(err.code(), "PDVIEWX-E0042");
}

#[test]
fn one_declaration_compiles_a_query_and_applies_the_representation_recipe() {
    let mut scene = scene();
    let color = crate::ColorScheme::Uniform(pdviewx_math::Rgba8::opaque(12, 34, 56));
    let recipe = crate::Representation::licorice()
        .color(color)
        .radius_scale(0.4);
    let handle = match scene.represent(crate::Select::protein(), recipe) {
        Ok(handle) => handle,
        Err(error) => panic!("declarative representation builds: {error}"),
    };
    let Some(view) = scene.representation(handle) else {
        panic!("representation resolves")
    };
    assert_eq!(view.kind, RepresentationKind::Licorice);
    assert_eq!(view.color, color);
    assert_eq!(view.params.radius_scale.to_bits(), 0.4_f32.to_bits());
}

#[test]
fn declarative_presets_reuse_canonical_representation_paths() {
    let mut scene = scene();
    let selection = scene.add_selection(AtomSelection::All);
    for (preset, kind) in [
        (
            crate::RepresentationPreset::Cpk,
            RepresentationKind::BallAndStick,
        ),
        (
            crate::RepresentationPreset::Licorice,
            RepresentationKind::Licorice,
        ),
        (
            crate::RepresentationPreset::PaperChain,
            RepresentationKind::PaperChain,
        ),
        (
            crate::RepresentationPreset::DottedSolvent,
            RepresentationKind::Surface,
        ),
    ] {
        let handles = match scene
            .represent_preset(crate::RepresentationTarget::Selection(selection), preset)
        {
            Ok(value) => value,
            Err(error) => panic!("preset applies: {error}"),
        };
        assert_eq!(handles.len(), 1);
        assert_eq!(
            scene.representation(handles[0]).map(|value| value.kind),
            Some(kind)
        );
    }
}

#[test]
fn signed_isosurface_preset_creates_two_colored_lobes() {
    let mut scene = Scene::new();
    let volume = match ScalarVolume::new(
        [2, 2, 2],
        Mat4::IDENTITY,
        Arc::from([-1.0, -0.5, 0.0, 0.5, 1.0, 0.5, 0.0, -0.5]),
    ) {
        Ok(value) => scene.add_volume(value),
        Err(error) => panic!("signed volume builds: {error}"),
    };
    let handles = match scene.represent_preset(
        crate::RepresentationTarget::Volume(volume),
        crate::RepresentationPreset::SignedIsosurface {
            negative_level: -0.4,
            positive_level: 0.4,
            negative_color: pdviewx_math::Rgba8::opaque(220, 40, 80),
            positive_color: pdviewx_math::Rgba8::opaque(40, 100, 230),
        },
    ) {
        Ok(value) => value,
        Err(error) => panic!("signed preset applies: {error}"),
    };
    assert_eq!(handles.len(), 2);
    assert_eq!(
        scene
            .representation(handles[0])
            .map(|value| value.params.isolevel),
        Some(-0.4)
    );
    assert_eq!(
        scene
            .representation(handles[1])
            .map(|value| value.params.isolevel),
        Some(0.4)
    );
}

#[test]
fn material_opacity_quantizes_and_classifies_transparency_deterministically() {
    let mut material = crate::Material::default();
    assert!(!material.is_translucent());
    assert_eq!(material.opacity_unorm8(), 255);
    material.opacity = 0.4;
    assert!(material.is_translucent());
    assert_eq!(material.opacity_unorm8(), 102);
    material.opacity = f32::NAN;
    assert!(!material.is_translucent());
    assert_eq!(material.opacity_unorm8(), 255);
}

#[test]
fn a_stale_selection_handle_is_refused_with_its_code() {
    let mut a = scene();
    let mut b = scene();
    let foreign = b.add_selection(AtomSelection::All);
    // Handle from another scene: same slot space shape, but scene `a` has no
    // entry there yet, so resolution fails.
    let Err(err) = a.represent(foreign, RepresentationKind::Spacefill) else {
        panic!("foreign handle must not resolve")
    };
    assert_eq!(err.code(), "PDVIEWX-E0041");
    let _ = b.representation_count();
}

#[test]
fn spatial_selection_uses_exact_distance_after_bvh_pruning() {
    let mut scene = scene();
    let reference = scene.add_selection(AtomSelection::Sparse(vec![1]));
    let within = match scene.select_within(reference, 1.6) {
        Ok(selection) => selection,
        Err(error) => panic!("spatial query succeeds: {error}"),
    };
    let Some(within) = scene.selection(within) else {
        panic!("result resolves")
    };
    assert_eq!(
        within.to_bitmap(8).iter().collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
}

#[test]
fn residue_spatial_selection_expands_each_hit_to_its_complete_residue() {
    let mut scene = scene();
    let reference = scene.add_selection(AtomSelection::Sparse(vec![1]));
    let within = match scene.select_residues_within(reference, 1.6) {
        Ok(selection) => selection,
        Err(error) => panic!("residue query succeeds: {error}"),
    };
    let Some(within) = scene.selection(within) else {
        panic!("result resolves")
    };
    assert_eq!(
        within.to_bitmap(8).iter().collect::<Vec<_>>(),
        (0..6).collect::<Vec<_>>()
    );
}

#[test]
fn molecular_component_filter_removes_disconnected_single_atom_dust() {
    let source = "data_components\n\
loop_\n_atom_site.group_PDB\n_atom_site.id\n_atom_site.type_symbol\n\
_atom_site.label_atom_id\n_atom_site.label_comp_id\n_atom_site.label_asym_id\n\
_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\n\
ATOM 1 C C1 LIG A 1 0 0 0\nATOM 2 C C2 LIG A 1 1 0 0\n\
HETATM 3 O O HOH W 2 5 0 0\nHETATM 4 ZN ZN ZN Z 3 9 0 0\n\
loop_\n_struct_conn.id\n_struct_conn.conn_type_id\n\
_struct_conn.ptnr1_label_asym_id\n_struct_conn.ptnr1_label_seq_id\n\
_struct_conn.ptnr1_label_comp_id\n_struct_conn.ptnr1_label_atom_id\n\
_struct_conn.ptnr2_label_asym_id\n_struct_conn.ptnr2_label_seq_id\n\
_struct_conn.ptnr2_label_comp_id\n_struct_conn.ptnr2_label_atom_id\n\
_struct_conn.pdbx_value_order\n1 covale A 1 LIG C1 A 1 LIG C2 SING\n";
    let structure = match pdbiox::read_bytes(
        source.as_bytes().to_vec(),
        Some("components.cif"),
        &pdbiox::ReadOptions::default(),
    ) {
        Ok((structure, _)) => structure,
        Err(error) => panic!("component fixture parses: {error:?}"),
    };
    let mut scene = match Scene::from_structure(&structure) {
        Ok(scene) => scene,
        Err(error) => panic!("component scene builds: {error}"),
    };
    let all = scene.add_selection(AtomSelection::All);
    let retained = match scene.select_molecular_components(all, 2) {
        Ok(selection) => selection,
        Err(error) => panic!("component selection applies: {error}"),
    };
    let Some(retained) = scene.selection(retained) else {
        panic!("component selection resolves")
    };
    assert_eq!(retained.to_bitmap(4).iter().collect::<Vec<_>>(), vec![0, 1]);
    let Err(error) = scene.select_molecular_components(all, 0) else {
        panic!("zero-sized component threshold must fail")
    };
    assert_eq!(error.code(), "PDVIEWX-E0034");
}

#[test]
fn water_selection_uses_declared_entity_kind_without_name_heuristics() {
    let source = "\
data_water
loop_
_entity.id
_entity.type
1 polymer
2 water
loop_
_struct_asym.id
_struct_asym.entity_id
A 1
W 2
loop_
_atom_site.group_PDB
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_entity_id
_atom_site.label_seq_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
ATOM 1 O O HOH A 1 1 0 0 0
HETATM 2 O O SOL W 2 . 1 0 0
";
    let structure = match pdbiox::read_bytes(
        source.as_bytes().to_vec(),
        Some("water.cif"),
        &pdbiox::ReadOptions::default(),
    ) {
        Ok((structure, _)) => structure,
        Err(diagnostics) => panic!("water fixture parses: {diagnostics:?}"),
    };
    let mut scene = match Scene::from_structure(&structure) {
        Ok(scene) => scene,
        Err(error) => panic!("water scene builds: {error}"),
    };
    let water = scene.select_water();
    let selected = scene
        .selection(water)
        .map(|selection| selection.to_bitmap(2).iter().collect::<Vec<_>>());
    assert_eq!(selected, Some(vec![1]));
}

#[test]
fn structure_scoped_spatial_selection_never_leaks_equal_row_ids() {
    let source = fixture::structure();
    let mut scene = Scene::new();
    let first = match scene.add_structure(&source) {
        Ok(handle) => handle,
        Err(error) => panic!("first fixture places: {error}"),
    };
    let second = match scene.add_structure(&source) {
        Ok(handle) => handle,
        Err(error) => panic!("second fixture places: {error}"),
    };
    let reference = match scene.add_structure_selection(first, AtomSelection::Sparse(vec![1])) {
        Ok(selection) => selection,
        Err(error) => panic!("scoped selection stores: {error}"),
    };
    let within = match scene.select_within(reference, 1.6) {
        Ok(selection) => selection,
        Err(error) => panic!("spatial query succeeds: {error}"),
    };
    let Some(first_result) = scene.selection_for(within, first) else {
        panic!("first structure result resolves")
    };
    assert_eq!(
        first_result.to_bitmap(8).iter().collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert!(scene.selection_for(within, second).is_none());
}

#[test]
fn malformed_spatial_distance_is_a_stable_typed_error() {
    let mut scene = scene();
    let reference = scene.add_selection(AtomSelection::All);
    let Err(error) = scene.select_within(reference, f32::NAN) else {
        panic!("non-finite distance is refused")
    };
    assert_eq!(error.code(), "PDVIEWX-E0034");
}

#[test]
fn representation_edits_bump_the_revision_and_reads_do_not() {
    let mut s = scene();
    let sel = s.add_selection(AtomSelection::All);
    let Ok(rep) = s.represent(sel, RepresentationKind::Spacefill) else {
        panic!("spacefill applies")
    };
    let after_add = s.representation_revision();
    let _ = s.representation(rep);
    let _ = s.representations().count();
    assert_eq!(s.representation_revision(), after_add);
    s.hide(rep);
    assert!(s.representation_revision() > after_add);
}

#[test]
fn removing_a_structure_stales_its_handle() {
    let mut s = Scene::new();
    let Ok(h) = s.add_structure(&fixture::structure()) else {
        panic!("fixture places")
    };
    assert!(s.structure(h).is_some());
    assert!(s.remove_structure(h).is_some());
    assert!(s.structure(h).is_none());
}

#[test]
fn the_world_bound_covers_the_placed_structure() {
    let s = scene();
    let aabb = s.world_aabb();
    assert!(!aabb.is_empty());
    // The sulfate sulfur sits at x = 8; the bound must reach it.
    assert!(aabb.max.x >= 8.0);
}

#[test]
fn pdbiox_secondary_structure_replaces_the_reversible_residue_column() {
    let mut scene = scene();
    let Some((handle, _)) = scene.structures().next() else {
        panic!("fixture structure exists")
    };
    let records = [(pdbiox::ResidueIndex::new(1), SecondaryStructure::Strand)];
    if let Err(error) = scene.apply_secondary_structure(handle, &records) {
        panic!("secondary structure applies: {error}")
    }
    let Some(placed) = scene.structure(handle) else {
        panic!("structure resolves")
    };
    assert_eq!(
        placed.secondary_structure.values(),
        &[
            SecondaryStructure::Coil,
            SecondaryStructure::Strand,
            SecondaryStructure::Coil
        ]
    );
}

#[test]
fn a_density_volume_has_a_dedicated_representation_target_and_world_bound() {
    let mut scene = Scene::new();
    let Ok(volume) = ScalarVolume::new([2, 2, 2], Mat4::IDENTITY, Arc::from([0.0; 8])) else {
        panic!("volume builds")
    };
    let volume = scene.add_volume(volume);
    let Ok(representation) = scene.represent(volume, crate::Representation::volume()) else {
        panic!("volume is representable")
    };
    let Some(representation) = scene.representation(representation) else {
        panic!("representation resolves")
    };
    assert_eq!(representation.volume_handle(), Some(volume));
    assert!(representation.selection().is_none());
    assert_eq!(scene.world_aabb().max, pdviewx_math::Vec3::ONE);
}

#[test]
fn direct_volume_and_isosurface_share_one_stored_grid() {
    let mut scene = Scene::new();
    let volume = match ScalarVolume::new(
        [2, 2, 2],
        Mat4::IDENTITY,
        Arc::from([0.0, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0]),
    ) {
        Ok(volume) => volume,
        Err(error) => panic!("volume builds: {error}"),
    };
    let volume = scene.add_volume(volume);
    let direct = match scene.represent(volume, crate::Representation::volume()) {
        Ok(handle) => handle,
        Err(error) => panic!("direct volume applies: {error}"),
    };
    let surface = match scene.represent(
        volume,
        crate::Representation::volume().volume_style(crate::VolumeStyle::isosurface()),
    ) {
        Ok(handle) => handle,
        Err(error) => panic!("isosurface applies: {error}"),
    };
    assert_eq!(
        scene
            .representation(direct)
            .map(|view| view.volume.rendering),
        Some(crate::VolumeRendering::Direct)
    );
    assert_eq!(
        scene
            .representation(surface)
            .map(|view| view.volume.rendering),
        Some(crate::VolumeRendering::Isosurface)
    );
    assert_eq!(scene.volume_content_revision(volume), Some(0));
}

#[test]
fn liquid_surface_reuses_one_stored_grid_with_a_distinct_presentation_mode() {
    let mut scene = Scene::new();
    let volume = match ScalarVolume::new([2, 2, 2], Mat4::IDENTITY, Arc::from([0.5; 8])) {
        Ok(volume) => volume,
        Err(error) => panic!("liquid volume builds: {error}"),
    };
    let volume = scene.add_volume(volume);
    let representation = match scene.represent(
        volume,
        crate::Representation::volume().volume_style(crate::VolumeStyle::liquid_surface()),
    ) {
        Ok(handle) => handle,
        Err(error) => panic!("liquid surface applies: {error}"),
    };
    let Some(value) = scene.representation(representation) else {
        panic!("liquid representation resolves")
    };
    assert_eq!(
        value.volume.rendering,
        crate::VolumeRendering::LiquidSurface
    );
    assert_eq!(scene.volume_content_revision(volume), Some(0));
}
