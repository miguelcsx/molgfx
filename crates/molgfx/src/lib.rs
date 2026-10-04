//! `MolGFX`'s curated declarative molecular-rendering API.
//!
//! A scene retains immutable `MolFrame` storage, stores portable semantic
//! specifications, and resolves physical GPU resources only inside the
//! renderer. Implementation crates remain separately usable by expert Rust
//! callers; they are intentionally not exposed through this facade.

#![forbid(unsafe_code)]

pub use molgfx_scene::PickReadback;
pub use molgfx_scene::Renderer;
#[cfg(not(target_arch = "wasm32"))]
pub use molgfx_scene::{AdapterReport, SystemInfo, system_info};
pub use molgfx_scene::{
    Anchor, AnnotationId, AnnotationSpec, AppearanceRuleId, AppearanceRuleSpec, AssemblyCopy,
    AssemblySpec, Camera, Color, ColorSpec, DepthCue, EllipsoidId, EllipsoidSpec, Error,
    InteractionChannel, InteractionId, InteractionKind, InteractionSpec, Legend, LegendStop,
    MAX_APPEARANCE_CLASSES, MeasurementId, MeasurementSpec, Parameter, ParameterType,
    ParameterValue, PatchError, PickKind, PickResult, PlaneId, PlaneSpec, Quality, RenderProfile,
    RepresentationId, RepresentationSpec, ScalarProperty, ScalarPropertyBinding, Scene, SceneItem,
    ScenePatch, SceneSpec, SceneTransaction, Selection, StructureId, TrajectoryId, TrajectorySpec,
    UnitCellSpec, VisualStyle, VolumeId, VolumeSpec, chain_selection, molframe,
};
pub use molgfx_scene::{
    CompletedFrame, CpuStages, EffectiveQuality, FrameReport, PassTiming, PassTimingCoverage,
    QualityTier, SurfaceLimit,
};
#[cfg(not(target_arch = "wasm32"))]
pub use molgfx_scene::{FrameTiming, GpuTiming};
#[cfg(not(target_arch = "wasm32"))]
pub use molgfx_scene::{HdrImage, Image};

pub use molgfx_scene::{OverlayHandles, TrajectoryBinding, TrajectoryFrame, VolumeBinding};
pub use molgfx_scene::{annotation, density, ellipsoid, interaction, measurement, trajectory};

/// Binding caller-owned molecular storage, for language bindings and embedders.
pub mod source {
    pub use molgfx_scene::molframe::Structure;
    pub use molgfx_scene::source::{
        AtomSelection, CoreError, MolecularProvider, MolecularSource, SourceAtom, SourceBond,
        SourceTopology, is_metal_atomic_number, topology_identity,
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
        let options = molgfx_scene::molframe::ReadOptions::new();
        molgfx_scene::molframe::read_bytes(bytes.to_vec(), Some(name), &options)
            .ok()
            .map(|(structure, _)| structure)
    }
}

/// Low-level wire-schema values.
pub mod schema {
    pub use molgfx_scene::source::SecondaryStructure;
    pub use molgfx_scene::{DataSource, PatchOperation, StructureSource};
}

/// Validated camera construction, and the mapping between world points and
/// screen pixels.
pub mod camera {
    pub use molgfx_scene::camera::{
        CameraEasing, CameraKeyframe, CameraPath, Ray, ScreenPoint, path, perspective, project, ray,
    };
}

/// Camera controllers: pure functions from abstract input events to camera
/// edits. The host translates its toolkit's events; no window is named here.
pub mod controls {
    pub use molgfx_scene::controls::{
        ArcballController, Button, FlyController, InputEvent, Key, OrbitController,
    };
}

/// Immutable representation specifications and constructors.
pub mod rep {
    pub use molgfx_scene::rep::{
        Backbone, BallAndStick, BasePairs, Bases, Beads, Cartoon, CartoonStyle, Dots, Glycan,
        Licorice, Lines, NucleicAcid, PointRepresentation, Putty, Spacefill, Surface, SurfaceKind,
        SurfaceStyle, Trace, Tube, backbone, ball_and_stick, base_pairs, bases, beads, cartoon,
        dots, glycan, licorice, lines, nucleic_acid, points, putty, spacefill, surface, trace,
        tube,
    };
}

/// `MolFrame`'s exact molecular selection-expression surface.
pub mod sel {
    pub use molgfx_scene::sel::{
        Builder, ColumnBuilder, all, aromatic, backbone, by_residue, chain, col, glycans, heavy,
        hetero, hydrogen, ions, ligands, lipids, name, none, nucleic, nucleic_backbone,
        nucleic_base, nucleic_sugar, occupancy, polymer, protein, residues_within, resname,
        sidechain, water, within,
    };
}

/// Color values and mappings.
pub mod color {
    pub use molgfx_scene::color::{
        AtomCategory, AtomMetric, Color, ColorSpec, Legend, LegendStop, carbon_by_chain, chain,
        element, entity, metric, molecule_type, palette_names, property, ramp_names, residue,
        residue_name, secondary_structure, uniform,
    };
}

/// Size- and focus-aware default representations.
pub mod preset {
    pub use molgfx_scene::preset::{
        DifferenceStyle, EnsembleMember, EnsembleStyle, PocketStyle, StructureSize,
        auto_representations, difference_visual, ensemble_representations, pocket_representations,
    };
}

/// Adaptive and fixed rendering profiles.
pub mod profile {
    pub use molgfx_scene::MeasuredOutput;
    pub use molgfx_scene::profile::{
        DepthCue, Quality, RenderProfile, adaptive, converged, highest_fixed, interactive,
    };
}

/// Typed immutable visual-expression DAGs.
pub mod visual {
    pub use molgfx_scene::visual::{
        BoolExpr, ColorExpr, Parameter, ParameterType, ParameterValue, ScalarExpr, VectorExpr,
        VisualStyle,
    };
}

/// Bounded provider-neutral data streaming.
pub mod streaming {
    pub use molgfx_scene::streaming::{
        Cancellation, Chunk, DataSource, Limits, Metadata, Priority, Request, Scheduler,
        SourceError,
    };
}

/// Portable scene interchange.
pub mod interop {
    pub use molgfx_scene::interop::{
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
