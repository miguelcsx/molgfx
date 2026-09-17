//! Stable editing operations for transform-only shared-mesh instances.

use crate::{CoreError, MeshHandle, MeshInstance, MeshInstanceHandle, Scene};

impl Scene {
    /// Adds an occurrence of an existing shared mesh.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] when the source mesh is absent.
    pub fn add_mesh_instance(
        &mut self,
        instance: MeshInstance,
    ) -> Result<MeshInstanceHandle, CoreError> {
        if self.mesh(instance.mesh()).is_none() {
            return Err(CoreError::StaleHandle);
        }
        self.mesh_revision = self.mesh_revision.wrapping_add(1);
        Ok(MeshInstanceHandle(self.mesh_instances.insert(instance)))
    }

    /// Resolves an instance handle.
    #[must_use]
    pub fn mesh_instance(&self, handle: MeshInstanceHandle) -> Option<&MeshInstance> {
        self.mesh_instances.get(handle.0)
    }

    /// Mutably resolves an instance and invalidates its resident batch.
    pub fn mesh_instance_mut(&mut self, handle: MeshInstanceHandle) -> Option<&mut MeshInstance> {
        let instance = self.mesh_instances.get_mut(handle.0)?;
        self.mesh_revision = self.mesh_revision.wrapping_add(1);
        Some(instance)
    }

    /// Removes one occurrence while retaining its shared mesh.
    pub fn remove_mesh_instance(&mut self, handle: MeshInstanceHandle) -> Option<MeshInstance> {
        let removed = self.mesh_instances.remove(handle.0);
        if removed.is_some() {
            self.mesh_revision = self.mesh_revision.wrapping_add(1);
        }
        removed
    }

    /// Iterates instances in stable handle order.
    pub fn mesh_instances(&self) -> impl Iterator<Item = (MeshInstanceHandle, &MeshInstance)> + '_ {
        self.mesh_instances
            .iter()
            .map(|(raw, value)| (MeshInstanceHandle(raw), value))
    }

    /// Visible transforms for one mesh, used to build one indirect batch.
    pub fn mesh_instance_transforms(
        &self,
        mesh: MeshHandle,
    ) -> impl Iterator<Item = molgfx_math::Mat4> + '_ {
        self.mesh_instances
            .iter()
            .map(|(_, value)| value)
            .filter(move |value| value.mesh() == mesh && value.visible())
            .map(MeshInstance::transform)
    }
}
