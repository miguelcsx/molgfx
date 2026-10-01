use super::{StructureSize, auto_representations};
use crate::StructureId;
use molgfx_core::MolecularSource;
use std::fmt::Write as _;

#[test]
fn residue_counts_map_to_the_four_size_classes() {
    assert_eq!(
        StructureSize::from_polymer_residues(0),
        StructureSize::Small
    );
    assert_eq!(
        StructureSize::from_polymer_residues(9),
        StructureSize::Small
    );
    assert_eq!(
        StructureSize::from_polymer_residues(10),
        StructureSize::Medium
    );
    assert_eq!(
        StructureSize::from_polymer_residues(4_999),
        StructureSize::Medium
    );
    assert_eq!(
        StructureSize::from_polymer_residues(5_000),
        StructureSize::Large
    );
    assert_eq!(
        StructureSize::from_polymer_residues(29_999),
        StructureSize::Large
    );
    assert_eq!(
        StructureSize::from_polymer_residues(30_000),
        StructureSize::Huge
    );
}

#[test]
fn size_classes_are_ordered_by_cost() {
    assert!(StructureSize::Small < StructureSize::Medium);
    assert!(StructureSize::Large < StructureSize::Huge);
}

fn structure_of(pdb: &str) -> molframe::Structure {
    let result = molframe::read_bytes(
        pdb.as_bytes().to_vec(),
        Some("fixture.pdb"),
        &molframe::ReadOptions::new(),
    );
    let Ok((structure, _)) = result else {
        panic!("fixture must parse")
    };
    structure
}

fn source_of(pdb: &str) -> MolecularSource {
    let result = molframe::read_bytes(
        pdb.as_bytes().to_vec(),
        Some("fixture.pdb"),
        &molframe::ReadOptions::new(),
    );
    let Ok((structure, _)) = result else {
        panic!("fixture must parse")
    };
    MolecularSource::from_molframe(&structure)
}

fn ligand_source() -> MolecularSource {
    source_of(
        "HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00  0.00           C\n\
HETATM    2  O1  LIG A   1       1.200   0.000   0.000  1.00  0.00           O\nEND\n",
    )
}

#[test]
fn a_ligand_without_polymer_is_drawn_at_atomic_detail() {
    let source = ligand_source();
    let Ok(forms) = auto_representations(&source, StructureId(1)) else {
        panic!("the ligand selects");
    };
    assert_eq!(forms.len(), 1);
    assert!(forms[0].explain().to_lowercase().contains("ball"));
}

/// A straight poly-alanine backbone of `residues` residues plus one ligand.
fn peptide_with_ligand(residues: u32) -> MolecularSource {
    source_of(&peptide_pdb(residues, 20.0))
}

fn peptide_pdb(residues: u32, ligand_y: f64) -> String {
    let mut pdb = String::new();
    let mut serial = 0_u32;
    for residue in 0..residues {
        let base = f64::from(residue) * 3.8;
        for (name, element, offset) in [
            ("N", "N", 0.0),
            ("CA", "C", 1.46),
            ("C", "C", 2.5),
            ("O", "O", 3.0),
        ] {
            serial += 1;
            let _ = writeln!(
                pdb,
                "ATOM  {serial:>5}  {name:<3} ALA A{:>4}    {:>8.3}{:>8.3}{:>8.3}  1.00  0.00          {element:>2}",
                residue + 1,
                base + offset,
                0.0,
                0.0,
            );
        }
    }
    serial += 1;
    let _ = write!(
        pdb,
        "HETATM{serial:>5}  C1  LIG B 900    {:>8.3}{:>8.3}{:>8.3}  1.00  0.00           C\nEND\n",
        0.0, ligand_y, 0.0
    );
    pdb
}

#[test]
fn a_protein_with_a_ligand_gets_a_cartoon_and_atomic_detail_for_the_rest() {
    let source = peptide_with_ligand(12);
    let Ok(forms) = auto_representations(&source, StructureId(1)) else {
        panic!("the peptide selects");
    };
    let kinds: Vec<String> = forms
        .iter()
        .map(|form| form.explain().to_lowercase())
        .collect();
    assert_eq!(forms.len(), 2, "{kinds:?}");
    assert!(kinds[0].contains("cartoon"), "{kinds:?}");
    assert!(kinds[1].contains("ball"), "{kinds:?}");
}

