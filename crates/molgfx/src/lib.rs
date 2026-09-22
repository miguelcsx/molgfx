//! `MolGFX`'s curated declarative molecular-rendering API.
//!
//! A scene retains immutable `MolFrame` storage, stores portable semantic
//! specifications, and resolves physical GPU resources only inside the
//! renderer. Implementation crates remain separately usable by expert Rust
//! callers; they are intentionally not exposed through this facade.

#![forbid(unsafe_code)]

#[cfg(not(target_arch = "wasm32"))]
pub use molgfx_api::Image;
pub use molgfx_api::Renderer;
pub use molgfx_api::{
    Anchor, AnnotationId, AnnotationSpec, Camera, Color, ColorSpec, Error, InteractionChannel,
    InteractionKind, Legend, LegendStop, MeasurementId, MeasurementSpec, Parameter, ParameterType,
    ParameterValue, PatchError, PickKind, PickResult, Quality, RenderProfile, RepresentationId,
    RepresentationSpec, ScalarProperty, ScalarPropertyBinding, Scene, SceneItem, ScenePatch,
    SceneSpec, SceneTransaction, ScientificInteractionId, ScientificInteractionSpec, Selection,
    StructureId, TrajectoryId, TrajectorySpec, VisualStyle, VolumeId, VolumeSpec, molframe,
};

pub use molgfx_api::{ScientificHandles, VolumeBinding};
pub use molgfx_api::{annotation, density, interaction, measurement, trajectory};

/// Binding caller-owned molecular storage, for language bindings and embedders.
pub mod source {
    pub use molgfx_api::source::{
        AtomSelection, CoreError, MolecularProvider, MolecularSource, SourceAtom, SourceBond,
        SourceTopology, topology_identity,
    };
}

/// Low-level wire-schema values.
pub mod schema {
    pub use molgfx_api::{DataSource, PatchOperation, StructureSource};
}

/// Validated camera construction from facade-native values.
pub mod camera {
    pub use molgfx_api::camera::perspective;
}

/// Immutable representation specifications and constructors.
pub mod rep {
    pub use molgfx_api::rep::{
        BallAndStick, BasePairs, Bases, Cartoon, CartoonStyle, Glycan, Licorice, Lines,
        NucleicAcid, PointRepresentation, Spacefill, Surface, SurfaceKind, SurfaceStyle,
        ball_and_stick, base_pairs, bases, cartoon, glycan, licorice, lines, nucleic_acid, points,
        spacefill, surface,
    };
}

/// `MolFrame`'s exact molecular selection-expression surface.
pub mod sel {
    pub use molgfx_api::sel::{
        Builder, ColumnBuilder, all, aromatic, backbone, by_residue, chain, col, glycans, heavy,
        hetero, hydrogen, ions, ligands, lipids, name, none, nucleic, nucleic_backbone,
        nucleic_base, nucleic_sugar, occupancy, polymer, protein, residues_within, resname,
        sidechain, water, within,
    };
}

/// Scientific color values and mappings.
pub mod color {
    pub use molgfx_api::color::{
        Color, ColorSpec, Legend, LegendStop, chain, element, property, residue,
        secondary_structure, uniform,
    };
}

/// Adaptive rendering profiles.
pub mod profile {
    pub use molgfx_api::profile::{Quality, RenderProfile, adaptive, interactive, publication};
}

/// Typed immutable visual-expression DAGs.
pub mod visual {
    pub use molgfx_api::visual::{
        BoolExpr, ColorExpr, Parameter, ParameterType, ParameterValue, ScalarExpr, VectorExpr,
        VisualStyle,
    };
}

/// Bounded provider-neutral data streaming.
pub mod streaming {
    pub use molgfx_api::streaming::{
        Cancellation, Chunk, DataSource, Limits, Metadata, Priority, Request, Scheduler,
        SourceError,
    };
}

/// Portable scene interchange.
pub mod interop {
    pub use molgfx_api::interop::{
        Diagnostic, MvsDocument, MvsImport, from_mvsj, from_mvsx, to_mvsj, to_mvsx,
    };
}

/// Ordinary imports for authoring and rendering a scene.
pub mod prelude {
    pub use crate::Renderer;
    pub use crate::{Scene, ScenePatch, color, profile, rep, sel, visual};
}
