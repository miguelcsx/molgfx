use super::atomic_targets;
use crate::Selection;
use molgfx_core::MolecularSource;

fn source() -> MolecularSource {
    let pdb = b"HETATM    1  C1  LIG A   1       0.000   0.000   0.000  1.00 20.00           C\nHETATM    2  C2  LIG A   1       1.400   0.000   0.000  1.00 20.00           C\nHETATM    3 MG    MG A   2      10.000   0.000   0.000  1.00 20.00          MG\nEND\n";
    let (structure, _) = molframe::read_bytes(
        pdb.to_vec(),
        Some("components.pdb"),
        &molframe::ReadOptions::new(),
    )
    .unwrap();
    MolecularSource::from_molframe(&structure)
}

fn rows(source: &MolecularSource, selection: &Selection) -> Vec<u32> {
    let selected = source
        .select_compiled(&selection.compiled().unwrap())
        .unwrap();
    let mut rows = Vec::new();
    selected.for_each(3, |row| rows.push(row));
    rows
}

#[test]
fn connected_and_isolated_targets_are_disjoint_and_preserve_source_rows() {
    let source = source();
    let targets = atomic_targets(&source, &"all".into()).unwrap();
    assert_eq!(rows(&source, &targets.connected.unwrap()), [0, 1]);
    assert_eq!(rows(&source, &targets.isolated.unwrap()), [2]);
}

#[test]
fn an_unknown_monatomic_component_can_draw_without_asserting_an_ionic_charge() {
    let source = source();
    let targets = atomic_targets(&source, &"resname MG".into()).unwrap();
    assert!(targets.connected.is_none());
    assert_eq!(rows(&source, &targets.isolated.unwrap()), [2]);
}

#[test]
fn an_empty_atomic_target_produces_no_empty_representation() {
    let targets = atomic_targets(&source(), &"resname NONE".into()).unwrap();
    assert!(targets.connected.is_none() && targets.isolated.is_none());
}
