//! Caller-owned particle glyphs for generic overlays.
//!
//! A particle is presentation data, not a molecular fact. The caller supplies
//! positions, shape, scale and colour; an optional fixed-step motion sample
//! advances the record in the shared analytic table without inferring a field.

use crate::{CoreError, StructureHandle};
use pdviewx_math::{Aabb, Quat, Rgba8, Vec3};

#[cfg(test)]
#[path = "particle_tests.rs"]
mod tests;

const EPSILON: f32 = 1.0e-6;

/// Deterministic boundary response for caller-supplied visual advection.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ParticleBoundary {
    /// Reflect velocity at each finite-field boundary.
    #[default]
    Bounce,
    /// Re-enter at the opposite side of the finite field.
    Wrap,
}

/// Caller-supplied velocity sample and bounded visual-advection state.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ParticleMotion {
    velocity: Vec3,
    bounds: Aabb,
    fixed_timestep: f32,
    seed: u32,
    boundary: ParticleBoundary,
    respawn_after_steps: u32,
}

impl ParticleMotion {
    /// Creates one bounded, fixed-step motion sample.
    ///
    /// The velocity is a caller-provided visual field sample. No integrator or
    /// physical solver is inferred from the source structure.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidPrimitive`] for a malformed
    /// velocity, empty bounds or a timestep outside `(0, 1]` seconds.
    pub fn new(
        velocity: Vec3,
        bounds: Aabb,
        fixed_timestep: f32,
        seed: u32,
        boundary: ParticleBoundary,
    ) -> Result<Self, CoreError> {
        if !velocity.is_finite()
            || bounds.is_empty()
            || !bounds.min.is_finite()
            || !bounds.max.is_finite()
            || bounds.min.x >= bounds.max.x
            || bounds.min.y >= bounds.max.y
            || bounds.min.z >= bounds.max.z
            || !fixed_timestep.is_finite()
            || !(0.0..=1.0).contains(&fixed_timestep)
            || fixed_timestep <= EPSILON
        {
            return Err(CoreError::InvalidPrimitive {
                reason: "particle motion must have finite non-empty bounds and a timestep in (0, 1]",
            });
        }
        Ok(Self {
            velocity,
            bounds,
            fixed_timestep,
            seed,
            boundary,
            respawn_after_steps: 0,
        })
    }

    /// Caller-supplied velocity sample in model units per second.
    #[must_use]
    pub const fn velocity(self) -> Vec3 {
        self.velocity
    }

    /// Finite model-space region in which the particle moves.
    #[must_use]
    pub const fn bounds(self) -> Aabb {
        self.bounds
    }

    /// Fixed integration step in seconds.
    #[must_use]
    pub const fn fixed_timestep(self) -> f32 {
        self.fixed_timestep
    }

    /// Stable caller seed retained for deterministic respawn/cycle policies.
    #[must_use]
    pub const fn seed(self) -> u32 {
        self.seed
    }

    /// Boundary response.
    #[must_use]
    pub const fn boundary(self) -> ParticleBoundary {
        self.boundary
    }

    /// Number of fixed steps before deterministic seeded respawn; zero disables it.
    #[must_use]
    pub const fn respawn_after_steps(self) -> u32 {
        self.respawn_after_steps
    }

    /// Enables deterministic visual respawn after a fixed number of steps.
    #[must_use]
    pub const fn with_respawn_after_steps(mut self, steps: u32) -> Self {
        self.respawn_after_steps = steps;
        self
    }
}

/// Analytic particle shape supported by the portable overlay path.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ParticleShape {
    /// A sphere impostor.
    #[default]
    Sphere = 0,
    /// An oriented box impostor.
    Box = 1,
    /// A finite cylinder with planar end caps; x and y are its diameter and z
    /// is its full axial extent.
    Cylinder = 2,
    /// A capsule with hemispherical ends; x and y are its diameter and z is
    /// its full axial extent.
    Spherocylinder = 3,
    /// An anisotropic Gaussian splat with finite six-sigma support extents.
    Gaussian = 4,
    /// Camera-facing circular billboard in the particle's local xy plane.
    Circle = 5,
    /// Camera-facing square billboard in the particle's local xy plane.
    Square = 6,
    /// Oriented superquadric controlled by two positive exponents.
    Superquadric = 7,
}

