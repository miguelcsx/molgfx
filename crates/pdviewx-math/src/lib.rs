//! glam-based math foundation: cameras, projections, bounds, splines,
//! transport frames.
//!
//! Conventions carried by every type here: the world unit is the Ångström,
//! all spaces are right-handed with +X right, +Y up and +Z toward the viewer,
//! the camera looks down −Z, and clip depth uses the reversed [0, 1] range
//! (near maps to 1.0, far to 0.0) so floating-point depth precision spreads
//! evenly across a molecular scene.

#![forbid(unsafe_code)]

mod bounds;
mod camera;
mod curves;
pub mod parallel;
mod quantize;
pub mod simd;
mod types;

pub(crate) use bounds::aabb;
pub(crate) use camera::projection;

pub use bounds::{
    Aabb, BoundingSphere, Bvh, BvhBuildError, BvhBuildScratch, BvhNode, BvhSource, SphereBounds,
    SweptSphereBounds,
};
pub use camera::{Camera, Projection};
pub use curves::{
    CurveSample, TransportFrame, parallel_transport, sample_catmull_rom,
    sample_catmull_rom_demanding, sample_catmull_rom_fixed, sample_cubic_bezier,
};
pub use quantize::{round_u8, truncate_u16, unit_to_grid, unorm8};
pub use types::{Mat3, Mat4, Quat, Rgba8, Vec2, Vec3, Vec4};
