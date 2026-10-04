//! Reading a live scene back: resolving renderer picks to exact atoms, and
//! the residue metadata a sequence view lists.

use super::Scene;
use super::atom_pick::atom_pick_details;
use crate::error::Error;
use crate::id::StructureId;

/// Metadata for one residue observed in a bound molecular structure.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ResidueMetadata {
    /// Chain identifier.
    pub chain: Option<String>,
    /// Author-provided chain identifier.
    pub auth_chain: Option<String>,
    /// Entity identifier.
    pub entity: Option<u32>,
    /// One-letter residue code.
    pub one_letter: Option<String>,
    /// Component identifier.
    pub component: Option<String>,
    /// Author-provided component identifier.
    pub auth_component: Option<String>,
    /// Author-provided residue number.
    pub auth_number: Option<i32>,
    /// Label residue number.
    pub label_number: Option<i32>,
    /// Insertion code.
    pub insertion_code: Option<String>,
    /// Whether the residue was observed.
    pub observed: bool,
    /// Zero-based residue index.
    pub residue_index: u32,
    /// Inclusive atom range.
    pub atom_range: Option<[u32; 2]>,
}

/// A molecular atom resolved against the exact structure and topology owned by a scene.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResolvedAtomPick {
    /// Structure identifier.
    pub structure: StructureId,
    /// Dataset identifier.
    pub dataset: u64,
    /// Chunk identifier.
    pub chunk: u64,
    /// Topology revision identifier.
    pub topology_revision: u64,
    /// First model index represented by the scene's coordinate column.
    pub model: Option<u32>,
    /// Entity index, when the source has entity assignments.
    pub entity: Option<u32>,
    /// Atom index within the topology.
    pub atom_index: u32,
    /// Normalised atom name, when the source carries one.
    pub atom_name: Option<String>,
    /// Depositor atom name, when it differs or was recorded.
    pub auth_atom_name: Option<String>,
    /// Alternate-location identifier.
    pub altloc: Option<String>,
    /// Atomic number, or zero for an unknown element.
    pub element: Option<u8>,
    /// Residue index in the source topology.
    pub residue_index: Option<u32>,
    /// Normalised residue/component name.
    pub residue_name: Option<String>,
    /// Depositor residue/component name.
    pub auth_residue_name: Option<String>,
    /// Normalised chain label.
    pub chain: Option<String>,
    /// Depositor chain label.
    pub auth_chain: Option<String>,
    /// Depositor residue number.
    pub residue_number: Option<i32>,
    /// Insertion code.
    pub insertion_code: Option<String>,
    /// Recorded occupancy.
    pub occupancy: Option<f32>,
    /// Recorded temperature factor.
    pub b_factor: Option<f32>,
    /// Recorded formal charge.
    pub formal_charge: Option<i8>,
    /// First-model coordinates in source/model space.
    pub position: Option<[f32; 3]>,
    /// File or analysis secondary-structure assignment.
    pub secondary_structure: Option<String>,
    /// Stable human-readable atom label for UI clients.
    pub label: String,
}

/// A molecular bond resolved against the exact topology owned by a scene.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResolvedBondPick {
    /// Structure identifier.
    pub structure: StructureId,
    /// Provider dataset identifier.
    pub dataset: u64,
    /// Provider chunk identifier.
    pub chunk: u64,
    /// Scene topology revision at which the bond was observed.
    pub topology_revision: u64,
    /// Source or dynamic-topology row.
    pub bond_index: u32,
    /// First endpoint atom row.
    pub atom_a: u32,
    /// Second endpoint atom row.
    pub atom_b: u32,
    /// Static source order; dynamic topology carries no order field.
    pub order: Option<String>,
    /// Whether the source marks this connection aromatic.
    pub aromatic: bool,
    /// Whether the source marks this connection as metal coordination.
    pub metal: bool,
    /// Dynamic topology weight, or `None` for an immutable source bond.
    pub weight: Option<f32>,
}

