//! Shared indexed meshes and coarse point proxies.

use super::payload::{bytes, invalid, row_count};
use crate::DatasetError;
use std::sync::Arc;

/// Indexed geometry represented entirely by portable scalar arrays.
#[derive(Clone, Debug)]
pub struct MeshChunkPayload {
    positions: Arc<[[f32; 3]]>,
    normals: Arc<[[f32; 3]]>,
    colors: Arc<[[u8; 4]]>,
    indices: Arc<[u32]>,
}

impl MeshChunkPayload {
    /// Validates aligned vertices and chunk-local triangle indices.
    ///
    /// # Errors
    ///
    /// Returns [`DatasetError::InvalidPayload`] for invalid columns, values,
    /// triangle indices or chunk-local row count.
    pub fn new(
        positions: Arc<[[f32; 3]]>,
        normals: Arc<[[f32; 3]]>,
        colors: Arc<[[u8; 4]]>,
        indices: Arc<[u32]>,
    ) -> Result<Self, DatasetError> {
        let vertices = row_count(positions.len())?;
        if vertices == 0
            || normals.len() != positions.len()
            || colors.len() != positions.len()
            || indices.is_empty()
            || !indices.len().is_multiple_of(3)
        {
            return Err(invalid(
                "mesh columns must be aligned and contain triangles",
            ));
        }
        if positions
            .iter()
            .chain(normals.iter())
            .flatten()
            .any(|value| !value.is_finite())
            || indices.iter().any(|index| *index >= vertices)
        {
            return Err(invalid(
                "mesh values must be finite and indices chunk-local",
            ));
        }
        Ok(Self {
            positions,
            normals,
            colors,
            indices,
        })
    }

    /// Model-space positions shared with the caller.
    #[must_use]
    pub fn positions(&self) -> &Arc<[[f32; 3]]> {
        &self.positions
    }

    /// Model-space normals shared with the caller.
    #[must_use]
    pub fn normals(&self) -> &Arc<[[f32; 3]]> {
        &self.normals
    }

    /// RGBA8 colors shared with the caller.
    #[must_use]
    pub fn colors(&self) -> &Arc<[[u8; 4]]> {
        &self.colors
    }

    /// Triangle indices local to this chunk.
    #[must_use]
    pub fn indices(&self) -> &Arc<[u32]> {
        &self.indices
    }

    pub(super) fn minimum_host_bytes(&self) -> Result<u64, DatasetError> {
        let positions = bytes(self.positions.len(), 12)?;
        let normals = bytes(self.normals.len(), 12)?;
        let colors = bytes(self.colors.len(), 4)?;
        let indices = bytes(self.indices.len(), 4)?;
        positions
            .checked_add(normals)
            .and_then(|value| value.checked_add(colors))
            .and_then(|value| value.checked_add(indices))
            .ok_or(DatasetError::PayloadByteSizeOverflow)
    }
}

/// Coarse point proxies retained while detail is absent.
#[derive(Clone, Debug)]
pub struct ProxyChunkPayload {
    centers: Arc<[[f32; 3]]>,
    radii: Arc<[f32]>,
    colors: Arc<[[u8; 4]]>,
}

impl ProxyChunkPayload {
    /// Validates aligned finite proxies without copying them.
    ///
    /// # Errors
    ///
    /// Returns [`DatasetError::InvalidPayload`] for invalid columns, values or
    /// chunk-local row count.
    pub fn new(
        centers: Arc<[[f32; 3]]>,
        radii: Arc<[f32]>,
        colors: Arc<[[u8; 4]]>,
    ) -> Result<Self, DatasetError> {
        row_count(centers.len())?;
        if centers.is_empty() || radii.len() != centers.len() || colors.len() != centers.len() {
            return Err(invalid("proxy columns must be non-empty and aligned"));
        }
        if centers.iter().flatten().any(|value| !value.is_finite())
            || radii
                .iter()
                .any(|radius| !radius.is_finite() || *radius < 0.0)
        {
            return Err(invalid("proxy centers and radii must be finite"));
        }
        Ok(Self {
            centers,
            radii,
            colors,
        })
    }

    /// Dataset-space centers shared with the caller.
    #[must_use]
    pub fn centers(&self) -> &Arc<[[f32; 3]]> {
        &self.centers
    }

    /// Conservative non-negative radii shared with the caller.
    #[must_use]
    pub fn radii(&self) -> &Arc<[f32]> {
        &self.radii
    }

    /// RGBA8 colors shared with the caller.
    #[must_use]
    pub fn colors(&self) -> &Arc<[[u8; 4]]> {
        &self.colors
    }

    pub(super) fn minimum_host_bytes(&self) -> Result<u64, DatasetError> {
        let centers = bytes(self.centers.len(), 12)?;
        let radii = bytes(self.radii.len(), 4)?;
        let colors = bytes(self.colors.len(), 4)?;
        centers
            .checked_add(radii)
            .and_then(|value| value.checked_add(colors))
            .ok_or(DatasetError::PayloadByteSizeOverflow)
    }
}
