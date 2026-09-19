//! Caller-supplied triangle geometry.
//!
//! Every other primitive in the scene is derived from the structure: spheres
//! from atoms, ribbons from a backbone trace, surfaces from an implicit field.
//! This one is not derived from anything — the caller hands over triangles with
//! their own normals and per-vertex colours. It is how a cone, an arbitrary
//! polygon, a membrane slab or an imported mesh enters a scene without the
//! engine pretending it inferred them.
//!
//! Because it carries no structural provenance, a mesh is presentation state.
//! It never contributes atoms, is never picked as chemistry, and never takes
//! part in measurement or interaction detection.

use crate::storage::handle::StructureHandle;
use crate::{ClipSet, error::CoreError};
use molgfx_math::{Rgba8, Vec3};

use super::material::Material;
use super::{SurfaceComponentPolicy, SurfaceComponentThreshold};

#[cfg(test)]
#[path = "mesh_tests.rs"]
mod tests;

/// Largest mesh a single primitive may carry, so one malformed caller buffer
/// cannot exhaust device memory.
pub const MAX_MESH_VERTICES: usize = 4_000_000;

/// How an ordered vertex stream is assembled into triangles.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MeshTopology {
    /// Each consecutive group of three vertices is one triangle.
    Triangles,
    /// Each vertex after the first two completes a triangle, with winding
    /// corrected on alternating faces.
    TriangleStrip,
    /// The first vertex is shared by every triangle in the fan.
    TriangleFan,
    /// Each consecutive group of four vertices is split along `(0, 2)`.
    Quads,
}

/// Which triangle winding is visible for a caller mesh.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum FaceVisibility {
    /// Both windings draw, suitable for sheets and incomplete surfaces.
    #[default]
    DoubleSided,
    /// Only counter-clockwise front faces draw.
    FrontOnly,
    /// Only clockwise back faces draw.
    BackOnly,
}

/// One caller-supplied vertex. The layout matches the generated cartoon vertex,
/// so meshes draw through the pipeline that already exists rather than adding a
/// second one.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MeshVertex {
    /// Model-space position.
    pub position: Vec3,
    /// Model-space normal; normalized while validating.
    pub normal: Vec3,
    /// Per-vertex display colour.
    pub color: Rgba8,
}

/// A validated triangle mesh owned by a structure, so it follows that
/// structure's placement.
#[derive(Clone, PartialEq, Debug)]
pub struct Mesh {
    owner: StructureHandle,
    vertices: Vec<MeshVertex>,
    indices: Vec<u32>,
    material: Material,
    clipping: ClipSet,
    face_visibility: FaceVisibility,
    component_policy: SurfaceComponentPolicy,
    visible: bool,
}

impl Mesh {
    /// Assembles a non-indexed vertex stream into a validated triangle mesh.
    ///
    /// The conversion is deterministic and happens once when scene state is
    /// authored. Rendering still uses the resident indexed mesh path.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidMesh`] when the stream has too few vertices
    /// or cannot be divided according to `topology`.
    pub fn from_topology(
        owner: StructureHandle,
        vertices: Vec<MeshVertex>,
        topology: MeshTopology,
        material: Material,
    ) -> Result<Self, CoreError> {
        let indices = topology_indices(vertices.len(), topology)?;
        Self::new(owner, vertices, indices, material)
    }

    /// Validates and stores caller triangles.
    ///
    /// Normals are normalized; a degenerate normal falls back to the triangle
    /// the vertex belongs to being flat-shaded by the pass, which is better
    /// than propagating a zero vector into the lighting.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidMesh`] for an empty mesh, an index count
    /// that is not a multiple of three, an index outside the vertex range, a
    /// non-finite position, or a mesh beyond [`MAX_MESH_VERTICES`].
    pub fn new(
        owner: StructureHandle,
        vertices: Vec<MeshVertex>,
        indices: Vec<u32>,
        material: Material,
    ) -> Result<Self, CoreError> {
        if vertices.is_empty() || indices.is_empty() {
            return Err(CoreError::InvalidMesh {
                reason: "a mesh needs at least one triangle",
            });
        }
        if vertices.len() > MAX_MESH_VERTICES {
            return Err(CoreError::InvalidMesh {
                reason: "mesh exceeds the vertex ceiling",
            });
        }
        if !indices.len().is_multiple_of(3) {
            return Err(CoreError::InvalidMesh {
                reason: "index count must be a multiple of three",
            });
        }
        let Ok(vertex_count) = u32::try_from(vertices.len()) else {
            return Err(CoreError::InvalidMesh {
                reason: "mesh exceeds the vertex ceiling",
            });
        };
        if indices.iter().any(|index| *index >= vertex_count) {
            return Err(CoreError::InvalidMesh {
                reason: "an index addresses no vertex",
            });
        }
        if vertices.iter().any(|vertex| !vertex.position.is_finite()) {
            return Err(CoreError::InvalidMesh {
                reason: "vertex positions must be finite",
            });
        }
        let vertices = vertices
            .into_iter()
            .map(|vertex| MeshVertex {
                normal: vertex.normal.try_normalize().map_or(Vec3::Y, |unit| unit),
                ..vertex
            })
            .collect();
        Ok(Self {
            owner,
            vertices,
            indices,
            material,
            clipping: ClipSet::default(),
            face_visibility: FaceVisibility::DoubleSided,
            component_policy: SurfaceComponentPolicy::default(),
            visible: true,
        })
    }

