//! Reusable licorice topology and compact rigid candidate poses.

use crate::CoreError;
use molgfx_math::{Aabb, Quat, Rgba8, Vec3};
use std::sync::Arc;

const EPSILON: f32 = 1.0e-6;
const DEFAULT_RADIUS: f32 = 0.16;

/// Reusable atom/bond geometry for many rigid ligand candidates.
///
/// Positions are converted once into coordinates relative to `origin`, and
/// bond axes are precomputed once. Applying poses therefore performs no bond
/// lookup or normalization per candidate.
#[derive(Clone, Debug)]
pub struct LicoriceTemplate {
    atoms: Arc<[Vec3]>,
    bonds: Arc<[TemplateBond]>,
    bond_indices: Arc<[[u32; 2]]>,
    atom_radius: f32,
    bond_radius: f32,
    bound_radius: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TemplateBond {
    pub(crate) center: Vec3,
    pub(crate) orientation: Quat,
    pub(crate) length: f32,
}

impl LicoriceTemplate {
    /// Builds a topology from atom positions and zero-based bond endpoints.
    ///
    /// # Errors
    ///
    /// Rejects empty/non-finite atoms, invalid endpoints and zero-length bonds.
    pub fn new(origin: Vec3, mut atoms: Vec<Vec3>, bonds: &[[u32; 2]]) -> Result<Self, CoreError> {
        if !origin.is_finite() || atoms.is_empty() || atoms.iter().any(|atom| !atom.is_finite()) {
            return Err(invalid(
                "licorice atoms and origin must be finite and non-empty",
            ));
        }
        for atom in &mut atoms {
            *atom -= origin;
        }
        if atoms.iter().any(|atom| !atom.is_finite()) {
            return Err(invalid("licorice local coordinates overflow"));
        }
        let mut packed_bonds = Vec::with_capacity(bonds.len());
        for &[a, b] in bonds {
            let (Some(start), Some(end)) = (atoms.get(a as usize), atoms.get(b as usize)) else {
                return Err(invalid("licorice bond endpoint is outside the atom table"));
            };
            let axis = *end - *start;
            let length = axis.length();
            if !length.is_finite() || length <= EPSILON {
                return Err(invalid("licorice bonds must have positive finite length"));
            }
            packed_bonds.push(TemplateBond {
                center: (*start + *end) * 0.5,
                orientation: Quat::from_rotation_arc(Vec3::Z, axis / length),
                length,
            });
        }
        let bound_radius = atoms
            .iter()
            .map(|atom| atom.length())
            .fold(0.0_f32, f32::max);
        if !bound_radius.is_finite() {
            return Err(invalid("licorice atom extent must be finite"));
        }
        Ok(Self {
            atoms: atoms.into(),
            bonds: packed_bonds.into(),
            bond_indices: bonds.into(),
            atom_radius: DEFAULT_RADIUS,
            bond_radius: DEFAULT_RADIUS,
            bound_radius,
        })
    }

    /// Sets atom and bond radii in Angstrom.
    ///
    /// # Errors
    ///
    /// Both radii must be finite and positive.
    pub fn radii(mut self, atom_radius: f32, bond_radius: f32) -> Result<Self, CoreError> {
        if !atom_radius.is_finite()
            || !bond_radius.is_finite()
            || atom_radius <= EPSILON
            || bond_radius <= EPSILON
            || !(atom_radius * 2.0).is_finite()
            || !(bond_radius * 2.0).is_finite()
            || self
                .bonds
                .iter()
                .any(|bond| !(bond.length + bond_radius * 2.0).is_finite())
        {
            return Err(invalid("licorice radii must be finite and positive"));
        }
        self.atom_radius = atom_radius;
        self.bond_radius = bond_radius;
        Ok(self)
    }

    /// Number of analytic instances emitted for each pose.
    #[must_use]
    pub fn instances_per_pose(&self) -> usize {
        self.atoms.len().saturating_add(self.bonds.len())
    }

    /// Local atom centres uploaded once as shared GPU topology.
    #[doc(hidden)]
    #[must_use]
    pub fn atom_centers(&self) -> &[Vec3] {
        &self.atoms
    }

    /// Precomputed local bond frames uploaded once as shared GPU topology.
    #[doc(hidden)]
    #[must_use]
    pub fn bond_frames(&self) -> impl ExactSizeIterator<Item = (Vec3, Quat, f32)> + '_ {
        self.bonds
            .iter()
            .map(|bond| (bond.center, bond.orientation, bond.length))
    }