fn selected_atoms(source: &MolecularSource, form: &crate::RepresentationSpec) -> Vec<u32> {
    let selection = form
        .common
        .target
        .compiled()
        .unwrap_or_else(|e| panic!("{e}"));
    let rows = u32::try_from(source.coordinates().len()).unwrap_or(u32::MAX);
    let mut atoms = Vec::new();
    source
        .select_compiled(&selection)
        .unwrap_or_else(|e| panic!("{e}"))
        .for_each(rows, |atom| atoms.push(atom));
    atoms
}

#[test]
fn the_pocket_bands_partition_the_structure_around_the_focus() {
    let pdb = peptide_pdb(12, 3.0);
    let source = source_of(&pdb);
    let focus = crate::Selection::from("resname LIG");
    let Ok(forms) =
        super::pocket_representations(&focus, StructureId(1), super::PocketStyle::default())
    else {
        panic!("the pocket composes");
    };
    assert_eq!(forms.len(), 6);
    let [context, mid, pocket, near, solvent, subject] =
        [0, 1, 2, 3, 4, 5].map(|i| selected_atoms(&source, &forms[i]));
    let total = u32::try_from(source.coordinates().len()).unwrap_or(0);
    assert_eq!(subject.len(), 1);
    assert!(
        !near.is_empty() && !context.is_empty(),
        "near {near:?} context {context:?}"
    );
    assert!(solvent.is_empty(), "no water in the fixture");
    // Focus, interaction shell, orienting shell and far context are disjoint
    // and cover every atom: each atom has exactly one band.
    let mut all: Vec<u32> = [&subject, &near, &mid, &context]
        .into_iter()
        .flatten()
        .copied()
        .collect();
    all.sort_unstable();
    assert_eq!(all, (0..total).collect::<Vec<_>>());
    assert!(pocket.iter().all(|atom| !subject.contains(atom)));
}

#[test]
fn a_pocket_style_with_unordered_distances_or_bad_opacity_is_rejected() {
    let focus = crate::Selection::from("resname LIG");
    let unordered = super::PocketStyle {
        near: 8.0,
        mid: 4.0,
        ..super::PocketStyle::default()
    };
    assert!(super::pocket_representations(&focus, StructureId(1), unordered).is_err());
    let opaque = super::PocketStyle {
        solvent_opacity: 1.5,
        ..super::PocketStyle::default()
    };
    assert!(super::pocket_representations(&focus, StructureId(1), opaque).is_err());
}

#[test]
fn a_scene_adds_the_pocket_in_one_call_and_rejects_an_empty_focus() {
    let structure = structure_of(&peptide_pdb(12, 3.0));
    let mut scene = crate::Scene::from_structure(&structure).unwrap_or_else(|e| panic!("{e}"));
    let ids = scene
        .add_pocket(StructureId(1), "resname LIG", super::PocketStyle::default())
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(ids.len(), 6);
    assert!(
        scene
            .add_pocket(
                StructureId(1),
                "resname NOPE",
                super::PocketStyle::default()
            )
            .is_err()
    );
}

fn member(structure: u64, weight: f32) -> super::EnsembleMember {
    super::EnsembleMember {
        structure: StructureId(structure),
        weight,
        color: crate::Color::rgb(10, 20, 30),
    }
}

fn opacities(forms: &[crate::RepresentationSpec]) -> Vec<f32> {
    forms
        .iter()
        .map(crate::RepresentationSpec::opacity)
        .collect()
}

#[test]
fn an_ensemble_draws_the_heaviest_member_most_opaque() {
    let members = [member(1, 1.0), member(2, 3.0), member(3, 0.0)];
    let style = super::EnsembleStyle::default();
    let Ok(forms) = super::ensemble_representations(&members, style) else {
        panic!("a valid ensemble builds");
    };
    assert_eq!(forms.len(), 3);
    let drawn = opacities(&forms);
    assert_eq!(drawn[1].to_bits(), style.dominant_opacity.to_bits());
    let light = style.alternate_opacity / 3.0;
    assert_eq!(drawn[0].to_bits(), light.to_bits());
    assert_eq!(drawn[2].to_bits(), style.minimum_opacity.to_bits());
}

#[test]
fn equal_weights_make_the_first_member_dominant() {
    let members = [member(1, 2.0), member(2, 2.0)];
    let style = super::EnsembleStyle::default();
    let Ok(forms) = super::ensemble_representations(&members, style) else {
        panic!("a valid ensemble builds");
    };
    let drawn = opacities(&forms);
    assert_eq!(drawn[0].to_bits(), style.dominant_opacity.to_bits());
    assert_eq!(drawn[1].to_bits(), style.alternate_opacity.to_bits());
}

