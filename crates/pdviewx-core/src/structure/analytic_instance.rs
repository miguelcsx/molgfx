//! Shared analytic templates and compact 32-byte rigid instances.

#[cfg(test)]
#[path = "analytic_instance_tests.rs"]
mod tests;

use crate::{CoreError, SourceRows};
use pdviewx_math::{Aabb, Quat, Rgba8, Vec3};
use std::sync::Arc;

const EPSILON: f32 = 1.0e-6;
const PARALLEL_BOUNDS_THRESHOLD: usize = pdviewx_math::parallel::BLOCK * 4;

/// One sphere part in local template space.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AnalyticSphere {
    /// Local center.
    pub center: [f32; 3],
    /// Positive local radius.
    pub radius: f32,
}

/// One capsule part in local template space.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AnalyticCapsule {
    /// Local start and radius.
    pub start_radius: [f32; 4],
    /// Local end and reserved word.
    pub end_reserved: [f32; 4],
}

impl AnalyticCapsule {
    /// Builds a finite non-degenerate capsule.
    ///
    /// # Errors
    ///
    /// Endpoints and radius must be finite; radius and length must be positive.
    pub fn new(start: Vec3, end: Vec3, radius: f32) -> Result<Self, CoreError> {
        if !start.is_finite()
            || !end.is_finite()
            || !radius.is_finite()
            || radius <= 0.0
            || (end - start).length_squared() <= EPSILON * EPSILON
        {
            return Err(invalid(
                "analytic capsule must be finite and non-degenerate",
            ));
        }
        Ok(Self {
            start_radius: [start.x, start.y, start.z, radius],
            end_reserved: [end.x, end.y, end.z, 0.0],
        })
    }

    /// Local start point.
    #[must_use]
    pub fn start(self) -> Vec3 {
        Vec3::new(
            self.start_radius[0],
            self.start_radius[1],
            self.start_radius[2],
        )
    }

    /// Local end point.
    #[must_use]
    pub fn end(self) -> Vec3 {
        Vec3::new(
            self.end_reserved[0],
            self.end_reserved[1],
            self.end_reserved[2],
        )
    }

    /// Positive local radius.
    #[must_use]
    pub const fn radius(self) -> f32 {
        self.start_radius[3]
    }
}

/// Shared local analytic shape partitioned by homogeneous pipeline.
#[derive(Clone, PartialEq, Debug)]
pub struct AnalyticTemplate {
    spheres: Arc<[AnalyticSphere]>,
    capsules: Arc<[AnalyticCapsule]>,
    source_rows: SourceRows,
    bound_radius: f32,
}

impl AnalyticTemplate {
    /// Retains one sphere stream and one capsule stream without per-part tags.
    ///
    /// # Errors
    ///
    /// At least one finite, positive part is required and source rows must
    /// match the sphere-first flattened part count.
    pub fn new(
        spheres: Arc<[AnalyticSphere]>,
        capsules: Arc<[AnalyticCapsule]>,
        source_rows: SourceRows,
    ) -> Result<Self, CoreError> {
        let part_count = spheres.len().saturating_add(capsules.len());
        if part_count == 0 || part_count != source_rows.len() as usize {
            return Err(invalid(
                "analytic template parts must be non-empty and match source rows",
            ));
        }
        if spheres.iter().any(|sphere| {
            !sphere.center.iter().all(|value| value.is_finite())
                || !sphere.radius.is_finite()
                || sphere.radius <= 0.0
        }) || capsules.iter().any(|capsule| {
            !capsule.start().is_finite()
                || !capsule.end().is_finite()
                || !capsule.radius().is_finite()
                || capsule.radius() <= 0.0
        }) {
            return Err(invalid(
                "analytic template parts must be finite and positive",
            ));
        }
        let bound_radius = template_bound_radius(&spheres, &capsules);
        Ok(Self {
            spheres,
            capsules,
            source_rows,
            bound_radius,
        })
    }

    /// Homogeneous sphere stream.
    #[must_use]
    pub const fn spheres(&self) -> &Arc<[AnalyticSphere]> {
        &self.spheres
    }

    /// Homogeneous capsule stream.
    #[must_use]
    pub const fn capsules(&self) -> &Arc<[AnalyticCapsule]> {
        &self.capsules
    }

    /// Flattened sphere-first source identity.
    #[must_use]
    pub const fn source_rows(&self) -> &SourceRows {
        &self.source_rows
    }

    /// Total analytic parts expanded only for visible instances.
    #[must_use]
    pub fn part_count(&self) -> usize {
        self.spheres.len().saturating_add(self.capsules.len())
    }

    /// Conservative local radius used for instance culling.
    #[must_use]
    pub const fn bound_radius(&self) -> f32 {
        self.bound_radius
    }
}

/// Exact 32-byte rigid transform consumed directly by the GPU.
#[repr(C, align(16))]
#[derive(Clone, Copy, PartialEq, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RigidInstance {
    /// Translation xyz and positive uniform scale.
    pub translation_scale: [f32; 4],
    /// Normalized orientation quaternion xyzw.
    pub orientation: [f32; 4],
}