    /// Zero-based template endpoints retained for scene serialization.
    #[doc(hidden)]
    #[must_use]
    pub fn bond_indices(&self) -> &[[u32; 2]] {
        &self.bond_indices
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn atom_radius(&self) -> f32 {
        self.atom_radius
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn bond_radius(&self) -> f32 {
        self.bond_radius
    }

    #[must_use]
    pub(crate) const fn bound_radius(&self) -> f32 {
        self.bound_radius + self.atom_radius.max(self.bond_radius)
    }
}

/// Compact scene-owned occurrence table for one ligand topology.
///
/// The topology is shared through reference-counted immutable slices and each
/// candidate occupies one rigid pose record. Analytic sphere and capsule
/// instances are expanded from these two tables in the GPU vertex stage.
#[derive(Debug)]
pub struct LigandPoseBatch {
    owner: crate::StructureHandle,
    template: LicoriceTemplate,
    poses: Box<[LigandPose]>,
    bounds: Aabb,
    visible: bool,
}

impl LigandPoseBatch {
    pub(crate) fn new(
        owner: crate::StructureHandle,
        template: LicoriceTemplate,
        poses: Vec<LigandPose>,
    ) -> Self {
        let radius = Vec3::splat(template.bound_radius());
        let bounds = poses.iter().fold(Aabb::EMPTY, |bounds, pose| {
            bounds.union(&Aabb::new(
                pose.translation() - radius,
                pose.translation() + radius,
            ))
        });
        Self {
            owner,
            template,
            poses: poses.into_boxed_slice(),
            bounds,
            visible: true,
        }
    }

    /// Structure carrying this batch's model-space transforms.
    #[must_use]
    pub const fn owner(&self) -> crate::StructureHandle {
        self.owner
    }

    /// Number of rigid ligand candidates retained by the batch.
    #[must_use]
    pub const fn pose_count(&self) -> usize {
        self.poses.len()
    }

    /// Number of analytic instances generated when the batch is visible.
    #[must_use]
    pub fn instance_count(&self) -> usize {
        self.template
            .instances_per_pose()
            .saturating_mul(self.poses.len())
    }

    /// Whether the batch participates in rendering.
    #[must_use]
    pub const fn visible(&self) -> bool {
        self.visible
    }

    /// Immutable topology shared by every GPU-expanded pose.
    #[doc(hidden)]
    #[must_use]
    pub const fn template(&self) -> &LicoriceTemplate {
        &self.template
    }

    /// Compact pose column consumed directly by the renderer.
    #[doc(hidden)]
    #[must_use]
    pub const fn poses(&self) -> &[LigandPose] {
        &self.poses
    }

    pub(crate) fn set_visible(&mut self, visible: bool) -> bool {
        if self.visible == visible {
            return false;
        }
        self.visible = visible;
        true
    }

    pub(crate) const fn bounds(&self) -> Aabb {
        self.bounds
    }
}

/// One rigid occurrence of a reusable ligand template.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct LigandPose {
    translation: Vec3,
    orientation: Quat,
    color: Rgba8,
    opacity: f32,
}

impl LigandPose {
    /// Creates a finite rigid pose with one candidate color.
    ///
    /// # Errors
    ///
    /// Rejects malformed transforms or opacity outside `[0, 1]`.
    pub fn new(
        translation: Vec3,
        orientation: Quat,
        color: Rgba8,
        opacity: f32,
    ) -> Result<Self, CoreError> {
        let orientation_norm = orientation.length_squared();
        if !translation.is_finite()
            || !orientation.is_finite()
            || !orientation_norm.is_finite()
            || orientation_norm <= EPSILON
            || !opacity.is_finite()
            || !(0.0..=1.0).contains(&opacity)
        {
            return Err(invalid("ligand pose and opacity must be finite and valid"));
        }
        Ok(Self {
            translation,
            orientation: orientation.normalize(),
            color,
            opacity,
        })
    }

    /// Destination of the template origin.
    #[must_use]
    pub const fn translation(self) -> Vec3 {
        self.translation
    }

    /// Unit rotation applied before translation.
    #[must_use]
    pub const fn orientation(self) -> Quat {
        self.orientation
    }

    /// Uniform candidate colour.
    #[must_use]
    pub const fn color(self) -> Rgba8 {
        self.color
    }

    /// Uniform candidate opacity.
    #[must_use]
    pub const fn opacity(self) -> f32 {
        self.opacity
    }

    pub(crate) fn transforms_finitely(self, radius: f32) -> bool {
        [-1.0, 1.0].iter().copied().all(|x| {
            [-1.0, 1.0].iter().copied().all(|y| {
                [-1.0, 1.0].iter().copied().all(|z| {
                    (self.translation + self.orientation * Vec3::new(x, y, z) * radius).is_finite()
                })
            })
        })
    }
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidPrimitive { reason }
}

#[cfg(test)]
#[path = "pose_batch_tests.rs"]
mod tests;