#[test]
fn an_invalid_ensemble_is_rejected() {
    let style = super::EnsembleStyle::default();
    assert!(super::ensemble_representations(&[], style).is_err());
    assert!(super::ensemble_representations(&[member(1, 0.0)], style).is_err());
    assert!(super::ensemble_representations(&[member(1, -1.0)], style).is_err());
    assert!(super::ensemble_representations(&[member(1, f32::NAN)], style).is_err());
    assert!(super::ensemble_representations(&[member(1, 1.0), member(1, 2.0)], style).is_err());
    let inverted = super::EnsembleStyle {
        minimum_opacity: 0.9,
        ..style
    };
    assert!(super::ensemble_representations(&[member(1, 1.0)], inverted).is_err());
}

#[test]
fn a_scene_overlays_known_structures_and_leaves_itself_unchanged_on_a_bad_member() {
    let structure = structure_of(&peptide_pdb(6, 20.0));
    let mut scene = crate::Scene::from_structure(&structure).unwrap_or_else(|e| panic!("{e}"));
    let second = scene
        .add_source(&source_of(&peptide_pdb(6, 20.0)))
        .unwrap_or_else(|e| panic!("{e}"));
    let before = scene.spec().representations.len();
    let unknown = [member(1, 1.0), member(second.0, 1.0), member(99, 1.0)];
    assert!(
        scene
            .add_ensemble(&unknown, super::EnsembleStyle::default())
            .is_err()
    );
    assert_eq!(scene.spec().representations.len(), before);
    let ids = scene
        .add_ensemble(
            &[member(1, 2.0), member(second.0, 1.0)],
            super::EnsembleStyle::default(),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(ids.len(), 2);
}

fn bound_property(scene: &mut crate::Scene, rows: usize) -> crate::ScalarProperty {
    let values: std::sync::Arc<[f32]> = vec![0.5; rows].into();
    scene
        .bind_property(crate::ScalarPropertyBinding::new(
            StructureId(1),
            "delta",
            crate::DataSource::new("delta-source"),
            values,
        ))
        .unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn a_difference_cartoon_is_coloured_by_the_bound_property() {
    let structure = structure_of(&peptide_pdb(6, 20.0));
    let mut scene = crate::Scene::from_structure(&structure).unwrap_or_else(|e| panic!("{e}"));
    let rows = usize::try_from(structure.atom_count()).unwrap_or_else(|e| panic!("{e}"));
    let property = bound_property(&mut scene, rows);
    let style = super::DifferenceStyle::default();
    let id = scene
        .add_difference(StructureId(1), "all", property, &style)
        .unwrap_or_else(|e| panic!("{e}"));
    let spec = scene.spec();
    let Some(added) = spec.representations.get(&id) else {
        panic!("the representation was added");
    };
    let plain = crate::rep::cartoon("all").structure(StructureId(1));
    assert_ne!(added.stable_hash(), plain.stable_hash());
}

#[test]
fn a_difference_style_with_unordered_thresholds_is_rejected() {
    let structure = structure_of(&peptide_pdb(6, 20.0));
    let mut scene = crate::Scene::from_structure(&structure).unwrap_or_else(|e| panic!("{e}"));
    let rows = usize::try_from(structure.atom_count()).unwrap_or_else(|e| panic!("{e}"));
    let property = bound_property(&mut scene, rows);
    for style in [
        super::DifferenceStyle {
            context_threshold: 2.0,
            emphasis_threshold: 1.0,
            ..super::DifferenceStyle::default()
        },
        super::DifferenceStyle {
            domain: [1.0, 1.0],
            ..super::DifferenceStyle::default()
        },
        super::DifferenceStyle {
            context_opacity: 2.0,
            ..super::DifferenceStyle::default()
        },
    ] {
        assert!(super::difference_visual(property.clone(), &style).is_err());
    }
}

#[test]
fn a_difference_over_a_property_of_another_scene_is_rejected() {
    let structure = structure_of(&peptide_pdb(6, 20.0));
    let rows = usize::try_from(structure.atom_count()).unwrap_or_else(|e| panic!("{e}"));
    let mut owner = crate::Scene::from_structure(&structure).unwrap_or_else(|e| panic!("{e}"));
    let property = bound_property(&mut owner, rows);
    let mut other = crate::Scene::from_structure(&structure).unwrap_or_else(|e| panic!("{e}"));
    let style = super::DifferenceStyle::default();
    assert!(
        other
            .add_difference(StructureId(1), "all", property, &style)
            .is_err()
    );
}
