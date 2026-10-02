//! Declarative authoring values, mutable scene state and the physical renderer.

#![forbid(unsafe_code)]

mod appearance;
pub mod camera;

/// Camera controllers and the abstract input events that drive them.
pub mod controls {
    pub use molgfx_core::{
        ArcballController, Button, FlyController, InputEvent, Key, OrbitController,
    };
}
pub mod color;
mod error;
mod id;
pub mod interop;
pub mod overlay;
mod patch;
pub mod preset;
pub mod profile;
pub mod property;
mod render;
mod representation;
mod scene;
mod selection;
pub mod source;
mod spec;
pub mod streaming;
pub mod visual;

pub(crate) use molgfx_core::fallback;

pub use appearance::{AppearanceRuleSpec, MAX_APPEARANCE_CLASSES};
pub use color::{Color, ColorSpec, Legend, LegendStop};
pub use error::{Error, PatchError};
pub use id::{
    AnnotationId, AppearanceRuleId, EllipsoidId, InteractionId, MeasurementId, PlaneId,
    RepresentationId, StructureId, TrajectoryId, VolumeId,
};
pub use interop::{Diagnostic, MvsDocument, MvsImport, from_mvsj, from_mvsx, to_mvsj, to_mvsx};
#[cfg(not(target_arch = "wasm32"))]
pub use molgfx_wgpu::{AdapterReport, SystemInfo, system_info};
pub use overlay::{
    Anchor, AnnotationSpec, AssemblySpec, DataSource, EllipsoidSpec, FitResult, InteractionKind,
    InteractionSpec, MeasurementSpec, MovieExportRequest, OverlayHandles, PlaneSpec,
    TrajectoryBinding, TrajectoryFrame, TrajectorySpec, UnitCellSpec, ValidationFinding,
    VolumeBinding, VolumeSpec,
};
pub use patch::{PatchOperation, ScenePatch};
pub use profile::{DepthCue, Quality, RenderProfile};
pub use property::{PropertySpec, ScalarProperty, ScalarPropertyBinding};
#[cfg(not(target_arch = "wasm32"))]
pub use render::Image;
pub use render::PickReadback;
pub use render::Renderer;
pub use render::{
    CompletedFrame, CpuStages, EffectiveQuality, FrameReport, FrameTiming, GpuTiming, PassTiming,
    PassTimingCoverage, QualityTier, SurfaceLimit,
};
pub use render::{PickKind, PickResult};
pub use representation::RepresentationSpec;
pub use representation::{SceneItem, Selection};
pub use scene::domains::{
    AssemblyChoice, MovieExportRequest as DomainMovieExportRequest,
    SceneSnapshot as DomainSceneSnapshot,
};
pub use scene::hashing::structure_hash;
pub use scene::transaction::SceneTransaction;
pub use scene::{
    AssemblyCopy, ResidueMetadata, ResolvedAtomPick, ResolvedBondPick, ResolvedLabelPick,
    ResolvedMeasurementPick, ResolvedPick, ResolvedVolumeSegmentPick, Scene, chain_selection,
};
pub use spec::{InteractionChannel, SceneSpec, StructureSource};
pub use visual::{
    BoolExpr, ColorExpr, Parameter, ParameterType, ParameterValue, ScalarExpr, VectorExpr,
    VisualStyle,
};

pub mod rep {
    //! Typed representation constructors.
    pub use crate::representation::{
        Backbone, BallAndStick, BasePairs, Bases, Beads, Cartoon, CartoonStyle, Dots, Glycan,
        Licorice, Lines, NucleicAcid, PointRepresentation, Putty, Spacefill, Surface, SurfaceKind,
        SurfaceStyle, Trace, Tube, backbone, ball_and_stick, base_pairs, bases, beads, cartoon,
        dots, glycan, licorice, lines, nucleic_acid, points, putty, spacefill, surface, trace,
        tube,
    };
}

pub use overlay::{annotation, density, ellipsoid, interaction, measurement, trajectory};

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