/// Result of resolving a renderer pick against a live semantic scene.
///
/// The serialized form is internally tagged by a `pick` discriminator with the
/// variant's own fields beside it, so a reader can dispatch on one string:
///
/// - `{"pick": "atom", ...}` and `{"pick": "bond", ...}` carry the molecular
///   record.
/// - `{"pick": "label", "annotation": ..., ...}`, `{"pick": "measurement",
///   ...}` and `{"pick": "volume_segment", ...}` carry the semantic overlay
///   item when the renderer's pick names one the scene still owns.
/// - `{"pick": "non_atom", "kind": ..., "dataset": ..., "chunk": ..., "row":
///   ..., "volume_label": ...}` carries the renderer's own [`crate::PickResult`]
///   for guided, instanced and relation entities that have no richer record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "pick", rename_all = "snake_case")]
pub enum ResolvedPick {
    /// An atom resolved to stable scene coordinates.
    Atom(Box<ResolvedAtomPick>),
    /// A bond resolved to source endpoints and current topology metadata.
    Bond(Box<ResolvedBondPick>),
    /// An annotation label resolved to its owning scene item.
    Label(Box<ResolvedLabelPick>),
    /// A measurement resolved to its kind, anchors and current value.
    Measurement(Box<ResolvedMeasurementPick>),
    /// A categorical volume segment resolved to its volume and caller label.
    VolumeSegment(Box<ResolvedVolumeSegmentPick>),
    /// A non-atom renderer pick without a molecular topology record.
    NonAtom(crate::PickResult),
}

/// An annotation label resolved against the scene that owns it.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResolvedLabelPick {
    /// Structure the label is anchored to.
    pub structure: StructureId,
    /// Annotation identity within the scene.
    pub annotation: crate::AnnotationId,
    /// Whether the label is a plain note, a region, or a marker.
    pub kind: String,
    /// The label text, exactly as authored.
    pub text: String,
    /// Stable human-readable anchor for UI clients.
    pub label: String,
}

/// A measurement resolved against the scene that owns it.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResolvedMeasurementPick {
    /// Structure the measurement is anchored to.
    pub structure: StructureId,
    /// Measurement identity within the scene.
    pub measurement: crate::MeasurementId,
    /// `distance`, `angle` or `dihedral`.
    pub kind: String,
    /// Number of anchors the measurement reads.
    pub arity: usize,
    /// The current value in ångström or degrees, when the scene resolved it.
    pub value: Option<f64>,
    /// Stable human-readable anchor for UI clients.
    pub label: String,
}

/// A categorical volume segment resolved against the scene that owns it.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResolvedVolumeSegmentPick {
    /// Categorical grid lifetime within the scene.
    pub segmentation: crate::SegmentationId,
    /// Caller-supplied categorical label.
    pub volume_label: u32,
    /// Stable human-readable anchor for UI clients.
    pub label: String,
}

