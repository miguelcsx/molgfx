//! Shared indexed geometry and extraction failures.

use bytemuck::{Pod, Zeroable};
use std::fmt;

/// An affine world-space boundary vertex with an exact categorical identity.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug, Pod, Zeroable)]
pub struct BoundaryVertex {
    /// World-space position in source coordinate units.
    pub position: [f32; 3],
    /// Exact source label, or zero for a scalar field.
    pub label: u32,
    /// Outward unit normal; zero at a singular field gradient.
    pub normal: [f32; 3],
    pub(super) padding: u32,
}

/// Deterministic, deduplicated geometry derived without copying source columns.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct BoundaryMesh {
    /// Vertices shared by adjacent triangles of the same label.
    pub vertices: Vec<BoundaryVertex>,
    /// Outward-wound triangles referring to `vertices`.
    pub triangles: Vec<[u32; 3]>,
}

/// Extraction failed before a complete boundary could be returned.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FieldError {
    /// An authored scalar level was not finite.
    InvalidLevel,
    /// The output exceeds the portable index representation.
    IndexOverflow,
    /// Host memory could not be reserved for complete geometry.
    Allocation,
    /// A transformed position or gradient exceeds finite floating-point range.
    NonfiniteGeometry,
}

impl fmt::Display for FieldError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLevel => "field boundary level must be finite",
            Self::IndexOverflow => "field boundary exceeds the portable index range",
            Self::Allocation => "field boundary geometry could not be allocated",
            Self::NonfiniteGeometry => {
                "field boundary has an unrepresentable world position or normal"
            }
        })
    }
}

impl std::error::Error for FieldError {}
