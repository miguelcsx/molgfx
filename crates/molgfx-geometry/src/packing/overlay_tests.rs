use super::*;
use crate::{PropertyColumns, RibbonColoring, RibbonVertex, recolor_ribbon_with_appearance};
use molgfx_core::{
    AtomPropertyMeaning, EntityId, EntityKind, MolecularSource, ScalarFieldSemantics, Scene,
};
use molgfx_math::Rgba8;
use std::sync::Arc;

const CIF: &str = "\
data_test
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
ATOM 1 C CA GLY A 1 1 0 0 0
ATOM 2 C CA GLY A 1 2 3.8 0 0
ATOM 3 C CA GLY B 1 1 7.6 0 0
";

fn vertex(atom: u64) -> RibbonVertex {
    let Ok(entity) = EntityId::pack(EntityKind::Atom, atom) else {
        panic!("atom entity packs")
    };
    RibbonVertex {
        position: [0.0; 3],
        entity_id: entity.0,
        normal: [0.0, 0.0, 1.0],
        color: Rgba8::opaque(0, 0, 0),
    }
}

#[test]
fn only_classed_atoms_take_the_overriding_scheme() {
    let Ok((structure, _)) = molframe::read_bytes(
        CIF.as_bytes().to_vec(),
        Some("t.cif"),
        &molframe::ReadOptions::new(),
    ) else {
        panic!("fixture parses")
    };
    let mut scene = Scene::new();
    let Ok(handle) = scene.add_source(MolecularSource::from_molframe(&structure)) else {
        panic!("fixture binds")
    };
    let Ok(classes) = molgfx_core::AtomProperty::new(
        handle,
        Arc::<str>::from("classes"),
        vec![0.0, 1.0, 0.0].into(),
        AtomPropertyMeaning::Generic,
        ScalarFieldSemantics::UncalibratedRank,
    ) else {
        panic!("class column builds")
    };
    let Ok(column) = scene.add_atom_property(classes.clone()) else {
        panic!("class column binds")
    };
    let Some(placed) = scene.structure(handle) else {
        panic!("structure is placed")
    };
    let red = Rgba8::opaque(255, 0, 0);
    let blue = Rgba8::opaque(0, 0, 255);
    let Ok(overlay) = ColorOverlay::new(column, &[ColorScheme::Uniform(red)]) else {
        panic!("overlay builds")
    };
    let mut vertices = [vertex(0), vertex(1), vertex(2)];
    recolor_ribbon_with_appearance(
        &mut vertices,
        &placed.atoms,
        &placed.hierarchy,
        placed.secondary_structure.values(),
        PropertyColumns {
            color: None,
            appearance: None,
            overlay: Some(OverlayColumn::new(overlay, &classes)),
        },
        RibbonColoring {
            color: ColorScheme::Uniform(blue),
            appearance: None,
            opacity: u8::MAX,
        },
    );
    let colors = vertices.map(|vertex| vertex.color);
    assert_eq!(colors, [blue, red, blue]);
}
