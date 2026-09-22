//! Compilation and execution of the shared typed selection IR.
//!
//! Entity predicates are linear in matching hierarchy rows. Spatial nodes use
//! the persistent atom BVH and cost `O(f log n + candidates)` for `f` reference
//! atoms. Scratch vectors are retained by the scene across queries.

use crate::handle::StructureHandle;
use crate::scene::Scene;
use crate::select::{AtomPredicate, EntityClass, ScalarProperty, SelectExpr};
use crate::{AtomSelection, CoreError, PlacedStructure, Select, SelectionHandle};
use molgfx_math::Aabb;
use roaring::RoaringBitmap;

#[cfg(test)]
#[path = "query_tests.rs"]
mod tests;

impl Scene {
    /// Compiles and executes one typed query into a stored adaptive selection.
    ///
    /// # Errors
    ///
    /// The typed expression is already validated; this remains fallible so the
    /// string and builder surfaces retain one stable API contract.
    pub fn select(&mut self, query: Select) -> Result<SelectionHandle, CoreError> {
        let Select(expression) = query;
        let mut traversal = std::mem::take(&mut self.spatial_traversal);
        let mut candidates = std::mem::take(&mut self.spatial_candidates);
        let mut scoped = Vec::with_capacity(self.structures.len());
        for (raw, placed) in self.structures.iter() {
            let rows = evaluate(&expression, placed, &mut traversal, &mut candidates)?;
            scoped.push((StructureHandle(raw), adaptive(rows, placed.atoms.len())));
        }
        self.spatial_traversal = traversal;
        self.spatial_candidates = candidates;
        Ok(self.add_scoped_selection(scoped))
    }

    /// Parses and executes the string front-end over the same typed IR.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidSelection`] for malformed syntax, unknown
    /// keywords or invalid spatial distances.
    pub fn select_str(&mut self, source: &str) -> Result<SelectionHandle, CoreError> {
        let query = molframe::Query::compile(source).map_err(|_| CoreError::InvalidSelection {
            reason: "MolFrame query evaluation failed",
        })?;
        let fingerprint = query.fingerprint().get();
        let mut scoped = Vec::with_capacity(self.structures.len());
        for (raw, placed) in self.structures.iter() {
            scoped.push((StructureHandle(raw), placed.source.select_compiled(&query)?));
        }
        Ok(self.add_scoped_selection_with_fingerprint(scoped, Some(fingerprint)))
    }
}

fn evaluate(
    expression: &SelectExpr,
    placed: &PlacedStructure,
    traversal: &mut Vec<u32>,
    candidates: &mut Vec<u32>,
) -> Result<RoaringBitmap, CoreError> {
    Ok(match expression {
        SelectExpr::Class(class) => class_rows(*class, placed),
        SelectExpr::Predicate(predicate) => predicate_rows(predicate, placed),
        SelectExpr::InSphere { center, radius } => {
            world_sphere_rows(*center, *radius, placed, traversal, candidates)?
        }
        SelectExpr::InBox { min, max } => box_rows(*min, *max, placed, traversal, candidates)?,
        SelectExpr::And(left, right) => {
            evaluate(left, placed, traversal, candidates)?
                & evaluate(right, placed, traversal, candidates)?
        }
        SelectExpr::Or(left, right) => {
            evaluate(left, placed, traversal, candidates)?
                | evaluate(right, placed, traversal, candidates)?
        }
        SelectExpr::Not(inner) => {
            let mut all = (0..placed.atoms.len()).collect::<RoaringBitmap>();
            all -= evaluate(inner, placed, traversal, candidates)?;
            all
        }
        SelectExpr::Within {
            distance,
            reference,
            residues,
        } => spatial_rows(
            *distance,
            evaluate(reference, placed, traversal, candidates)?,
            *residues,
            placed,
            traversal,
            candidates,
        )?,
    })
}

