//! Molecular gallery sources and declarative representation recipes.
use super::catalog::{Fixture, Result, Style};
use molgfx::{Color, ColorSpec, Scene, rep, sel};
use serde_json::{Value, json};
use std::{collections::BTreeMap, io, path::Path};

/// Loads the canonical molecular source and reports scientific metadata.
///
/// # Errors
/// Returns source I/O and `MolFrame` parsing errors.
pub fn structure(fixture: &Fixture, cache: &Path) -> Result<(molframe::Structure, Value)> {
    let (structure, diagnostics) = crate::reader::read_structure(&cache.join(&fixture.file))?;
    let mut orders = BTreeMap::<String, usize>::new();
    let mut provenance = BTreeMap::<String, usize>::new();
    let mut aromatic = 0;
    for bond in structure.bonds().iter() {
        *orders.entry(format!("{:?}", bond.order)).or_default() += 1;
        *provenance
            .entry(format!("{:?}", bond.provenance))
            .or_default() += 1;
        aromatic += usize::from(bond.order == molframe::BondOrder::Aromatic);
    }
    let mut secondary = BTreeMap::<String, usize>::new();
    for state in structure.secondary_structure() {
        let code = molgfx::schema::SecondaryStructure::from(*state).name();
        *secondary.entry(code.into()).or_default() += 1;
    }
    let mut secondary_residues = Vec::with_capacity(structure.residue_count());
    for chain in structure.chains() {
        for residue in chain.residues() {
            if let Some(sequence) = residue.label_seq_id() {
                let state = structure.secondary_structure()[residue.index().as_usize()];
                secondary_residues.push((
                    chain.label(),
                    sequence,
                    molgfx::schema::SecondaryStructure::from(state).name(),
                ));
            }
        }
    }
    let metadata = json!({"atoms":structure.atom_count(),"residues":structure.residue_count(),
        "bonds":structure.bonds().iter().count(),"bond_orders":orders,"aromatic_bonds":aromatic,
        "bond_provenance":provenance,"secondary_structure":secondary,
        "secondary_residues":secondary_residues,
        "secondary_structure_policy":"MolFrame file > DSSP > CA-only; exact eleven-state labels",
        "secondary_structure_encoding":"canonical exact-state labels; no helix or beta collapse","diagnostics":diagnostics});
    Ok((structure, metadata))
}

pub(super) fn add_form(scene: &mut Scene, form: &str, style: &Style) -> Result<()> {
    let [r, g, b] = style.color_rgb;
    let color = ColorSpec::Uniform {
        color: Color::rgb(r, g, b),
    };
    match form {
        "spacefill" => scene.add(
            rep::spacefill(sel::all())
                .radius(style.atom_radius_scale)
                .color(color)
                .opacity(style.opacity),
        )?,
        "ball_and_stick" => scene.add(
            rep::ball_and_stick(sel::all())
                .radius(style.atom_radius_scale)
                .bond_radius(style.bond_radius_angstrom)
                .color(color)
                .opacity(style.opacity),
        )?,
        "cartoon" => scene.add(
            rep::cartoon(sel::all())
                .width(style.cartoon_width_angstrom)
                .color(color)
                .opacity(style.opacity),
        )?,
        _ => return Err(io::Error::other("unsupported molecular parity form").into()),
    };
    Ok(())
}