impl RigidInstance {
    /// Validates and packs a rigid transform.
    ///
    /// # Errors
    ///
    /// Translation, scale and orientation must be finite; scale and quaternion
    /// norm must be positive.
    pub fn new(translation: Vec3, orientation: Quat, scale: f32) -> Result<Self, CoreError> {
        let norm = orientation.length_squared();
        if !translation.is_finite()
            || !orientation.is_finite()
            || !scale.is_finite()
            || scale <= 0.0
            || !norm.is_finite()
            || norm <= EPSILON
        {
            return Err(invalid(
                "rigid instance transform must be finite and invertible",
            ));
        }
        Ok(Self {
            translation_scale: [translation.x, translation.y, translation.z, scale],
            orientation: orientation.normalize().to_array(),
        })
    }

    /// Translation component.
    #[must_use]
    pub fn translation(self) -> Vec3 {
        Vec3::new(
            self.translation_scale[0],
            self.translation_scale[1],
            self.translation_scale[2],
        )
    }

    /// Positive uniform scale.
    #[must_use]
    pub const fn scale(self) -> f32 {
        self.translation_scale[3]
    }
}

/// Batch-wide visual fallback for analytic instances.
///
/// Per-instance and per-part variation belongs in typed attributes; this
/// compact value keeps an unprogrammed batch renderable without row storage.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct InstanceStyle {
    /// Packed linear-display fallback color.
    pub color: Rgba8,
}

impl Default for InstanceStyle {
    fn default() -> Self {
        Self {
            color: Rgba8::WHITE,
        }
    }
}

/// Immutable transforms sharing one analytic template.
#[derive(Clone, PartialEq, Debug)]
pub struct InstanceBatch {
    template: Arc<AnalyticTemplate>,
    transforms: Arc<[RigidInstance]>,
    source_rows: SourceRows,
    style: InstanceStyle,
    bounds: Aabb,
    visible: bool,
}

impl InstanceBatch {
    /// Retains one 32-byte transform per instance without expanding parts.
    ///
    /// # Errors
    ///
    /// The batch must be non-empty, source-aligned and finite after applying
    /// the template bound.
    pub fn new(
        template: Arc<AnalyticTemplate>,
        transforms: Arc<[RigidInstance]>,
        source_rows: SourceRows,
    ) -> Result<Self, CoreError> {
        if transforms.is_empty() || transforms.len() != source_rows.len() as usize {
            return Err(invalid(
                "instance transforms must be non-empty and match source rows",
            ));
        }
        let occurrences = transforms
            .len()
            .checked_mul(template.part_count())
            .ok_or_else(|| invalid("instance-part occurrences exceed picking capacity"))?;
        if occurrences > crate::EntityId::MAX_INDEX as usize + 1 {
            return Err(invalid("instance-part occurrences exceed picking capacity"));
        }
        let bounds = instance_bounds(&transforms, template.bound_radius());
        if bounds.is_empty() {
            return Err(invalid("instance bounds must be finite"));
        }
        Ok(Self {
            template,
            transforms,
            source_rows,
            style: InstanceStyle::default(),
            bounds,
            visible: true,
        })
    }

    /// Shared analytic topology.
    #[must_use]
    pub const fn template(&self) -> &Arc<AnalyticTemplate> {
        &self.template
    }

    /// Shared 32-byte transform rows.
    #[must_use]
    pub const fn transforms(&self) -> &Arc<[RigidInstance]> {
        &self.transforms
    }

    /// Stable instance source identity.
    #[must_use]
    pub const fn source_rows(&self) -> &SourceRows {
        &self.source_rows
    }

    /// Replaces the allocation-free batch fallback style.
    #[must_use]
    pub const fn with_style(mut self, style: InstanceStyle) -> Self {
        self.style = style;
        self
    }

    /// Batch-wide fallback used when no visual program targets the batch.
    #[must_use]
    pub const fn style(&self) -> InstanceStyle {
        self.style
    }

    /// Conservative batch bound.
    #[must_use]
    pub const fn bounds(&self) -> Aabb {
        self.bounds
    }

    /// Current scene visibility.
    #[must_use]
    pub const fn visible(&self) -> bool {
        self.visible
    }

    pub(crate) fn set_visible(&mut self, visible: bool) -> bool {
        if self.visible == visible {
            return false;
        }
        self.visible = visible;
        true
    }
}

fn template_bound_radius(spheres: &[AnalyticSphere], capsules: &[AnalyticCapsule]) -> f32 {
    let sphere_bound = spheres
        .iter()
        .map(|sphere| Vec3::from(sphere.center).length() + sphere.radius)
        .fold(0.0f32, f32::max);
    capsules.iter().fold(sphere_bound, |bound, capsule| {
        bound.max(capsule.start().length().max(capsule.end().length()) + capsule.radius())
    })
}

fn instance_bounds(transforms: &[RigidInstance], template_radius: f32) -> Aabb {
    pdviewx_math::parallel::reduce_blocks(
        transforms,
        PARALLEL_BOUNDS_THRESHOLD,
        Aabb::EMPTY,
        |block| {
            let mut bound = Aabb::EMPTY;
            for transform in block {
                bound.extend_sphere(transform.translation(), template_radius * transform.scale());
            }
            bound
        },
        |left, right| left.union(&right),
    )
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidBatch { reason }
}
