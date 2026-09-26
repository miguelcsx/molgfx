//! Declarative authoring values, mutable scene state and the physical renderer.

#![forbid(unsafe_code)]

mod appearance;
pub mod camera;
pub mod color;
mod error;
mod id;
pub mod interop;
mod patch;
pub mod profile;
pub mod property;
mod render;
mod representation;
mod scene;
pub mod science;
mod selection;
pub mod source;
mod spec;
pub mod streaming;
pub mod visual;

#[inline]
fn fallback<T>(candidate: impl IntoIterator<Item = T>, fallback: T) -> T {
    candidate.into_iter().fold(fallback, |_, value| value)
}

pub use appearance::{AppearanceRuleSpec, MAX_APPEARANCE_CLASSES};
pub use color::{Color, ColorSpec, Legend, LegendStop};
pub use error::{Error, PatchError};
pub use id::{
    AnnotationId, AppearanceRuleId, MeasurementId, RepresentationId, ScientificInteractionId,
    StructureId, TrajectoryId, VolumeId,
};
pub use interop::{Diagnostic, MvsDocument, MvsImport, from_mvsj, from_mvsx, to_mvsj, to_mvsx};
#[cfg(not(target_arch = "wasm32"))]
pub use molgfx_wgpu::{AdapterReport, SystemInfo, system_info};
pub use patch::{PatchOperation, ScenePatch};
pub use profile::{Quality, RenderProfile};
pub use property::{PropertySpec, ScalarProperty, ScalarPropertyBinding};
#[cfg(not(target_arch = "wasm32"))]
pub use render::Image;
pub use render::Renderer;
pub use render::{PickKind, PickResult};
pub use representation::RepresentationSpec;
pub use representation::{SceneItem, Selection};
pub use scene::Scene;
pub use scene::hashing::structure_hash;
pub use scene::transaction::SceneTransaction;
pub use science::{
    Anchor, AnnotationSpec, DataSource, InteractionKind, MeasurementSpec, ScientificHandles,
    ScientificInteractionSpec, TrajectoryBinding, TrajectoryFrame, TrajectorySpec, VolumeBinding,
    VolumeSpec,
};
pub use spec::{InteractionChannel, SceneSpec, StructureSource};
pub use visual::{
    BoolExpr, ColorExpr, Parameter, ParameterType, ParameterValue, ScalarExpr, VectorExpr,
    VisualStyle,
};

pub mod rep {
    //! Typed representation constructors.
    pub use crate::representation::{
        BallAndStick, BasePairs, Bases, Cartoon, CartoonStyle, Glycan, Licorice, Lines,
        NucleicAcid, PointRepresentation, Spacefill, Surface, SurfaceKind, SurfaceStyle,
        ball_and_stick, base_pairs, bases, cartoon, glycan, licorice, lines, nucleic_acid, points,
        spacefill, surface,
    };
}

pub use science::{annotation, density, interaction, measurement, trajectory};

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
