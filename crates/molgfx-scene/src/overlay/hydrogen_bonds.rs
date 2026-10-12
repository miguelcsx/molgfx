//! Bulk hydrogen-bond presentation over shared upstream chemistry results.
//!
//! Lowering costs O(records), performed only when a descriptor or binding changes.
//! GPU endpoints and geometric gates gather live atom coordinates. Chemical
//! eligibility remains upstream; motion candidates cover their prepared segment.

use super::{DataSource, InteractionSpec};
use crate::{Color, Error, Selection, StructureId};
pub use molgfx_core::OrientedGeometry as HydrogenBondGeometry;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A chemically classified hydrogen-bond table rendered as donor-acceptor strokes.
///
/// The table must come from molecular analysis with explicit hydrogen atoms and
/// donor/acceptor roles. A distance-only contact table is not a substitute.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct HydrogenBonds {
    /// Content identity of the upstream analysis table.
    pub source: DataSource,
    /// Structure whose atom rows the table addresses.
    pub structure: StructureId,
    /// Both heavy-atom endpoints must belong to this selection.
    pub selection: Selection,
    /// Stroke width in physical pixels.
    pub width_pixels: f32,
    /// Stroke color.
    pub color: Color,
    /// Coverage opacity.
    pub opacity: f32,
    /// Optional instantaneous distance and angle policy. Required for candidates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<HydrogenBondGeometry>,
}

impl HydrogenBonds {
    /// Creates a dotted presentation of an upstream table without detecting bonds.
    #[must_use]
    pub fn new(source: DataSource, structure: StructureId) -> Self {
        Self {
            source,
            structure,
            selection: "all".into(),
            width_pixels: 1.5,
            color: Color::rgb(128, 204, 255),
            opacity: 1.0,
            geometry: None,
        }
    }

    /// Selects heavy-atom endpoints without requiring hydrogens in the selection.
    #[must_use]
    pub fn selection(mut self, selection: impl Into<Selection>) -> Self {
        self.selection = selection.into();
        self
    }

    /// Applies distance and explicit-hydrogen angle tests to every current pose.
    #[must_use]
    pub const fn geometry(mut self, geometry: HydrogenBondGeometry) -> Self {
        self.geometry = Some(geometry);
        self
    }

    /// Turns this presentation into an insertable interaction descriptor.
    #[must_use]
    pub fn interaction(self) -> InteractionSpec {
        InteractionSpec::HydrogenBonds(self)
    }

    pub(crate) fn validate(&self, scene: &crate::SceneSpec) -> Result<(), Error> {
        if !scene.structures.contains_key(&self.structure)
            || self.source.content_hash.trim().is_empty()
            || !self.width_pixels.is_finite()
            || self.width_pixels <= 0.0
            || !self.opacity.is_finite()
            || !(0.0..=1.0).contains(&self.opacity)
        {
            return Err(invalid(
                "hydrogen bonds require a known structure, a source identity, a positive width and opacity in zero to one",
            ));
        }
        let _ = self.selection.fingerprint()?;
        if let Some(geometry) = self.geometry {
            geometry.validate()?;
        }
        Ok(())
    }
}

/// Shared `MolFrame` results bound outside the portable scene document.
#[derive(Clone, Debug)]
pub struct HydrogenBondBinding {
    source: DataSource,
    rows: HydrogenBondRows,
    candidate_distance: Option<f32>,
    pose: Option<molgfx_core::MolecularPose>,
}

#[derive(Clone, Debug)]
enum HydrogenBondRows {
    Endpoints(Arc<[[u32; 3]]>),
    #[cfg(not(target_arch = "wasm32"))]
    MolFrame(Arc<molframe::analysis::HydrogenBondTable>),
    #[cfg(not(target_arch = "wasm32"))]
    Candidates(Arc<molframe::analysis::HydrogenBondCandidateTable>),
}

impl HydrogenBondBinding {
    /// Portable content identity of this immutable chemical table.
    #[must_use]
    pub const fn source(&self) -> &DataSource {
        &self.source
    }

    pub(crate) fn same_input(&self, other: &Self) -> bool {
        if self.candidate_distance.map(f32::to_bits) != other.candidate_distance.map(f32::to_bits) {
            return false;
        }
        let same_pose = match (&self.pose, &other.pose) {
            (Some(a), Some(b)) if self.candidate_distance.is_some() => a.shares_interval(b),
            (a, b) => a == b,
        };
        if !same_pose || !self.triples().eq(other.triples()) {
            return false;
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let (HydrogenBondRows::MolFrame(a), HydrogenBondRows::MolFrame(b)) =
            (&self.rows, &other.rows)
        {
            return a == b;
        }
        true
    }
    /// Borrows classified original atom addresses in deterministic table order.
    pub fn triples(&self) -> impl Iterator<Item = [u32; 3]> + '_ {
        (0..self.len()).filter_map(|index| self.endpoints(index))
    }

