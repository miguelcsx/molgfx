//! The semantic scene graph: columnar tables, GPU record layouts, the
//! borrowed coordinate seam.
//!
//! This crate holds data. It knows how bytes must be laid out for the GPU but
//! never issues a device command, so everything here is testable without one.
//! Coordinates are borrowed from the parsed structure and never copied; all
//! other per-atom state lives in owned, revision-counted columns.

#![forbid(unsafe_code)]

#[cfg(test)]
mod fixture;

mod atoms;
mod column;
mod controller;
mod coord;
mod error;
mod gpu_types;
mod handle;
mod hierarchy;
mod input;
mod placed;
mod radii;
mod representation;
mod scene;
mod selection;

pub use atoms::AtomTable;
pub use column::{Column, Revision};
pub use controller::{ArcballController, FlyController, OrbitController};
pub use coord::CoordRef;
pub use error::CoreError;
pub use gpu_types::{AtomFlags, AtomGpu, BondGpu, DrawIndirectArgs, EntityId, EntityKind};
pub use handle::{RepresentationHandle, SelectionHandle, StructureHandle};
pub use hierarchy::Hierarchy;
pub use input::{Button, InputEvent, Key};
pub use placed::PlacedStructure;
pub use radii::{cpk_color, vdw_radius};
pub use representation::{
    ColorScheme, Material, Representation, RepresentationKind, RepresentationParams,
};
pub use scene::Scene;
pub use selection::AtomSelection;