fn class_rows(class: EntityClass, placed: &PlacedStructure) -> RoaringBitmap {
    if class == EntityClass::None {
        return RoaringBitmap::new();
    }
    if class == EntityClass::All {
        return (0..placed.atoms.len()).collect();
    }
    let Some(structure) = placed.source.molframe() else {
        return RoaringBitmap::new();
    };
    let data = structure.engine().data();
    let mut rows = RoaringBitmap::new();
    for chain in data.chains() {
        let entity_kind = chain
            .entity()
            .and_then(|entity| data.topology.entities.kind(entity));
        let polymer_kind = chain.polymer_kind();
        let matches = match class {
            EntityClass::Polymer => {
                polymer_kind.is_polymer() || entity_kind == Some(molframe::EntityKind::Polymer)
            }
            EntityClass::Protein => matches!(polymer_kind, molframe::PolymerKind::Protein),
            EntityClass::Nucleic => polymer_kind.is_nucleic(),
            EntityClass::NonPolymer => entity_kind == Some(molframe::EntityKind::NonPolymer),
            EntityClass::Water => entity_kind == Some(molframe::EntityKind::Water),
            EntityClass::Branched => entity_kind == Some(molframe::EntityKind::Branched),
            EntityClass::All | EntityClass::None => false,
        };
        if matches {
            for residue in chain.residues() {
                for atom in residue.atoms() {
                    rows.insert(atom.index().get());
                }
            }
        }
    }
    rows
}

fn predicate_rows(predicate: &AtomPredicate, placed: &PlacedStructure) -> RoaringBitmap {
    let mut rows = RoaringBitmap::new();
    let Some(structure) = placed.source.molframe() else {
        return rows;
    };
    let data = structure.engine().data();
    for chain in data.chains() {
        let first_residue = chain.residue_at(0).map(molframe::ResidueRef::index);
        let last_residue = chain.residues().last().map(molframe::ResidueRef::index);
        for residue in chain.residues() {
            for atom in residue.atoms() {
                if predicate_matches(
                    predicate,
                    chain,
                    residue,
                    atom,
                    placed,
                    first_residue,
                    last_residue,
                ) {
                    rows.insert(atom.index().get());
                }
            }
        }
    }
    rows
}

fn predicate_matches(
    predicate: &AtomPredicate,
    chain: molframe::ChainRef<'_>,
    residue: molframe::ResidueRef<'_>,
    atom: molframe::AtomRef<'_>,
    placed: &PlacedStructure,
    first_residue: Option<molframe::ResidueIndex>,
    last_residue: Option<molframe::ResidueIndex>,
) -> bool {
    match predicate {
        AtomPredicate::Chain(wanted) => chain
            .label()
            .into_iter()
            .chain(chain.auth_label())
            .any(|value| value.eq_ignore_ascii_case(wanted)),
        AtomPredicate::ResidueName(wanted) => residue
            .name()
            .into_iter()
            .chain(residue.auth_name())
            .any(|value| value.eq_ignore_ascii_case(wanted)),
        AtomPredicate::AtomName(wanted) => atom
            .name()
            .into_iter()
            .chain(atom.auth_name())
            .any(|value| value.eq_ignore_ascii_case(wanted)),
        AtomPredicate::ResidueNumber(wanted) => {
            residue.auth_seq_id() == Some(*wanted) || residue.label_seq_id() == Some(*wanted)
        }
        AtomPredicate::Element(wanted) => atom
            .element()
            .is_some_and(|value| value.atomic_number() == *wanted),
        AtomPredicate::Secondary(wanted) => placed
            .secondary_structure
            .values()
            .get(residue.index().get() as usize)
            .is_some_and(|value| value == wanted),
        AtomPredicate::Scalar {
            property,
            comparison,
            threshold,
        } => {
            let value = match property {
                ScalarProperty::BFactor => atom.b_factor(),
                ScalarProperty::Occupancy => atom.occupancy(),
            };
            value.is_some_and(|value| comparison.matches(value, *threshold))
        }
        AtomPredicate::Hydrogen => atom.element().is_some_and(molframe::Element::is_hydrogen),
        AtomPredicate::Heavy => atom
            .element()
            .is_some_and(|value| !value.is_unknown() && !value.is_hydrogen()),
        AtomPredicate::Backbone => atom.name().is_some_and(is_backbone_name),
        AtomPredicate::Terminus => {
            Some(residue.index()) == first_residue || Some(residue.index()) == last_residue
        }
    }
}

fn is_backbone_name(name: &str) -> bool {
    let name = name.trim();
    [
        "N", "CA", "C", "O", "OXT", "P", "OP1", "OP2", "OP3", "O3'", "O5'", "C1'", "C2'", "C3'",
        "C4'", "C5'",
    ]
    .into_iter()
    .any(|candidate| name.eq_ignore_ascii_case(candidate))
}

