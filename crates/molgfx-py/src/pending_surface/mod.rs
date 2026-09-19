//! Internal typed adapters for the facade surface-completion slice.
//!
//! These modules only translate Python values to the Rust-owned facade
//! contracts; which of them the module exports is declared in
//! `crate::python_module`.

pub(crate) mod descriptions;
pub(crate) mod geometry;
pub(crate) mod mapping;
pub(crate) mod overlay;
pub(crate) mod particle;
pub(crate) mod presentation;
pub(crate) mod provenance;
pub(crate) mod representation;
pub(crate) mod selection;
pub(crate) mod semantic;
pub(crate) mod visual;
pub(crate) mod volume;
