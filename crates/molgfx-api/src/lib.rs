//! Declarative authoring values, mutable scene state and the physical renderer.

#![forbid(unsafe_code)]

pub mod color;
mod error;
mod id;
pub mod interop;
mod patch;
pub mod profile;
mod renderer;
mod representation;
mod scene;
mod scene_runtime;
mod scene_transaction;
mod spec;
mod spec_native;
pub mod streaming;
pub mod visual;
mod visual_native;

#[inline]
fn fallback<T>(candidate: impl IntoIterator<Item = T>, fallback: T) -> T {
    candidate.into_iter().fold(fallback, |_, value| value)
}

#[cfg(test)]
#[path = "color_tests.rs"]
mod color_tests;
#[cfg(test)]
#[path = "interop_tests.rs"]
mod interop_tests;
#[cfg(test)]
#[path = "streaming_tests.rs"]
mod streaming_tests;
#[cfg(test)]
#[path = "visual_tests.rs"]
mod visual_tests;

pub use color::{Color, ColorSpec, Legend, LegendStop};
pub use error::{Error, PatchError};
pub use id::{RepresentationId, StructureId};
pub use interop::{Diagnostic, MvsDocument, MvsImport, from_mvsj, from_mvsx, to_mvsj, to_mvsx};
pub use patch::{PatchOperation, ScenePatch};
pub use profile::{Quality, RenderProfile};
#[cfg(not(target_arch = "wasm32"))]
pub use renderer::Image;
pub use renderer::Renderer;
pub use renderer::{PickKind, PickResult};
pub use representation::{SceneItem, Selection};
pub use scene::Scene;
pub use scene_transaction::SceneTransaction;
pub use spec::{InteractionChannel, RepresentationSpec, Revisions, SceneSpec, StructureSource};
pub use visual::{
    BoolExpr, ColorExpr, Parameter, ParameterType, ParameterValue, ScalarExpr, VectorExpr,
    VisualStyle,
};

pub mod rep {
    //! Typed representation constructors.
    pub use crate::representation::{
        AtomRepresentation, Cartoon, CartoonStyle, PointRepresentation, Surface, SurfaceKind,
        SurfaceStyle, ball_and_stick, base_pairs, bases, cartoon, glycan, licorice, lines,
        nucleic_acid, points, spacefill, surface,
    };
}

/// `MolFrame`'s immutable molecular selection expressions.
pub mod sel {
    pub use molframe::query::col::{
        all, aromatic, backbone, by_residue, chain, glycans, heavy, hetero, hydrogen, ions,
        is_protein as protein, ligands, lipids, name, none, nucleic, nucleic_backbone,
        nucleic_base, nucleic_sugar, occupancy, polymer, residues_within, resname, sidechain,
        water, within,
    };
    pub use molframe::query::{Builder, ColumnBuilder, col};
}

pub use molframe;
pub use molgfx_math::Camera;
