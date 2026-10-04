//! The semantic entity a pick resolves to.

use crate::Error;

/// Semantic entity namespace returned by picking.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PickKind {
    /// Molecular atom.
    Atom,
    /// Molecular bond.
    Bond,
    /// Detected or caller-supplied interaction.
    Interaction,
    /// Annotation label.
    Label,
    /// Distance, angle or dihedral measurement.
    Measurement,
    /// Analytic extension primitive.
    Primitive,
    /// Mesh extension entity.
    Mesh,
    /// Ligand-pose candidate batch.
    LigandPoseBatch,
    /// Analytic guide.
    Guide,
    /// Time-dependent bond.
    DynamicBond,
    /// Data-extension point.
    Point,
    /// Shared-template occurrence.
    Instance,
    /// Part of a shared-template occurrence.
    TemplatePart,
    /// Data-extension relation.
    Relation,
    /// Categorical volume segment.
    VolumeSegment,
}

/// Stable semantic provenance resolved from one physical GPU token.
#[derive(Clone, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct PickResult {
    /// Entity namespace.
    pub kind: PickKind,
    /// Provider dataset identity for molecular and extension entities.
    pub dataset: Option<u64>,
    /// Provider chunk identity when data is streamed.
    pub chunk: Option<u64>,
    /// Stable logical source row.
    pub row: Option<u64>,
    /// Exact categorical label for a volume segment.
    pub volume_label: Option<u32>,
    /// Exact generational categorical grid handle captured by the renderer.
    #[serde(default)]
    pub segmentation: Option<molgfx_core::SegmentationHandle>,
    /// Source scene lifetime captured with a categorical grid handle.
    #[serde(default)]
    pub source_id: Option<u64>,
}

impl PickResult {
    /// Deterministic JSON payload used by browser events.
    ///
    /// # Errors
    ///
    /// Returns an error only if serialization of the fixed schema fails.
    pub fn to_json(&self) -> Result<String, Error> {
        serde_json::to_string(self).map_err(Error::from)
    }
}