    /// Converts a `molframe` indexed surface into renderable triangles.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidMesh`] for invalid component limits or a
    /// provider mesh that violates the portable mesh contract.
    pub fn from_surface(
        owner: StructureHandle,
        surface: &molframe::surface::IndexedSurfaceMesh,
        color: Rgba8,
        material: Material,
        policy: SurfaceComponentPolicy,
    ) -> Result<Self, CoreError> {
        let minimum_area = match policy.threshold() {
            SurfaceComponentThreshold::Disabled => 0.0,
            SurfaceComponentThreshold::Area(area) => area,
            SurfaceComponentThreshold::Volume(_) | SurfaceComponentThreshold::Voxels(_) => {
                return Err(CoreError::InvalidMesh {
                    reason: "indexed meshes support area component thresholds only",
                });
            }
        };
        let maximum_components = policy
            .maximum_components()
            .map(usize::try_from)
            .transpose()
            .map_err(|_| CoreError::InvalidMesh {
                reason: "surface component maximum exceeds the host index range",
            })?;
        let filtered = molframe::surface::filter_surface_components(
            surface,
            molframe::surface::SurfaceComponentFilter {
                minimum_area,
                maximum_components,
            },
        )
        .map_err(|_| CoreError::InvalidMesh {
            reason: "surface component policy is invalid",
        })?;
        let vertices = filtered
            .vertices
            .iter()
            .zip(&filtered.vertex_normals)
            .map(|(position, normal)| MeshVertex {
                position: Vec3::from_array(*position),
                normal: Vec3::from_array(*normal),
                color,
            })
            .collect();
        let indices = filtered.faces.iter().flat_map(|face| face.0).collect();
        let mut mesh = Self::new(owner, vertices, indices, material)?;
        mesh.component_policy = policy;
        Ok(mesh)
    }

    /// The structure whose placement carries this mesh.
    #[must_use]
    pub const fn owner(&self) -> StructureHandle {
        self.owner
    }

    /// Validated vertices.
    #[must_use]
    pub fn vertices(&self) -> &[MeshVertex] {
        &self.vertices
    }

    /// Validated triangle indices.
    #[must_use]
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    /// Surface response.
    #[must_use]
    pub const fn material(&self) -> Material {
        self.material
    }

    /// Per-mesh clipping state.
    #[must_use]
    pub const fn clipping(&self) -> ClipSet {
        self.clipping
    }

    /// Replaces per-mesh clipping state.
    pub const fn set_clipping(&mut self, clipping: ClipSet) {
        self.clipping = clipping;
    }

    /// Triangle winding policy.
    #[must_use]
    pub const fn face_visibility(&self) -> FaceVisibility {
        self.face_visibility
    }

    /// Provider component policy recorded with this mesh.
    #[must_use]
    pub const fn component_policy(&self) -> SurfaceComponentPolicy {
        self.component_policy
    }

    pub(crate) const fn set_owner(&mut self, owner: StructureHandle) {
        self.owner = owner;
    }

    /// Replaces triangle winding policy.
    pub const fn set_face_visibility(&mut self, visibility: FaceVisibility) {
        self.face_visibility = visibility;
    }

    /// Replaces the surface response.
    pub const fn set_material(&mut self, material: Material) {
        self.material = material;
    }

    /// Whether the mesh draws.
    #[must_use]
    pub const fn visible(&self) -> bool {
        self.visible
    }

    /// Shows or hides the mesh without discarding it.
    pub const fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Model-space bounds, for scene fitting.
    #[must_use]
    pub fn bounds(&self) -> molgfx_math::Aabb {
        molgfx_math::Aabb::from_points(self.vertices.iter().map(|vertex| vertex.position))
    }
}

fn topology_indices(vertex_count: usize, topology: MeshTopology) -> Result<Vec<u32>, CoreError> {
    if vertex_count > MAX_MESH_VERTICES {
        return Err(CoreError::InvalidMesh {
            reason: "mesh exceeds the vertex ceiling",
        });
    }
    let invalid = |reason| CoreError::InvalidMesh { reason };
    let mut indices = Vec::new();
    match topology {
        MeshTopology::Triangles => {
            if vertex_count < 3 || !vertex_count.is_multiple_of(3) {
                return Err(invalid("triangle streams require groups of three vertices"));
            }
            indices.extend((0..vertex_count).map(index_u32));
        }
        MeshTopology::TriangleStrip => {
            if vertex_count < 3 {
                return Err(invalid("triangle strips require at least three vertices"));
            }
            indices.reserve((vertex_count - 2) * 3);
            for index in 0..vertex_count - 2 {
                let a = index_u32(index);
                let b = index_u32(index + 1);
                let c = index_u32(index + 2);
                if index.is_multiple_of(2) {
                    indices.extend([a, b, c]);
                } else {
                    indices.extend([b, a, c]);
                }
            }
        }
        MeshTopology::TriangleFan => {
            if vertex_count < 3 {
                return Err(invalid("triangle fans require at least three vertices"));
            }
            indices.reserve((vertex_count - 2) * 3);
            for index in 1..vertex_count - 1 {
                indices.extend([0, index_u32(index), index_u32(index + 1)]);
            }
        }
        MeshTopology::Quads => {
            if vertex_count < 4 || !vertex_count.is_multiple_of(4) {
                return Err(invalid("quad streams require groups of four vertices"));
            }
            indices.reserve(vertex_count / 4 * 6);
            for base in (0..vertex_count).step_by(4) {
                let a = index_u32(base);
                let b = index_u32(base + 1);
                let c = index_u32(base + 2);
                let d = index_u32(base + 3);
                indices.extend([a, b, c, a, c, d]);
            }
        }
    }
    Ok(indices)
}

fn index_u32(index: usize) -> u32 {
    u32::try_from(index).map_or(u32::MAX, |value| value)
}
