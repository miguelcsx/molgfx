//! Serializable caller-authored analytic primitive payloads.

use super::ObjectIdentity;
use serde::{Deserialize, Serialize};

/// JSON representation of one caller-supplied visual particle motion sample.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ParticleMotionDescription {
    /// Velocity in model units per second.
    pub velocity: [f32; 3],
    /// Inclusive minimum and maximum model-space bounds.
    pub bounds: [[f32; 3]; 2],
    /// Fixed integration step in seconds.
    pub fixed_timestep: f32,
    /// Stable caller seed.
    pub seed: u32,
    /// `bounce` or `wrap`.
    pub boundary: String,
    /// Fixed-step lifetime before deterministic respawn; zero disables it.
    pub respawn_after_steps: u32,
}

/// One caller-authored analytic primitive.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct PrimitiveDescription {
    /// Stable primitive row and generation.
    pub row: u32,
    /// Primitive handle generation.
    pub generation: u32,
    /// Owning structure identity.
    pub owner: ObjectIdentity,
    /// `ellipsoid`, `carbohydrate`, `planar` or `particle`.
    pub kind: String,
    /// Whether the primitive participates in rendering.
    pub visible: bool,
    /// Display colour including source alpha.
    pub color: [u8; 4],
    /// Additional scalar opacity for ellipsoids and planes.
    pub opacity: f32,
    /// Model-space center.
    pub center: [f32; 3],
    /// Quaternion for oriented symbols; identity for other primitive kinds.
    pub orientation: [f32; 4],
    /// Symbol dimensions or planar dimensions; zero for ellipsoids.
    pub size: [f32; 3],
    /// Symmetric ellipsoid tensor, when the primitive is an ellipsoid.
    pub tensor: Option<[f32; 6]>,
    /// Plane normal, tangent and bitangent, when the primitive is planar.
    pub plane_axes: Option<[[f32; 3]; 3]>,
    /// Stable carbohydrate family or particle shape.
    pub shape: Option<String>,
    /// Shape-specific parameters; superquadrics store their two exponents.
    pub shape_parameters: [f32; 2],
    /// Optional bounded visual-advection state for a particle.
    pub motion: Option<ParticleMotionDescription>,
}
