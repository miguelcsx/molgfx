//! GPU-ready analytical surfaces with atom colors and triangle wire coordinates.
//! Packing is O(vertices + faces); coordinates remain shared in the source asset.
use super::{RibbonDeformation, RibbonMesh, RibbonVertex};
use crate::PackingError;
use molgfx_core::{AtomSelection, EntityId, EntityKind, MolecularSurfaceGeometry};
use molgfx_math::Rgba8;

impl RibbonMesh {
    /// Packs provider triangles, retaining original molecular identities.
    /// Only faces whose three vertex owners are selected are drawn. Wire mode
    /// carries screen-linear barycentric coordinates for the actual triangle
    /// edges, rather than an unrelated grid drawn over the molecular boundary.
    ///
    /// # Errors
    /// Rejects geometry beyond the shared resident vertex/index budget.
    pub fn generate_molecular_surface(
        &mut self,
        surface: &MolecularSurfaceGeometry,
        selection: &AtomSelection,
        wire: bool,
        color: Rgba8,
    ) -> Result<(), PackingError> {
        self.clear();
        let mesh = surface.mesh();
        let faces = mesh.geometry.faces.iter().filter(|face| {
            face.0
                .iter()
                .all(|&vertex| selection.contains(mesh.atom_rows[vertex as usize]))
        });
        if wire {
            let count =
                faces
                    .clone()
                    .count()
                    .checked_mul(3)
                    .ok_or(PackingError::GeometryBudget {
                        resource: "molecular surface wire vertices",
                    })?;
            if count > molgfx_core::MAX_MESH_VERTICES {
                return Err(PackingError::GeometryBudget {
                    resource: "molecular surface wire vertices",
                });
            }
            self.vertices.reserve(count);
            self.indices.reserve(count);
            self.deformations.reserve(count);
            for face in faces {
                for (corner, &vertex) in face.0.iter().enumerate() {
                    self.indices.push(super::super::draw_limits::vertex_index(
                        self.vertices.len(),
                    )?);
                    self.vertices.push(surface_vertex(mesh, vertex, color)?);
                    let mut barycentric = [0.0; 4];
                    barycentric[corner] = 1.0;
                    self.deformations.push(RibbonDeformation {
                        controls: [u32::MAX; 4],
                        parameter: barycentric,
                    });
                }
            }
        } else {
            self.vertices.reserve(mesh.geometry.vertices.len());
            for index in 0..mesh.geometry.vertices.len() {
                self.vertices.push(surface_vertex(
                    mesh,
                    super::super::draw_limits::vertex_index(index)?,
                    color,
                )?);
            }
            for face in faces {
                self.indices.extend_from_slice(&face.0);
            }
            self.pad_static_deformations();
        }
        Ok(())
    }
}

fn surface_vertex(
    mesh: &molframe::surface::MolecularSurfaceMesh,
    vertex: u32,
    color: Rgba8,
) -> Result<RibbonVertex, PackingError> {
    let index = vertex as usize;
    let entity_id = EntityId::pack(EntityKind::Atom, u64::from(mesh.atom_rows[index]))?.0;
    Ok(RibbonVertex {
        position: mesh.geometry.vertices[index],
        normal: mesh.geometry.vertex_normals[index],
        entity_id,
        color,
    })
}
