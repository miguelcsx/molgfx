//! Schema-8 records for generic row tables and declarative domain visuals.

use super::manifest_io::PayloadReference;
use super::types::VisualStyleDescription;
use serde::{Deserialize, Serialize};

/// Process-independent identity of one exact row domain.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct RowDomainDescription {
    /// `atoms`, `points`, `instances`, `template-parts` or `relations`.
    pub kind: String,
    /// Generational table row.
    pub row: u32,
    /// Generational table generation.
    pub generation: u32,
}

/// External source-row namespace without embedding optional key arrays.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SourceRowsDescription {
    /// Caller-owned namespace.
    pub namespace: u64,
    /// Logical row count.
    pub row_count: u32,
    /// Whether the referenced payload carries explicit `u64` keys.
    pub keyed: bool,
}

/// Generic 12-byte point source and batch-wide fallback style.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct PointBatchDescription {
    /// Stable point-batch slot row.
    pub row: u32,
    /// Stable point-batch slot generation.
    pub generation: u32,
    /// External identity of logical point rows.
    pub source_rows: SourceRowsDescription,
    /// `disc` or `sphere`.
    pub glyph: String,
    /// Exact bits of the positive fallback radius.
    pub radius_bits: u32,
    /// Packed fallback color.
    pub color: [u8; 4],
    /// Whether the batch draws.
    pub visible: bool,
    /// Content-addressed positions, keys and immutable batch metadata.
    pub payload: PayloadReference,
}

/// Shared-template rigid-instance source.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct InstanceBatchDescription {
    /// Stable instance-batch slot row.
    pub row: u32,
    /// Stable instance-batch slot generation.
    pub generation: u32,
    /// External identity of rigid-instance rows.
    pub source_rows: SourceRowsDescription,
    /// External identity of flattened analytic-template parts.
    pub template_rows: SourceRowsDescription,
    /// Number of homogeneous sphere parts.
    pub sphere_count: u32,
    /// Number of homogeneous capsule parts.
    pub capsule_count: u32,
    /// Packed fallback color.
    pub color: [u8; 4],
    /// Whether the batch draws.
    pub visible: bool,
    /// Content-addressed template, transforms and both source-key columns.
    pub payload: PayloadReference,
}

/// One immutable typed column retained outside JSON.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AttributeDescription {
    /// Stable attribute slot row.
    pub row: u32,
    /// Stable attribute slot generation.
    pub generation: u32,
    /// Exact target row table.
    pub domain: RowDomainDescription,
    /// Caller-authored introspection name.
    pub name: String,
    /// Optional uninterpreted measured quantity.
    pub quantity: Option<String>,
    /// Optional caller unit string.
    pub unit: Option<String>,
    /// Optional caller method, dataset or evidence identifier.
    pub provenance: Option<String>,
    /// `scalar`, `category`, `vector` or `color`.
    pub kind: String,
    /// Number of physical rows.
    pub row_count: u32,
    /// Deterministic typed-column fingerprint.
    pub fingerprint: u64,
    /// Content-addressed native-width values.
    pub payload: PayloadReference,
}

/// Generic spatial relation source and visual fallback.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct RelationBatchDescription {
    /// Stable relation-batch slot row.
    pub row: u32,
    /// Stable relation-batch slot generation.
    pub generation: u32,
    /// External identity of logical relation rows.
    pub source_rows: SourceRowsDescription,
    /// Exact bits of the positive fallback width.
    pub width_bits: u32,
    /// Packed fallback color.
    pub color: [u8; 4],
    /// Exact bits of the bounded fallback opacity.
    pub opacity_bits: u32,
    /// Exact bits of the non-negative start/end screen-space trims.
    pub endpoint_inset_bits: [u32; 2],
    /// Whether opaque scene geometry occludes the connector as a background overlay.
    pub depth_behind_anchors: bool,
    /// `solid`, `dashed` or `dotted`.
    pub pattern: String,
    /// Whether the batch draws.
    pub visible: bool,
    /// Content-addressed anchors, keys and immutable batch metadata.
    pub payload: PayloadReference,
}

/// Visual program and parameters attached directly to one generic domain.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct DomainVisualDescription {
    /// Exact row table receiving the visual program.
    pub domain: RowDomainDescription,
    /// Stable caller-defined draw/composition order.
    pub order: i32,
    /// Typed bounded program and its parameter values.
    pub style: VisualStyleDescription,
}