fn world_sphere_rows(
    center: molgfx_math::Vec3,
    radius: f32,
    placed: &PlacedStructure,
    traversal: &mut Vec<u32>,
    candidates: &mut Vec<u32>,
) -> Result<RoaringBitmap, CoreError> {
    let inverse = placed.model_to_world.inverse();
    let local_center = inverse.transform_point3(center);
    let local_radius = radius
        * [
            molgfx_math::Vec3::X,
            molgfx_math::Vec3::Y,
            molgfx_math::Vec3::Z,
        ]
        .into_iter()
        .map(|axis| inverse.transform_vector3(axis).length_squared())
        .sum::<f32>()
        .sqrt();
    placed
        .spatial_bvh()?
        .sphere_candidates(local_center, local_radius, traversal, candidates);
    let radius_sq = radius * radius;
    Ok(candidates
        .iter()
        .copied()
        .filter(|&row| {
            placed
                .atoms
                .coords()
                .slice()
                .get(row as usize)
                .is_some_and(|position| {
                    let world = placed
                        .model_to_world
                        .transform_point3(molgfx_math::Vec3::from_array(*position));
                    world.distance_squared(center) <= radius_sq
                })
        })
        .collect())
}

fn box_rows(
    min: molgfx_math::Vec3,
    max: molgfx_math::Vec3,
    placed: &PlacedStructure,
    traversal: &mut Vec<u32>,
    candidates: &mut Vec<u32>,
) -> Result<RoaringBitmap, CoreError> {
    let world_box = Aabb::new(min, max);
    let local_box = world_box.transform(&placed.model_to_world.inverse());
    placed
        .spatial_bvh()?
        .aabb_candidates(local_box, traversal, candidates);
    Ok(candidates
        .iter()
        .copied()
        .filter(|&row| {
            let Some(position) = placed.atoms.coords().slice().get(row as usize) else {
                return false;
            };
            let world = placed
                .model_to_world
                .transform_point3(molgfx_math::Vec3::from_array(*position));
            world.x >= min.x
                && world.y >= min.y
                && world.z >= min.z
                && world.x <= max.x
                && world.y <= max.y
                && world.z <= max.z
        })
        .collect())
}

fn spatial_rows(
    distance: f32,
    reference: RoaringBitmap,
    complete_residues: bool,
    placed: &PlacedStructure,
    traversal: &mut Vec<u32>,
    candidates: &mut Vec<u32>,
) -> Result<RoaringBitmap, CoreError> {
    let distance_sq = distance * distance;
    let coordinates = placed.atoms.coords().slice();
    let world_from_model = placed.model_to_world;
    let inverse_scale_bound = [
        molgfx_math::Vec3::X,
        molgfx_math::Vec3::Y,
        molgfx_math::Vec3::Z,
    ]
    .into_iter()
    .map(|axis| {
        world_from_model
            .inverse()
            .transform_vector3(axis)
            .length_squared()
    })
    .sum::<f32>()
    .sqrt();
    let local_radius = distance * inverse_scale_bound;
    let hierarchy = placed.spatial_bvh()?;
    let mut rows = RoaringBitmap::new();
    for source in reference {
        let Some(source_position) = coordinates.get(source as usize).copied() else {
            continue;
        };
        let source_position = molgfx_math::Vec3::from_array(source_position);
        let source_world = world_from_model.transform_point3(source_position);
        hierarchy.sphere_candidates(source_position, local_radius, traversal, candidates);
        for &candidate in candidates.iter() {
            let Some(candidate_position) = coordinates.get(candidate as usize).copied() else {
                continue;
            };
            let candidate_world = world_from_model
                .transform_point3(molgfx_math::Vec3::from_array(candidate_position));
            if source_world.distance_squared(candidate_world) > distance_sq {
                continue;
            }
            if complete_residues {
                if let Some(residue) = placed.hierarchy.residue_of_atom(candidate) {
                    rows.insert_range(placed.hierarchy.residue_atoms(residue));
                }
            } else {
                rows.insert(candidate);
            }
        }
    }
    Ok(rows)
}

fn adaptive(rows: RoaringBitmap, table_len: u32) -> AtomSelection {
    if rows.is_empty() {
        AtomSelection::Empty
    } else if rows.len() == u64::from(table_len) {
        AtomSelection::All
    } else {
        AtomSelection::Roaring(rows)
    }
}
