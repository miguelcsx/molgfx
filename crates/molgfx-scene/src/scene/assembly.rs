//! Biological assemblies as placed copies of one structure.

use super::Scene;
use crate::error::Error;
use crate::id::StructureId;
use crate::representation::Selection;
use molframe::crystal::{AssemblyExt as _, AssemblyView};
use num_traits::ToPrimitive as _;

/// One transform of an assembly and the chains it applies to.
#[derive(Clone, Debug, PartialEq)]
pub struct AssemblyCopy {
    /// The placed structure holding this copy.
    pub structure: StructureId,
    /// `label_asym_id` of every chain the transform applies to, in order.
    pub chains: Vec<Box<str>>,
    /// A query selecting exactly those chains, for the copy's representations.
    pub selection: Selection,
}

/// A query selecting exactly the chains with these `label_asym_id` values.
#[must_use]
pub fn chain_selection(chains: &[impl AsRef<str>]) -> Selection {
    Selection::from(
        chains
            .iter()
            .map(|chain| format!("label_chain {}", chain.as_ref()))
            .collect::<Vec<_>>()
            .join(" or "),
    )
}

/// A rigid transform as a column-major 4×4 affine matrix.
fn column_major(transform: &molframe::geometry::Rigid) -> Result<[f32; 16], Error> {
    let mut wide = [0.0_f64; 16];
    for column in 0..3 {
        for row in 0..3 {
            wide[column * 4 + row] = transform.rotation[row][column];
        }
    }
    wide[12..15].copy_from_slice(&transform.translation);
    wide[15] = 1.0;
    let mut matrix = [0.0_f32; 16];
    for (narrow, value) in matrix.iter_mut().zip(wide) {
        // Single precision is the width of the GPU transform.
        *narrow = value
            .to_f32()
            .filter(|narrow| narrow.is_finite())
            .ok_or_else(|| {
                Error::InvalidSpec("an assembly transform is not representable".to_owned())
            })?;
    }
    Ok(matrix)
}

impl Scene {
    /// Places one copy of `of` per transform of a biological assembly.
    ///
    /// `structure` is the `MolFrame` structure `of` was built from, which holds
    /// the assembly definitions. A transform that applies to several chains
    /// yields one copy listing them; draw it with the returned `selection` so a
    /// copy shows only its own chains. The identity transform is returned like
    /// any other, so the assembly's copies replace, rather than add to, the
    /// deposited asymmetric unit: represent the copies and leave `of` undrawn.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown structure or assembly, an assembly that
    /// expands past the instance limit, or a transform that cannot be placed.
    pub fn add_assembly(
        &mut self,
        of: StructureId,
        structure: &molframe::Structure,
        assembly: &str,
    ) -> Result<Vec<AssemblyCopy>, Error> {
        let set = structure.engine().assembly_set().ok_or_else(|| {
            Error::InvalidSpec("the structure declares no biological assemblies".to_owned())
        })?;
        let view = AssemblyView::new(structure.engine(), set, assembly)
            .map_err(|diagnostic| Error::InvalidSpec(diagnostic.to_string()))?;
        let mut groups: Vec<(molframe::geometry::Rigid, Vec<Box<str>>)> = Vec::new();
        for instance in view.chains() {
            let label: Box<str> = structure
                .engine()
                .chain(instance.source_chain)
                .and_then(molframe::ChainRef::label)
                .ok_or_else(|| Error::InvalidSpec("an assembly chain has no label".to_owned()))?
                .into();
            match groups
                .iter_mut()
                .find(|(known, _)| *known == instance.transform)
            {
                Some((_, chains)) => chains.push(label),
                None => groups.push((instance.transform, vec![label])),
            }
        }
        let mut copies = Vec::with_capacity(groups.len());
        for (transform, chains) in groups {
            let placed = self.place(of, column_major(&transform)?)?;
            let selection = chain_selection(&chains);
            copies.push(AssemblyCopy {
                structure: placed,
                chains,
                selection,
            });
        }
        Ok(copies)
    }
}
