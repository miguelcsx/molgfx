//! Geometry and linear algebra: cameras, projections, bounds, splines, frames.
//!
//! Every type here is project-owned. `glam` backs them but is not part of the
//! public surface, so a breaking change upstream is absorbed here rather than
//! pushed onto callers.

pub use molgfx_math::*;
