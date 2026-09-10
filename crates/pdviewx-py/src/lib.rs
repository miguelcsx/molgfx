//! Thin Python bindings over the public pdviewx facade.
//!
//! This crate contains only `PyO3` type adapters, argument marshalling and
//! registration. Scene queries, validation, rendering and serialization stay
//! in the Rust crates so every host language follows one implementation.

#![forbid(unsafe_code)]

mod annotations;
mod authoring;
mod controls;
mod core;
mod error;
mod generated_registration;
mod math;
mod memory;
mod mesh_instances;
mod overlay_authoring;
mod pending_surface;
mod python_module;
mod render;
mod semantic;
mod timeline;
mod topology;
mod trajectory;
mod validation_markers;
mod values;
mod visual;
