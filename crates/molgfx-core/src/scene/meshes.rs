//! Mesh-table editing kept separate from the central scene lifecycle.

use crate::{CoreError, Mesh, MeshHandle, Scene};

impl Scene {
    /// Stores one validated caller mesh.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] when the owning structure is absent.
    pub fn add_mesh(&mut self, mesh: Mesh) -> Result<MeshHandle, CoreError> {
        if self.structure(mesh.owner()).is_none() {
            return Err(CoreError::StaleHandle);
        }
        self.mesh_revision = self.mesh_revision.wrapping_add(1);
        Ok(MeshHandle(self.meshes.insert(mesh)))
    }

    /// Resolves a mesh handle.
    #[must_use]
    pub fn mesh(&self, handle: MeshHandle) -> Option<&Mesh> {
        self.meshes.get(handle.0)
    }

    /// Mutable resolution; bumps the mesh revision because any field edit can
    /// change what draws.
    pub fn mesh_mut(&mut self, handle: MeshHandle) -> Option<&mut Mesh> {
        let mesh = self.meshes.get_mut(handle.0)?;
        self.mesh_revision = self.mesh_revision.wrapping_add(1);
        Some(mesh)
    }

    /// Removes a mesh, leaving other handles valid.
    pub fn remove_mesh(&mut self, handle: MeshHandle) {
        if self.meshes.remove(handle.0).is_some() {
            self.mesh_revision = self.mesh_revision.wrapping_add(1);
        }
    }

    /// Every stored mesh in deterministic handle order.
    pub fn meshes(&self) -> impl Iterator<Item = (MeshHandle, &Mesh)> + '_ {
        self.meshes
            .iter()
            .map(|(raw, mesh)| (MeshHandle(raw), mesh))
    }

    /// Stable row written to the picking attachment for this mesh.
    #[must_use]
    pub const fn mesh_row(handle: MeshHandle) -> u32 {
        handle.row()
    }

    /// Resolves a picked mesh row while also checking its owning structure.
    #[must_use]
    pub fn mesh_for_entity(&self, entity: crate::EntityRef) -> Option<&Mesh> {
        if entity.kind != crate::EntityKind::Mesh {
            return None;
        }
        let (_, mesh) = self.meshes.get_index(entity.index)?;
        (mesh.owner() == entity.structure).then_some(mesh)
    }

    /// Revision of the mesh table, bumped by every edit.
    #[must_use]
    pub const fn mesh_revision(&self) -> u64 {
        self.mesh_revision
    }
}
