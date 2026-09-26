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
#[cfg(not(target_arch = "wasm32"))]
pub use molgfx_api::{AdapterReport, SystemInfo, system_info};
pub use molgfx_api::{
    Anchor, AnnotationId, AnnotationSpec, AppearanceRuleId, AppearanceRuleSpec, Camera, Color,
    ColorSpec, Error, InteractionChannel, InteractionKind, Legend, LegendStop,
    MAX_APPEARANCE_CLASSES, MeasurementId, MeasurementSpec, Parameter, ParameterType,
    ParameterValue, PatchError, PickKind, PickResult, Quality, RenderProfile, RepresentationId,
    RepresentationSpec, ScalarProperty, ScalarPropertyBinding, Scene, SceneItem, ScenePatch,
    SceneSpec, SceneTransaction, ScientificInteractionId, ScientificInteractionSpec, Selection,
    StructureId, TrajectoryId, TrajectorySpec, VisualStyle, VolumeId, VolumeSpec, molframe,
};

pub use molgfx_api::{ScientificHandles, TrajectoryBinding, TrajectoryFrame, VolumeBinding};
pub use molgfx_api::{annotation, density, interaction, measurement, trajectory};

/// Binding caller-owned molecular storage, for language bindings and embedders.
pub mod source {
    pub use molgfx_api::molframe::Structure;
    pub use molgfx_api::source::{
        AtomSelection, CoreError, MolecularProvider, MolecularSource, SourceAtom, SourceBond,
        SourceTopology, topology_identity,
    };

    /// Re-parses a transported molecular payload into the structural model.
    ///
    /// A scene's content identity is recomputed by whoever receives the
    /// molecule, so a publisher must digest exactly the bytes it ships. A
    /// binding whose provider cannot see the payload's structure — because the
    /// payload crosses an extension boundary that carries no Rust values — uses
    /// this to obtain the view its consumer will take.
    ///
    /// Returns `None` when the payload is not a supported structure.
    #[must_use]
    pub fn structure_from_payload(bytes: &[u8], name: &str) -> Option<Structure> {
        let options = molgfx_api::molframe::ReadOptions::new();
        molgfx_api::molframe::read_bytes(bytes.to_vec(), Some(name), &options)
            .ok()
            .map(|(structure, _)| structure)
    }
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

/// The authoring command language: typed commands, their text form, and the
/// session that plans them into atomic scene patches.
pub mod command {
    pub use molgfx_command::registry;
    pub use molgfx_command::{
        ColorValue, Command, CommandError, CommandErrors, Completion, ErrorKind, Finite, Form,
        FormKind, InvalidName, LayerSpec, Name, Opacity, OptionError, OptionInfo, OptionKind,
        Outcome, Positive, Program, QueryText, RuleSpec, Session, SessionSpec, Show, Span,
        Statement, Target,
    };
}

/// Ordinary imports for authoring and rendering a scene.
pub mod prelude {
    pub use crate::Renderer;
    pub use crate::{Scene, ScenePatch, color, profile, rep, sel, visual};
}