impl Scene {
    /// Resolves a physical pick against this scene's exact dataset and topology.
    ///
    /// The dataset-to-structure mapping is derived explicitly from each placed
    /// structure's authoritative metadata: a placed source proves its semantic
    /// owner by shared storage with exactly one bound structure. A dataset that
    /// maps to several structures is rejected as ambiguous, and one that maps
    /// to none is rejected as stale, instead of trusting positional order.
    ///
    /// # Errors
    ///
    /// Returns [`Error::AmbiguousPick`] when a dataset or source identity maps
    /// to multiple structures, [`Error::StalePick`] when no bound structure
    /// proves ownership, and [`Error::InvalidSpec`] for malformed picks.
    pub fn resolve_pick(&self, pick: &crate::PickResult) -> Result<ResolvedPick, Error> {
        if pick.kind == crate::PickKind::VolumeSegment {
            return self
                .volume_segment_pick(pick)
                .map(|segment| ResolvedPick::VolumeSegment(Box::new(segment)));
        }
        let Some(dataset) = pick.dataset else {
            return Ok(self.overlay_pick(pick));
        };

        let Some(chunk) = pick.chunk else {
            return Err(Error::InvalidSpec(
                "pick is missing its chunk identity".to_owned(),
            ));
        };

        let Some(row) = pick.row else {
            return Err(Error::InvalidSpec(
                "pick is missing its logical row".to_owned(),
            ));
        };

        let mut placed = None;

        for candidate in self.resolved.structures().map(|(_, placed)| placed) {
            if candidate.dataset_id().get() != dataset {
                continue;
            }

            if placed.is_some() {
                return Err(Error::AmbiguousPick { dataset });
            }

            placed = Some(candidate);
        }

        let placed = placed.ok_or(Error::StalePick { dataset })?;
        let structure = self.structure_for_source(&placed.source, dataset)?;

        let row_index = u32::try_from(row).map_err(|_| {
            Error::InvalidSpec(format!(
                "pick row {row} is outside the supported index space"
            ))
        })?;

        if pick.kind == crate::PickKind::Bond {
            return resolve_static_bond_pick(
                &placed.source,
                structure,
                dataset,
                chunk,
                self.resolved.structure_revision(),
                row,
                row_index,
            );
        }

        if pick.kind == crate::PickKind::DynamicBond {
            return resolve_dynamic_bond_pick(
                placed,
                structure,
                dataset,
                chunk,
                self.resolved.structure_revision(),
                row,
                row_index,
            );
        }

        let is_atom = pick.kind == crate::PickKind::Atom;

        if is_atom && row_index >= placed.atoms.len() {
            return Err(Error::InvalidSpec(format!(
                "pick row {row} is outside structure topology"
            )));
        }

        if !is_atom {
            return Ok(ResolvedPick::NonAtom(pick.clone()));
        }

        let atom_index = row_index;
        let details = atom_pick_details(placed.source.molframe(), atom_index);

        Ok(ResolvedPick::Atom(Box::new(ResolvedAtomPick {
            structure,
            dataset,
            chunk,
            topology_revision: self.resolved.structure_revision(),
            model: details.model,
            entity: details.entity,
            atom_index,
            atom_name: details.atom_name,
            auth_atom_name: details.auth_atom_name,
            altloc: details.altloc,
            element: details.element,
            residue_index: details.residue_index,
            residue_name: details.residue_name,
            auth_residue_name: details.auth_residue_name,
            chain: details.chain,
            auth_chain: details.auth_chain,
            residue_number: details.residue_number,
            insertion_code: details.insertion_code,
            occupancy: details.occupancy,
            b_factor: details.b_factor,
            formal_charge: details.formal_charge,
            position: details.position,
            secondary_structure: details.secondary_structure,
            label: details.label,
        })))
    }

    /// Resolves the semantic structure that owns one placed physical source.
    ///
    /// Ownership is proven by shared provider storage with a bound structure,
    /// never by iteration position. Two bindings sharing one source cannot be
    /// told apart, so the mapping is ambiguous rather than positional.
    fn structure_for_source(
        &self,
        source: &molgfx_core::MolecularSource,
        dataset: u64,
    ) -> Result<StructureId, Error> {
        let mut owner = None;

        for (identity, bound) in &self.structures {
            if !bound.shares_storage_with(source) {
                continue;
            }

            if owner.is_some() {
                return Err(Error::AmbiguousPick { dataset });
            }

            owner = Some(*identity);
        }

        owner.ok_or(Error::StalePick { dataset })
    }

