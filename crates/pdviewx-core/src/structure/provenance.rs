//! Constant-time resolution from rendered entity ids to source records.

use crate::{
    ActiveTopologyBond, Annotation, EntityKind, EntityRef, Guide, InteractionEdge, LigandPoseBatch,
    Measurement, Mesh, Primitive, Scene,
};

#[cfg(test)]
#[path = "provenance_tests.rs"]
mod tests;

/// Cold source data for one rendered entity.
#[derive(Clone, Copy, Debug)]
pub struct EntityProvenance<'a> {
    /// Exact entity written by the GPU id attachment.
    pub entity: EntityRef,
    /// Structure-level deposition metadata retained by `pdbiox`.
    pub entry: &'a pdbiox::EntryMetadata,
    /// Kind-specific source record.
    pub detail: ProvenanceDetail<'a>,
}

/// Source record behind a rendered entity.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum ProvenanceDetail<'a> {
    /// Original atom row, including names, altloc, occupancy and B factor.
    Atom(pdbiox::AtomRef<'a>),
    /// Indexed covalent bond and its file/inference/user provenance.
    Bond(pdbiox::BondRecord),
    /// Caller-decoded reactive bond and its current birth/death weight.
    DynamicBond(&'a ActiveTopologyBond),
    /// Caller- or `pdbiox`-supplied interaction fact.
    Interaction(&'a InteractionEdge),
    /// Caller-authored analytic guide segment.
    Guide(&'a Guide),
    /// Persistent human-authored note, marker, region or hypothesis.
    Annotation(&'a Annotation),
    /// Persistent caller-computed measurement and its provenance label.
    Measurement(&'a Measurement),
    /// Caller-authored analytic primitive.
    Primitive(&'a Primitive),
    /// Compact reusable-topology ligand candidate batch.
    LigandPoseBatch(&'a LigandPoseBatch),
    /// Caller-supplied indexed mesh.
    Mesh(&'a Mesh),
}

impl Scene {
    /// Resolves a picked entity to its complete cold provenance in `O(1)`.
    ///
    /// The method returns `None` for stale structures or source rows. It never
    /// scans scene tables and never uploads provenance to the GPU.
    #[must_use]
    pub fn provenance(&self, entity: EntityRef) -> Option<EntityProvenance<'_>> {
        let placed = self.structure(entity.structure)?;
        let detail = match entity.kind {
            EntityKind::Atom => ProvenanceDetail::Atom(
                placed
                    .structure
                    .atom(pdbiox::AtomIndex::new(entity.index))?,
            ),
            EntityKind::Bond => ProvenanceDetail::Bond(
                placed
                    .structure
                    .data()
                    .bonds
                    .get(pdbiox::BondIndex::new(entity.index))?,
            ),
            EntityKind::DynamicBond => {
                ProvenanceDetail::DynamicBond(placed.bond_topology()?.bond(entity.index)?)
            }
            EntityKind::Edge => {
                ProvenanceDetail::Interaction(self.interaction_for_entity(entity)?.1)
            }
            EntityKind::Guide => ProvenanceDetail::Guide(self.guide_for_entity(entity)?.1),
            EntityKind::Label => {
                if let Some((_, annotation)) = self.annotation_for_entity(entity) {
                    ProvenanceDetail::Annotation(annotation)
                } else {
                    ProvenanceDetail::Measurement(self.measurement_for_entity(entity)?.1)
                }
            }
            EntityKind::Primitive => {
                ProvenanceDetail::Primitive(self.primitive_for_entity(entity)?)
            }
            EntityKind::Mesh => ProvenanceDetail::Mesh(self.mesh_for_entity(entity)?),
            EntityKind::LigandPoseBatch => {
                ProvenanceDetail::LigandPoseBatch(self.ligand_pose_batch_for_entity(entity)?)
            }
            EntityKind::Point
            | EntityKind::Instance
            | EntityKind::TemplatePart
            | EntityKind::Relation => return None,
        };
        Some(EntityProvenance {
            entity,
            entry: &placed.structure.data().entry,
            detail,
        })
    }
}
