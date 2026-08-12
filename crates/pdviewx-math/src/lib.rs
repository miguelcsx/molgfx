//! glam-based math foundation: cameras, projections, bounds, splines,
//! transport frames.
//!
//! Conventions carried by every type here: the world unit is the Ångström,
//! all spaces are right-handed with +X right, +Y up and +Z toward the viewer,
//! the camera looks down −Z, and clip depth uses the reversed [0, 1] range
//! (near maps to 1.0, far to 0.0) so floating-point depth precision spreads
//! evenly across a molecular scene.

#![forbid(unsafe_code)]

mod aabb;
mod camera;
mod projection;
mod types;

pub use aabb::{Aabb, BoundingSphere};
pub use camera::Camera;
pub use projection::Projection;
pub use types::{Mat3, Mat4, Quat, Rgba8, Vec2, Vec3, Vec4};