    /// Returns typed metadata for residues observed in one structure.
    ///
    /// # Errors
    ///
    /// Returns an error when the structure is unknown, is not backed by
    /// `MolFrame` data, or its residue index space cannot be represented.
    pub fn residue_metadata(&self, structure: StructureId) -> Result<Vec<ResidueMetadata>, Error> {
        let source = self
            .structures
            .get(&structure)
            .ok_or_else(|| Error::InvalidSpec(format!("unknown structure {}", structure.get())))?;

        let Some(input) = source.molframe() else {
            return Err(Error::InvalidSpec(
                "residue metadata requires a MolFrame-backed structure".to_owned(),
            ));
        };

        let topology = source.topology();
        let mut metadata = Vec::with_capacity(input.residue_count());

        for (chain_index, chain) in input.chains().iter().enumerate() {
            let chain_start = topology
                .chain_residue_start
                .get(chain_index)
                .map_or(0, |start| *start);

            let chain_label = chain.label();
            let auth_chain_label = chain.auth_label();
            let entity = chain.entity().map(molframe::EntityIndex::get);

            for (local_residue, residue) in chain.residues().enumerate() {
                let local_residue = u32::try_from(local_residue).map_err(|_| {
                    Error::InvalidSpec("residue index exceeds the supported range".to_owned())
                })?;

                let residue_index = chain_start.checked_add(local_residue).ok_or_else(|| {
                    Error::InvalidSpec("residue index exceeds the supported range".to_owned())
                })?;

                let atom_range = usize::try_from(residue_index)
                    .ok()
                    .and_then(|start| {
                        let end = start.checked_add(2)?;
                        topology.residue_atom_start.get(start..end)
                    })
                    .and_then(|range| match range {
                        [first, second] => Some([*first, *second]),
                        _ => None,
                    });

                metadata.push(ResidueMetadata {
                    chain: chain_label.map(str::to_owned),
                    auth_chain: auth_chain_label.map(str::to_owned),
                    entity,
                    one_letter: None,
                    component: residue.name().map(str::to_owned),
                    auth_component: residue.auth_name().map(str::to_owned),
                    auth_number: residue.auth_seq_id(),
                    label_number: residue.label_seq_id(),
                    insertion_code: residue.ins_code().map(str::to_owned),
                    observed: true,
                    residue_index,
                    atom_range,
                });
            }
        }

        Ok(metadata)
    }
}

fn resolve_static_bond_pick(
    source: &molgfx_core::MolecularSource,
    structure: StructureId,
    dataset: u64,
    chunk: u64,
    topology_revision: u64,
    row: u64,
    row_index: u32,
) -> Result<ResolvedPick, Error> {
    let bond = source
        .topology()
        .bonds
        .get(row_index as usize)
        .ok_or_else(|| {
            Error::InvalidSpec(format!("pick row {row} is outside the bond topology"))
        })?;
    Ok(ResolvedPick::Bond(Box::new(ResolvedBondPick {
        structure,
        dataset,
        chunk,
        topology_revision,
        bond_index: row_index,
        atom_a: bond.atoms[0],
        atom_b: bond.atoms[1],
        order: Some(format!("{:?}", bond.order).to_ascii_lowercase()),
        aromatic: bond.aromatic,
        metal: bond.metal,
        weight: None,
    })))
}

fn resolve_dynamic_bond_pick(
    placed: &molgfx_core::PlacedStructure,
    structure: StructureId,
    dataset: u64,
    chunk: u64,
    topology_revision: u64,
    row: u64,
    row_index: u32,
) -> Result<ResolvedPick, Error> {
    let active = placed
        .bond_topology()
        .and_then(|topology| topology.bond(row_index))
        .ok_or_else(|| {
            Error::InvalidSpec(format!(
                "pick row {row} is outside the dynamic bond topology"
            ))
        })?;
    let bond = active.bond();
    let [atom_a, atom_b] = bond.atoms();
    Ok(ResolvedPick::Bond(Box::new(ResolvedBondPick {
        structure,
        dataset,
        chunk,
        topology_revision,
        bond_index: row_index,
        atom_a,
        atom_b,
        order: None,
        aromatic: bond.is_aromatic(),
        metal: false,
        weight: Some(active.weight()),
    })))
}
