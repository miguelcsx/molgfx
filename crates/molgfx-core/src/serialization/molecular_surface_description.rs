//! Restorable analytical triangle assets with original molecular atom identity.
use crate::{CoreError, MolecularSurfaceGeometry};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Complete analytical molecular triangle asset, without provider process state.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct MolecularSurfaceDescription {
    /// Generated surface positions, not atom coordinates.
    pub vertices: Vec<[f32; 3]>,
    /// Provider's analytic unit normals.
    pub normals: Vec<[f32; 3]>,
    /// Triangle vertex indices.
    pub faces: Vec<[u32; 3]>,
    /// Original molecular atom row for each vertex.
    pub atom_rows: Vec<u32>,
    /// Exactly zero-area provider faces omitted during scientific decoding.
    #[serde(default)]
    pub discarded_degenerate_faces: u64,
}

impl MolecularSurfaceDescription {
    /// Copies a derived surface asset for transport; molecular coordinates remain borrowed.
    #[must_use]
    pub fn from_geometry(value: &MolecularSurfaceGeometry) -> Self {
        let mesh = value.mesh();
        Self {
            vertices: mesh.geometry.vertices.clone(),
            normals: mesh.geometry.vertex_normals.clone(),
            faces: mesh.geometry.faces.iter().map(|face| face.0).collect(),
            atom_rows: mesh.atom_rows.to_vec(),
            discarded_degenerate_faces: mesh.discarded_degenerate_faces,
        }
    }

    /// Validates and restores a transported triangle asset.
    ///
    /// # Errors
    /// Rejects oversized, malformed or degenerate geometry and invalid normals.
    pub fn geometry(&self) -> Result<MolecularSurfaceGeometry, CoreError> {
        self.clone().into_geometry()
    }

    /// Restores a transported asset by moving its derived columns into shared storage.
    ///
    /// # Errors
    /// Rejects oversized or malformed geometry and invalid normals.
    pub fn into_geometry(self) -> Result<MolecularSurfaceGeometry, CoreError> {
        self.into_geometry_with_cancellation(&molframe::CancellationToken::default())
    }

    /// Moves transported columns into shared storage with cancellable validation.
    /// # Errors
    /// Refuses invalid columns, oversized geometry or cancelled restoration.
    pub fn into_geometry_with_cancellation(
        self,
        cancellation: &molframe::CancellationToken,
    ) -> Result<MolecularSurfaceGeometry, CoreError> {
        if cancellation.is_cancelled() {
            return Err(CoreError::InvalidMesh {
                reason: "surface restoration was cancelled",
            });
        }
        if self.normals.len() != self.vertices.len() || self.atom_rows.len() != self.vertices.len()
        {
            return Err(CoreError::InvalidMesh {
                reason: "molecular surface columns have different lengths",
            });
        }
        if self.vertices.len() > crate::MAX_MESH_VERTICES || self.faces.len() > 8_000_000 {
            return Err(CoreError::InvalidMesh {
                reason: "restored molecular surface exceeds geometry limits",
            });
        }
        let geometry = molframe::surface::IndexedSurfaceMesh::from_analytic_normals(
            self.vertices,
            self.faces
                .into_iter()
                .map(molframe::surface::SurfaceFace)
                .collect(),
            self.normals,
            cancellation,
        )
        .map_err(|_| CoreError::InvalidMesh {
            reason: "analytical surface geometry could not be restored",
        })?;
        MolecularSurfaceGeometry::with_cancellation(
            Arc::new(molframe::surface::MolecularSurfaceMesh {
                geometry,
                atom_rows: self.atom_rows.into_boxed_slice(),
                discarded_degenerate_faces: self.discarded_degenerate_faces,
            }),
            cancellation,
        )
    }
}
