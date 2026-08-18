//! Transform-only instances of a shared caller mesh.

use crate::MeshHandle;
use pdviewx_math::Mat4;

/// One transform-only occurrence of a shared [`crate::Mesh`].
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MeshInstance {
    mesh: MeshHandle,
    transform: Mat4,
    visible: bool,
}

impl MeshInstance {
    /// Creates a visible instance.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CoreError::InvalidMesh`] for a non-finite transform.
    pub fn new(mesh: MeshHandle, transform: Mat4) -> Result<Self, crate::CoreError> {
        if !transform.is_finite() {
            return Err(crate::CoreError::InvalidMesh {
                reason: "mesh instance transforms must be finite",
            });
        }
        Ok(Self {
            mesh,
            transform,
            visible: true,
        })
    }

    /// Shared source mesh.
    #[must_use]
    pub const fn mesh(&self) -> MeshHandle {
        self.mesh
    }

    /// Instance-local transform, applied before structure placement.
    #[must_use]
    pub const fn transform(&self) -> Mat4 {
        self.transform
    }

    /// Replaces the instance transform.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CoreError::InvalidMesh`] for a non-finite transform.
    pub fn set_transform(&mut self, transform: Mat4) -> Result<(), crate::CoreError> {
        if !transform.is_finite() {
            return Err(crate::CoreError::InvalidMesh {
                reason: "mesh instance transforms must be finite",
            });
        }
        self.transform = transform;
        Ok(())
    }

    /// Whether this occurrence participates in its mesh batch.
    #[must_use]
    pub const fn visible(&self) -> bool {
        self.visible
    }

    /// Shows or hides this occurrence without changing its stable handle.
    pub const fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}