    /// Distance envelope used to prepare motion candidates, absent for classified snapshots.
    #[must_use]
    pub const fn candidate_distance(&self) -> Option<f32> {
        self.candidate_distance
    }
    /// Retains the exact scientific input; conservative candidates cover its whole interval.
    #[must_use]
    pub fn at_pose(mut self, pose: molgfx_core::MolecularPose) -> Self {
        self.pose = Some(pose);
        self
    }

    /// Borrows retained input provenance without copying molecular coordinates.
    #[must_use]
    pub const fn pose(&self) -> Option<&molgfx_core::MolecularPose> {
        self.pose.as_ref()
    }
    /// Binds caller-classified donor, explicit hydrogen and acceptor atom rows.
    /// These are rendering addresses, not a distance-based chemistry detector.
    #[must_use]
    pub const fn new(source: DataSource, endpoints: Arc<[[u32; 3]]>) -> Self {
        Self {
            source,
            rows: HydrogenBondRows::Endpoints(endpoints),
            candidate_distance: None,
            pose: None,
        }
    }

    /// Shares a native upstream analysis table without copying any columns.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub const fn from_molframe(
        source: DataSource,
        table: Arc<molframe::analysis::HydrogenBondTable>,
    ) -> Self {
        Self {
            source,
            rows: HydrogenBondRows::MolFrame(table),
            candidate_distance: None,
            pose: None,
        }
    }

    /// Binds conservative caller-classified candidates for a distance envelope.
    #[must_use]
    pub const fn candidates(
        source: DataSource,
        endpoints: Arc<[[u32; 3]]>,
        maximum_distance: f32,
    ) -> Self {
        Self {
            source,
            rows: HydrogenBondRows::Endpoints(endpoints),
            candidate_distance: Some(maximum_distance),
            pose: None,
        }
    }

    /// Borrows upstream oriented motion candidates without copying their columns.
    /// Retain the preparation interval with [`Self::at_pose`] before leaving a worker.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub const fn from_molframe_candidates(
        source: DataSource,
        table: Arc<molframe::analysis::HydrogenBondCandidateTable>,
        maximum_distance: f32,
    ) -> Self {
        Self {
            source,
            rows: HydrogenBondRows::Candidates(table),
            candidate_distance: Some(maximum_distance),
            pose: None,
        }
    }

    fn len(&self) -> usize {
        match &self.rows {
            HydrogenBondRows::Endpoints(rows) => rows.len(),
            #[cfg(not(target_arch = "wasm32"))]
            HydrogenBondRows::MolFrame(table) => table.len(),
            #[cfg(not(target_arch = "wasm32"))]
            HydrogenBondRows::Candidates(table) => table.len(),
        }
    }

    fn endpoints(&self, index: usize) -> Option<[u32; 3]> {
        match &self.rows {
            HydrogenBondRows::Endpoints(rows) => rows.get(index).copied(),
            #[cfg(not(target_arch = "wasm32"))]
            HydrogenBondRows::MolFrame(table) => table
                .row(index)
                .map(|row| [row.donor.get(), row.hydrogen.get(), row.acceptor.get()]),
            #[cfg(not(target_arch = "wasm32"))]
            HydrogenBondRows::Candidates(table) => table
                .row(index)
                .map(|row| [row.donor.get(), row.hydrogen.get(), row.acceptor.get()]),
        }
    }

    pub(crate) fn content_hash(&self) -> &str {
        &self.source.content_hash
    }

    pub(crate) fn validate(&self) -> Result<(), Error> {
        if self.content_hash().trim().is_empty() || self.len() > u32::MAX as usize {
            return Err(invalid(
                "hydrogen-bond binding requires a source identity and addressable rows",
            ));
        }
        if self
            .candidate_distance
            .is_some_and(|distance| !distance.is_finite() || distance <= 0.0)
        {
            return Err(invalid(
                "hydrogen-bond candidate distance must be finite and positive",
            ));
        }
        for index in 0..self.len() {
            let Some([donor, hydrogen, acceptor]) = self.endpoints(index) else {
                return Err(invalid("hydrogen-bond table has an incomplete row"));
            };
            if donor == hydrogen || donor == acceptor || hydrogen == acceptor {
                return Err(invalid(
                    "hydrogen-bond endpoints must name three distinct atoms",
                ));
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let HydrogenBondRows::MolFrame(table) = &self.rows {
            validate_geometry(table)?;
        }
        Ok(())
    }
}

pub(super) fn lower(
    spec: &HydrogenBonds,
    lowering: &mut super::lower::OverlayLowering<'_>,
) -> Result<Option<molgfx_core::RelationBatchHandle>, Error> {
    use molgfx_core::{
        Relation, RelationBatch, RelationPattern, RelationStyle, RowDomain, RowEntityRef,
        SourceNamespace, SourceRows, SpatialAnchor,
    };
    let Some(binding) = lowering.bindings.hydrogen_bonds(&spec.source.content_hash) else {
        return Ok(None);
    };
    if let Some(prepared_distance) = binding.candidate_distance {
        let Some(geometry) = spec.geometry else {
            return Err(invalid(
                "hydrogen-bond candidates require instantaneous geometric cutoffs",
            ));
        };
        if geometry.maximum_endpoint_distance > prepared_distance {
            return Err(invalid(
                "hydrogen-bond cutoff exceeds the prepared candidate distance envelope",
            ));
        }
    }
    let source = lowering
        .structures
        .get(&spec.structure)
        .ok_or_else(|| invalid("hydrogen bonds target an unbound structure"))?;
    let handle = *lowering
        .handles
        .get(&spec.structure)
        .ok_or_else(|| invalid("hydrogen bonds target an unbound structure"))?;
    let selected = source.select_compiled(&*spec.selection.compiled()?)?;
    let atom_count = source.coordinates().len();
    let mut relations = Vec::with_capacity(binding.len());
    let mut keys = Vec::with_capacity(binding.len());
    let mut geometry = Vec::new();
    if spec.geometry.is_some() {
        geometry.reserve(binding.len());
    }
    for index in 0..binding.len() {
        let Some([donor, hydrogen, acceptor]) = binding.endpoints(index) else {
            return Err(invalid("hydrogen-bond table has an incomplete row"));
        };
        if [donor, hydrogen, acceptor]
            .iter()
            .any(|&atom| atom as usize >= atom_count)
        {
            return Err(invalid(
                "hydrogen-bond table addresses atoms outside its structure",
            ));
        }
        if !selected.contains(donor) || !selected.contains(acceptor) {
            continue;
        }
        relations.push(Relation {
            start: SpatialAnchor::entity(RowEntityRef::new(RowDomain::Atoms(handle), donor))?,
            end: SpatialAnchor::entity(RowEntityRef::new(RowDomain::Atoms(handle), acceptor))?,
        });
        keys.push(
            u64::try_from(index)
                .map_err(|_| invalid("hydrogen-bond source row is not addressable"))?,
        );
        if let Some(policy) = spec.geometry {
            geometry.push(molgfx_core::OrientedRelation {
                pivot: RowEntityRef::new(RowDomain::Atoms(handle), hydrogen),
                geometry: policy,
            });
        }
    }
    if relations.is_empty() {
        return Ok(None);
    }
    let hash = crate::spec::stable_json_hash(&(&spec.source, spec.structure));
    let prefix = hash
        .get(..16)
        .ok_or_else(|| invalid("hydrogen-bond source identity is incomplete"))?;
    let namespace = u64::from_str_radix(prefix, 16)
        .map_err(|_| invalid("hydrogen-bond source identity is invalid"))?;
    let batch = RelationBatch::new(
        Arc::from(relations),
        SourceRows::keyed(SourceNamespace(namespace), Arc::from(keys))?,
        RelationStyle {
            color: spec.color.native(),
            width_pixels: spec.width_pixels,
            opacity: spec.opacity,
            pattern: RelationPattern::Dotted,
            ..RelationStyle::default()
        },
    )?;
    let batch = if spec.geometry.is_some() {
        batch.with_oriented_geometry(Arc::from(geometry))?
    } else {
        batch
    };
    let pose = binding.pose().ok_or_else(|| {
        invalid("hydrogen-bond input requires its original molecular coordinate pose")
    })?;
    let batch = if binding.candidate_distance.is_some() {
        batch.with_molecular_interval(pose.clone())?
    } else {
        batch.with_molecular_sample(pose.clone())?
    };
    Ok(Some(lowering.scene.add_relation_batch(batch)?))
}

fn invalid(message: &str) -> Error {
    Error::InvalidSpec(message.to_owned())
}

#[cfg(not(target_arch = "wasm32"))]
fn validate_geometry(table: &molframe::analysis::HydrogenBondTable) -> Result<(), Error> {
    for index in 0..table.len() {
        let Some(row) = table.row(index) else {
            return Err(invalid("hydrogen-bond table has an incomplete row"));
        };
        if !row.donor_acceptor_distance.is_finite()
            || row.donor_acceptor_distance <= 0.0
            || !row.hydrogen_acceptor_distance.is_finite()
            || row.hydrogen_acceptor_distance <= 0.0
            || !row.angle_degrees.is_finite()
            || !(0.0..=180.0).contains(&row.angle_degrees)
        {
            return Err(invalid(
                "hydrogen-bond table contains invalid oriented geometry",
            ));
        }
    }
    Ok(())
}
