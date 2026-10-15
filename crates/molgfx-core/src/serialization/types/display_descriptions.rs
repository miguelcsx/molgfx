//! Manifest records for guides, labels and table counts.

use serde::{Deserialize, Serialize};

/// Stable generational identity for a scene-owned object.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ObjectIdentity {
    /// Slot row.
    pub row: u32,
    /// Slot generation.
    pub generation: u32,
}

/// An entity identity carried by an annotation or interaction anchor.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct EntityDescription {
    /// Owning structure identity.
    pub structure: ObjectIdentity,
    /// Stable entity kind name.
    pub kind: String,
    /// Source table row.
    pub index: u32,
}

/// A world-space anchor with optional entity provenance.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct AnchorDescription {
    /// Position in ångström.
    pub position: [f32; 3],
    /// Optional source entity.
    pub entity: Option<EntityDescription>,
}

/// Serialized guide style.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct GuideStyleDescription {
    /// Display colour.
    pub color: [u8; 4],
    /// Stable pattern name.
    pub pattern: String,
    /// Width in physical pixels.
    pub width_pixels: f32,
    /// Final opacity.
    pub opacity: f32,
    /// Repetition period in pixels.
    pub period_pixels: f32,
    /// Mark duty cycle.
    pub duty_cycle: f32,
    /// Stable cap name.
    pub cap: String,
    /// Arrowhead size in pixels.
    pub arrow_pixels: f32,
}

/// One caller-authored guide.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct GuideDescription {
    /// Stable slot row.
    pub row: u32,
    /// Slot generation.
    pub generation: u32,
    /// Owning structure.
    pub owner: ObjectIdentity,
    /// Model-space start.
    pub start: [f32; 3],
    /// Model-space end.
    pub end: [f32; 3],
    /// Guide presentation.
    pub style: GuideStyleDescription,
    /// Render visibility.
    pub visible: bool,
}

/// Serialized marker style.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct MarkerStyleDescription {
    /// Display colour.
    pub color: [u8; 4],
    /// Physical-pixel radius.
    pub radius_pixels: f32,
    /// Stable shape name.
    pub shape: String,
}

/// One persistent annotation.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct AnnotationDescription {
    /// Stable slot row.
    pub row: u32,
    /// Slot generation.
    pub generation: u32,
    /// Owning structure.
    pub owner: ObjectIdentity,
    /// Stable annotation kind.
    pub kind: String,
    /// Optional geometric anchor.
    pub anchor: Option<AnchorDescription>,
    /// Optional region selection.
    pub region: Option<ObjectIdentity>,
    /// Human-authored text.
    pub text: String,
    /// Marker presentation.
    pub marker: MarkerStyleDescription,
    /// Decluttering priority.
    pub priority: i16,
    /// Render visibility.
    pub visible: bool,
}

/// One persistent caller-computed measurement.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct MeasurementDescription {
    /// Stable slot row.
    pub row: u32,
    /// Slot generation.
    pub generation: u32,
    /// Owning structure.
    pub owner: ObjectIdentity,
    /// Stable measurement kind.
    pub kind: String,
    /// Ordered source anchors.
    pub anchors: Vec<AnchorDescription>,
    /// Caller-computed value.
    pub value: f32,
    /// Deterministic display label.
    pub label: String,
    /// Source computation identifier.
    pub provenance: String,
    /// Decluttering priority.
    pub priority: i16,
    /// Render visibility.
    pub visible: bool,
}

/// Counts of dense or caller-owned scene tables.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct TableCounts {
    /// Interaction edges.
    pub interactions: u64,
    /// Analytic guides.
    pub guides: u64,
    /// Notes and markers.
    pub annotations: u64,
    /// Caller-computed measurements.
    pub measurements: u64,
    /// Per-atom scalar columns.
    pub atom_properties: u64,
    /// Shared-mesh instances.
    pub mesh_instances: u64,
    /// Screen overlays.
    pub overlays: u64,
    /// Compact ligand pose batches.
    pub ligand_pose_batches: u64,
    /// Generic point batches.
    pub point_batches: u64,
    /// Shared-template instance batches.
    pub instance_batches: u64,
    /// Generic typed attribute columns.
    pub attributes: u64,
    /// Generic relation batches.
    pub relation_batches: u64,
    /// Generic domain visual descriptors.
    pub domain_visuals: u64,
}
