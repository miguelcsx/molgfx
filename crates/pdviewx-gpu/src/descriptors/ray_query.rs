//! Backend-neutral acceleration-structure descriptors.

use crate::Device;

bitflags::bitflags! {
    /// Construction preferences for a BLAS or TLAS.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct AccelerationStructureFlags: u8 {
        /// Permit topology-preserving updates.
        const ALLOW_UPDATE = 1;
        /// Prefer traversal speed over build speed.
        const PREFER_FAST_TRACE = 1 << 1;
        /// Prefer build speed over traversal speed.
        const PREFER_FAST_BUILD = 1 << 2;
        /// Prefer a smaller device-memory footprint.
        const LOW_MEMORY = 1 << 3;
    }
}

bitflags::bitflags! {
    /// Per-geometry ray-query behavior.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct AccelerationGeometryFlags: u8 {
        /// Geometry has no any-hit rejection step.
        const OPAQUE = 1;
    }
}

/// Whether a build may update an existing acceleration structure.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AccelerationStructureUpdateMode {
    /// Always perform a complete build.
    #[default]
    Build,
    /// Prefer a topology-preserving update when supported.
    PreferUpdate,
}

/// Index encoding for triangle BLAS geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccelerationIndexFormat {
    /// Unsigned 16-bit indices.
    Uint16,
    /// Unsigned 32-bit indices.
    Uint32,
}

/// Size ceiling for one triangle geometry in a BLAS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TriangleGeometrySize {
    /// Maximum vertex count.
    pub vertex_count: u32,
    /// Optional index format and maximum index count.
    pub indices: Option<(AccelerationIndexFormat, u32)>,
    /// Geometry behavior.
    pub flags: AccelerationGeometryFlags,
}

/// Size ceiling for one procedural AABB geometry in a BLAS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AabbGeometrySize {
    /// Maximum primitive count.
    pub primitive_count: u32,
    /// Geometry behavior.
    pub flags: AccelerationGeometryFlags,
}

/// Geometry family and size ceilings used to allocate a BLAS.
#[derive(Clone, Copy, Debug)]
pub enum BlasGeometrySizes<'a> {
    /// Triangle geometries with `Float32x3` positions.
    Triangles(&'a [TriangleGeometrySize]),
    /// Procedural AABB geometries.
    Aabbs(&'a [AabbGeometrySize]),
}

/// Descriptor for allocating a BLAS.
#[derive(Clone, Copy, Debug)]
pub struct BlasDesc<'a> {
    /// Diagnostic label.
    pub label: &'static str,
    /// Construction preferences.
    pub flags: AccelerationStructureFlags,
    /// Build/update policy.
    pub update_mode: AccelerationStructureUpdateMode,
    /// Geometry family and allocation ceilings.
    pub geometries: BlasGeometrySizes<'a>,
}

/// Descriptor for allocating a TLAS.
#[derive(Clone, Copy, Debug)]
pub struct TlasDesc {
    /// Diagnostic label.
    pub label: &'static str,
    /// Maximum number of instances.
    pub max_instances: u32,
    /// Construction preferences.
    pub flags: AccelerationStructureFlags,
    /// Build/update policy.
    pub update_mode: AccelerationStructureUpdateMode,
}

/// Triangle geometry supplied to one BLAS build.
#[derive(Clone, Copy, Debug)]
pub struct TriangleGeometry<'a, D: Device> {
    /// Size descriptor used when the BLAS was allocated.
    pub size: &'a TriangleGeometrySize,
    /// Packed vertex storage.
    pub vertex_buffer: &'a D::Buffer,
    /// First vertex in the buffer.
    pub first_vertex: u32,
    /// Byte stride between vertices.
    pub vertex_stride: u64,
    /// Optional index buffer, first index and format.
    pub indices: Option<(&'a D::Buffer, u32, AccelerationIndexFormat)>,
}

/// Procedural AABB geometry supplied to one BLAS build.
#[derive(Clone, Copy, Debug)]
pub struct AabbGeometry<'a, D: Device> {
    /// Size descriptor used when the BLAS was allocated.
    pub size: &'a AabbGeometrySize,
    /// Packed min/max AABB storage.
    pub buffer: &'a D::Buffer,
    /// Byte offset to the first primitive.
    pub offset: u32,
    /// Byte stride between primitives; at least 24 and divisible by 8.
    pub stride: u64,
}

/// Geometry family supplied to one BLAS build.
#[derive(Clone, Copy, Debug)]
pub enum BlasGeometries<'a, D: Device> {
    /// Triangle geometries.
    Triangles(&'a [TriangleGeometry<'a, D>]),
    /// Procedural AABB geometries.
    Aabbs(&'a [AabbGeometry<'a, D>]),
}

/// One BLAS build operation.
#[derive(Clone, Copy, Debug)]
pub struct BlasBuildDesc<'a, D: Device> {
    /// Destination BLAS.
    pub blas: &'a D::Blas,
    /// Geometry buffers and ranges.
    pub geometries: BlasGeometries<'a, D>,
}

/// One instance stored in a TLAS.
#[derive(Clone, Copy, Debug)]
pub struct TlasInstance<'a, D: Device> {
    /// Referenced BLAS.
    pub blas: &'a D::Blas,
    /// Row-major affine 3x4 transform.
    pub transform: [f32; 12],
    /// Shader-visible 24-bit application value.
    pub custom_data: u32,
    /// Eight-bit ray mask.
    pub mask: u8,
}