/// One caller-authored particle record.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Particle {
    /// Structure carrying the model-space position.
    pub owner: StructureHandle,
    /// Center in the owner's model space.
    pub center: Vec3,
    /// Orientation for box particles; normalized for every record.
    pub orientation: Quat,
    /// Diameter for spheres, full extents for boxes and cylinders, or finite
    /// six-sigma support extents for Gaussian splats.
    pub size: Vec3,
    /// Analytic shape.
    pub shape: ParticleShape,
    /// Shape-specific parameters. Superquadrics use the two exponents; other
    /// shapes retain `[1, 1]`.
    pub shape_parameters: [f32; 2],
    /// Display colour.
    pub color: Rgba8,
    /// Final source alpha.
    pub opacity: f32,
    /// Whether the record participates in rendering.
    pub visible: bool,
    /// Optional caller-supplied fixed-step visual advection.
    pub motion: Option<ParticleMotion>,
}

impl Particle {
    /// Validates one caller-authored particle.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidPrimitive`] for malformed pose,
    /// dimensions or opacity.
    pub fn new(
        owner: StructureHandle,
        center: Vec3,
        orientation: Quat,
        size: Vec3,
        shape: ParticleShape,
        color: Rgba8,
        opacity: f32,
    ) -> Result<Self, CoreError> {
        if !center.is_finite()
            || !orientation.is_finite()
            || orientation.length_squared() <= EPSILON
            || !size.is_finite()
            || size.min_element() <= EPSILON
            || !opacity.is_finite()
            || !(0.0..=1.0).contains(&opacity)
            || (matches!(
                shape,
                ParticleShape::Sphere | ParticleShape::Cylinder | ParticleShape::Spherocylinder
            ) && (size.x - size.y).abs() > EPSILON)
            || (shape == ParticleShape::Sphere && (size.x - size.z).abs() > EPSILON)
            || (shape == ParticleShape::Spherocylinder && size.z + EPSILON < size.x)
        {
            return Err(CoreError::InvalidPrimitive {
                reason: "particle pose, size and opacity must be finite and valid",
            });
        }
        Ok(Self {
            owner,
            center,
            orientation: orientation.normalize(),
            size,
            shape,
            color,
            opacity,
            shape_parameters: [1.0, 1.0],
            visible: true,
            motion: None,
        })
    }

    /// Sets the north-south and east-west superquadric exponents.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidPrimitive`] unless this is a
    /// superquadric and both exponents are finite in `[0.1, 8]`.
    pub fn with_superquadric_exponents(
        mut self,
        latitude: f32,
        longitude: f32,
    ) -> Result<Self, CoreError> {
        if self.shape != ParticleShape::Superquadric
            || !latitude.is_finite()
            || !longitude.is_finite()
            || !(0.1..=8.0).contains(&latitude)
            || !(0.1..=8.0).contains(&longitude)
        {
            return Err(CoreError::InvalidPrimitive {
                reason: "superquadric exponents must be finite in [0.1, 8]",
            });
        }
        self.shape_parameters = [latitude, longitude];
        Ok(self)
    }

    /// Enables bounded GPU advection without changing the source position
    /// stored by the caller.
    #[must_use]
    pub const fn with_motion(mut self, motion: ParticleMotion) -> Self {
        self.motion = Some(motion);
        self
    }

    /// Removes visual advection and leaves the current source pose intact.
    #[must_use]
    pub const fn without_motion(mut self) -> Self {
        self.motion = None;
        self
    }

    /// Conservative model-space bound used by culling and shadow fitting.
    #[must_use]
    pub fn bounds(self) -> Aabb {
        let half = self.size * 0.5;
        Aabb::from_points(
            [
                Vec3::new(-half.x, -half.y, -half.z),
                Vec3::new(half.x, -half.y, -half.z),
                Vec3::new(-half.x, half.y, -half.z),
                Vec3::new(half.x, half.y, -half.z),
                Vec3::new(-half.x, -half.y, half.z),
                Vec3::new(half.x, -half.y, half.z),
                Vec3::new(-half.x, half.y, half.z),
                Vec3::new(half.x, half.y, half.z),
            ]
            .map(|corner| self.center + self.orientation * corner),
        )
    }
}
