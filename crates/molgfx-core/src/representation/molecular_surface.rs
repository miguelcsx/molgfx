//! Shared provider geometry with molecular atom provenance, validated once.
//!
//! Validation is O(vertices + faces). Generated coordinates are a surface asset,
//! not a second atom-coordinate column; original atom rows drive color and picks.
use crate::{CoreError, MAX_MESH_VERTICES};
use sha2::{Digest, Sha256};
use std::{
    fmt::Debug,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

static NEXT_GEOMETRY_ID: AtomicU64 = AtomicU64::new(1);

/// Immutable analytical provider output, including any retained resource reservation.
/// Implementations must return the same complete geometry for their entire lifetime.
pub trait MolecularSurfaceSource: Debug + Send + Sync {
    /// Complete indexed surface, analytic normals and original atom identities.
    fn surface_mesh(&self) -> &molframe::surface::MolecularSurfaceMesh;
}

impl MolecularSurfaceSource for molframe::surface::MolecularSurfaceMesh {
    fn surface_mesh(&self) -> &molframe::surface::MolecularSurfaceMesh {
        self
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl MolecularSurfaceSource for molframe::surface::native_solver::NativeSurfaceMesh {
    fn surface_mesh(&self) -> &molframe::surface::MolecularSurfaceMesh {
        &self.mesh
    }
}

/// Immutable validated molecular triangle geometry, shared without copying.
#[derive(Clone, Debug)]
pub struct MolecularSurfaceGeometry {
    source: Arc<dyn MolecularSurfaceSource>,
    identity: u64,
    maximum_atom_row: u32,
    content_hash: [u8; 32],
    molecular_key: Option<[u64; 2]>,
    pose: Option<crate::MolecularPose>,
}

impl PartialEq for MolecularSurfaceGeometry {
    fn eq(&self, other: &Self) -> bool {
        self.molecular_key == other.molecular_key
            && self.pose == other.pose
            && Arc::ptr_eq(&self.source, &other.source)
    }
}

impl MolecularSurfaceGeometry {
    /// Borrows the original calculation sample, retaining its resident frame owners.
    #[must_use]
    pub const fn pose(&self) -> Option<&crate::MolecularPose> {
        self.pose.as_ref()
    }

    /// Compares immutable geometry by its internally computed SHA-256 digest.
    /// A fixed-size comparison lets independently computed assets be installed
    /// without scanning their vertex columns on an application's UI thread.
    #[must_use]
    pub fn same_content(&self, other: &Self) -> bool {
        self.content_hash == other.content_hash
    }

    /// Validates a complete surface while retaining the provider's shared owner.
    ///
    /// # Errors
    /// Rejects empty or oversized geometry, mismatched columns, nonfinite points,
    /// invalid normals, degenerate faces and indices outside the vertex column.
    pub fn new(source: Arc<dyn MolecularSurfaceSource>) -> Result<Self, CoreError> {
        Self::with_cancellation(source, &molframe::CancellationToken::default())
    }

    /// Validates a shared surface with bounded cancellation checkpoints.
    /// # Errors
    /// Refuses malformed geometry or cancelled work without publishing an asset.
    pub fn with_cancellation(
        source: Arc<dyn MolecularSurfaceSource>,
        cancellation: &molframe::CancellationToken,
    ) -> Result<Self, CoreError> {
        check(cancellation)?;
        let mesh = source.surface_mesh();
        let geometry = &mesh.geometry;
        let count = geometry.vertices.len();
        if count == 0
            || count > MAX_MESH_VERTICES
            || geometry.faces.is_empty()
            || geometry.faces.len() > 8_000_000
            || geometry.vertex_normals.len() != count
            || mesh.atom_rows.len() != count
        {
            return Err(invalid(
                "molecular surface columns or geometry limits disagree",
            ));
        }
        for (index, (position, normal)) in geometry
            .vertices
            .iter()
            .zip(&geometry.vertex_normals)
            .enumerate()
        {
            if index.is_multiple_of(1024) {
                check(cancellation)?;
            }
            let length = normal.iter().map(|value| value * value).sum::<f32>();
            if !position.iter().all(|value| value.is_finite())
                || !normal.iter().all(|value| value.is_finite())
                || (length - 1.0).abs() > 0.002
            {
                return Err(invalid(
                    "molecular surface requires finite points and unit normals",
                ));
            }
        }
        for (index, face) in geometry.faces.iter().enumerate() {
            if index.is_multiple_of(1024) {
                check(cancellation)?;
            }
            if face.0.iter().any(|&index| index as usize >= count) {
                return Err(invalid("molecular surface face addresses no vertex"));
            }
            let [a, b, c] = face.0.map(|index| geometry.vertices[index as usize]);
            let ab = std::array::from_fn::<_, 3, _>(|axis| f64::from(b[axis]) - f64::from(a[axis]));
            let ac = std::array::from_fn::<_, 3, _>(|axis| f64::from(c[axis]) - f64::from(a[axis]));
            let cross = [
                ab[1] * ac[2] - ab[2] * ac[1],
                ab[2] * ac[0] - ab[0] * ac[2],
                ab[0] * ac[1] - ab[1] * ac[0],
            ];
            if cross.iter().all(|&value| value == 0.0) {
                return Err(invalid("molecular surface contains a degenerate face"));
            }
        }
        let Some(maximum_atom_row) = mesh.atom_rows.iter().copied().max() else {
            return Err(invalid("molecular surface requires atom identities"));
        };
        let content_hash = geometry_digest(mesh, cancellation)?;
        Ok(Self {
            source,
            maximum_atom_row,
            content_hash,
            identity: NEXT_GEOMETRY_ID.fetch_add(1, Ordering::Relaxed),
            molecular_key: None,
            pose: None,
        })
    }

    /// Pins this immutable geometry to one molecular source and coordinate revision.
    #[must_use]
    pub fn for_source(mut self, source: &crate::MolecularSource) -> Self {
        self.molecular_key = Some([source.identity(), source.coordinate_revision()]);
        self.identity = NEXT_GEOMETRY_ID.fetch_add(1, Ordering::Relaxed);
        self
    }

    /// Pins scientific geometry to the exact borrowed coordinate sample used by its provider.
    /// The caller must calculate this output from the captured pose's coordinate columns.
    #[must_use]
    pub fn for_pose(mut self, pose: &crate::MolecularPose) -> Self {
        self.molecular_key = Some([
            pose.source().identity(),
            pose.source().coordinate_revision(),
        ]);
        self.pose = Some(pose.clone());
        self.identity = NEXT_GEOMETRY_ID.fetch_add(1, Ordering::Relaxed);
        self
    }

    /// Confirms a scientific snapshot belongs to the exact displayed pose.
    ///
    /// # Errors
    /// Refuses stale samples and static geometry used with moving molecular coordinates.
    pub fn validate_pose(&self, placed: &crate::PlacedStructure) -> Result<(), CoreError> {
        self.validate_source(&placed.source)?;
        let current = match &self.pose {
            Some(pose) => pose.matches(placed),
            None => placed.trajectory().is_none(),
        };
        if !current {
            return Err(invalid(
                "analytical molecular surface belongs to a different coordinate sample",
            ));
        }
        Ok(())
    }

    /// Tests whether two results were computed at the same retained coordinate sample.
    #[must_use]
    pub fn same_pose(&self, other: &Self) -> bool {
        self.molecular_key == other.molecular_key && self.pose == other.pose
    }

    /// Confirms a source-bound snapshot still belongs to the current molecule.
    ///
    /// # Errors
    /// Refuses a different molecule or updated coordinate revision.
    pub fn validate_source(&self, source: &crate::MolecularSource) -> Result<(), CoreError> {
        self.validate_atom_count(source.coordinates().len())?;
        if self
            .molecular_key
            .is_some_and(|key| key != [source.identity(), source.coordinate_revision()])
        {
            return Err(invalid(
                "analytical molecular surface belongs to a different molecule or coordinate revision",
            ));
        }
        Ok(())
    }

    /// Process-local GPU cache identity, retained by clones and never serialized.
    #[must_use]
    pub const fn cache_identity(&self) -> u64 {
        self.identity
    }

    /// Borrows validated provider geometry and its original atom-row column.
    #[must_use]
    pub fn mesh(&self) -> &molframe::surface::MolecularSurfaceMesh {
        self.source.surface_mesh()
    }

    /// Confirms every vertex's molecular owner exists in the target structure.
    ///
    /// # Errors
    /// Rejects atom identities beyond the target's coordinate column.
    pub fn validate_atom_count(&self, atoms: usize) -> Result<(), CoreError> {
        if self.maximum_atom_row as usize >= atoms {
            return Err(invalid(
                "molecular surface atom identity exceeds the target structure",
            ));
        }
        Ok(())
    }
}

fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidMesh { reason }
}

#[cfg(test)]
#[path = "molecular_surface_tests.rs"]
mod tests;

fn geometry_digest(
    mesh: &molframe::surface::MolecularSurfaceMesh,
    cancellation: &molframe::CancellationToken,
) -> Result<[u8; 32], CoreError> {
    let mut digest = Sha256::new();
    digest.update((mesh.geometry.vertices.len() as u64).to_le_bytes());
    digest.update((mesh.geometry.faces.len() as u64).to_le_bytes());
    for (index, ((position, normal), row)) in mesh
        .geometry
        .vertices
        .iter()
        .zip(&mesh.geometry.vertex_normals)
        .zip(&mesh.atom_rows)
        .enumerate()
    {
        if index.is_multiple_of(1024) {
            check(cancellation)?;
        }
        for value in position.iter().chain(normal) {
            digest.update(value.to_bits().to_le_bytes());
        }
        digest.update(row.to_le_bytes());
    }
    for (index, face) in mesh.geometry.faces.iter().enumerate() {
        if index.is_multiple_of(1024) {
            check(cancellation)?;
        }
        for index in face.0 {
            digest.update(index.to_le_bytes());
        }
    }
    Ok(digest.finalize().into())
}

fn check(cancellation: &molframe::CancellationToken) -> Result<(), CoreError> {
    if cancellation.is_cancelled() {
        return Err(invalid("molecular surface validation was cancelled"));
    }
    Ok(())
}
