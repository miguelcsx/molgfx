//! Declarative authoring values, mutable scene state and the physical renderer.

#![forbid(unsafe_code)]

mod appearance;
pub mod camera;

/// Camera controllers and the abstract input events that drive them.
pub mod controls {
    pub use molgfx_core::{
        ArcballController, Button, FlyController, InputEvent, Key, OrbitCamera, OrbitController,
        ScaleController,
    };
}
/// Physical time: typed simulation instants and the units they are read in.
pub mod time {
    pub use molgfx_core::{PlaybackTime, SimTime, TimeUnit};
}

/// The multi-track clock: warps that map presentation seconds onto each track's
/// physical time, the timeline that applies them, and its authored form.
pub mod timeline {
    pub use crate::spec::{TimelineSpec, TrackPace, TrackSpec};
    pub use molgfx_core::{PlaybackMode, TimeWarp, Timeline, TimelineTrackHandle, TrackReading};
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
    RepresentationId, SegmentationId, StructureId, TrajectoryId, VolumeId,
};
pub use interop::{Diagnostic, MvsDocument, MvsImport, from_mvsj, to_mvsj};
#[cfg(feature = "mvsx")]
pub use interop::{from_mvsx, to_mvsx};
pub use molgfx_core::SegmentationPresentation;
#[cfg(not(target_arch = "wasm32"))]
pub use molgfx_wgpu::{AdapterReport, SystemInfo, system_info};
pub use overlay::{
    Anchor, AnnotationSpec, AssemblySpec, DataSource, EllipsoidSpec, FieldLines, FitResult,
    HydrogenBondBinding, HydrogenBondGeometry, HydrogenBonds, InteractionKind, InteractionSpec,
    IsoStyle, MeasurementSpec, OverlayHandles, OverlayId, PlaneSpec, SegmentStyle,
    SegmentationBinding, SegmentationSpec, TrajectoryBinding, TrajectoryFrame, TrajectorySpec,
    UnitCellSpec, ValidationFinding, VolumeBinding, VolumePresentation, VolumeRegion, VolumeSpec,
    VolumeTransferPoint,
};
pub use patch::{PatchOperation, ScenePatch};
pub use profile::{AntiAliasing, DepthCue, Effect, EffectKind, EffectSet, Quality, RenderProfile};
pub use property::{PropertySpec, ScalarProperty, ScalarPropertyBinding};
pub use render::PickReadback;
pub use render::{
    CompletedFrame, CpuStages, EffectiveQuality, FrameReport, FrameTiming, GpuTiming,
    MeasuredOutput, PassTiming, PassTimingCoverage, QualityTier, SurfaceLimit,
};
pub use render::{DeviceLossReport, PointCloud, Renderer};
#[cfg(not(target_arch = "wasm32"))]
pub use render::{HdrImage, Image, StereoImages, StereoLayout};
pub use render::{PickKind, PickResult};

/// Host interop: render into textures owned by an embedding application.
///
/// The one place the public API names the graphics backend. The host and the
/// renderer must agree on a single wgpu version, so it is re-exported here.
#[cfg(feature = "host")]
pub mod host {
    pub use crate::render::{HostFrame, HostOutputFormat, HostTarget};
    pub use molgfx_gpu::FenceValue;
    pub use molgfx_wgpu::{
        HostContext, HostRequirements, host_device_descriptor, host_requirements,
        unmet_requirements, wgpu,
    };
}
pub use representation::{
    MolecularSurfaceBinding, MolecularSurfaceControls, MolecularSurfaceData,
    MolecularSurfaceGeometry, MolecularSurfaceMethod, MolecularSurfaceSource, RepresentationSpec,
};
pub use representation::{SceneItem, Selection};
pub use scene::domains::AssemblyChoice;
pub use scene::hashing::structure_hash;
pub use scene::transaction::SceneTransaction;
pub use scene::{
    AssemblyCopy, PlaybackBinding, ResidueMetadata, ResolvedAtomPick, ResolvedBondPick,
    ResolvedLabelPick, ResolvedMeasurementPick, ResolvedPick, ResolvedVolumeSegmentPick,
    RuntimeBindings, Scene, chain_selection,
};
pub use spec::{InteractionChannel, SceneSpec, StructureSource};
pub use visual::{
    BoolExpr, ColorExpr, Parameter, ParameterType, ParameterValue, ScalarExpr, VectorExpr,
    VisualStyle,
};

pub mod rep {
    //! Typed representation constructors.
    pub use crate::representation::{
        Backbone, BallAndStick, BasePairs, Bases, Beads, Bonds, Cartoon, CartoonProfile,
        CartoonStyle, Dots, DynamicBonds, GapStyle, Glycan, Licorice, Lines, MolecularSurface,
        NewRibbons, NucleicAcid, PaperChain, PointRepresentation, Polyhedra, Putty, QuickSurf,
        RepresentationApplicability, RepresentationRequirement, RepresentationReview, Ribbons,
        Snfg, Solvent, Spacefill, Surface, SurfaceKind, SurfaceStyle, Trace, Tube, backbone,
        ball_and_stick, base_pairs, bases, beads, bonds, cartoon, dots, dynamic_bonds, glycan,
        licorice, lines, msms, new_cartoon, new_ribbons, nucleic_acid, paper_chain, points,
        polyhedra, putty, quick_surf, ribbons, snfg, solvent, spacefill, surf, surface, trace,
        tube,
    };
}

pub use overlay::{annotation, density, ellipsoid, interaction, measurement, trajectory};

/// `MolFrame`'s immutable molecular selection expressions.
pub mod sel {
    pub use molframe::query::col::{
        all, alpha_helix, aromatic, backbone, bend, bridge, by_residue, chain, coil, glycans,
        heavy, helix, helix_310, hetero, hydrogen, ions, is_protein as protein, ligands, lipids,
        name, none, nucleic, nucleic_backbone, nucleic_base, nucleic_sugar, occupancy, pi_helix,
        polymer, polyproline, residues_within, resname, sheet, sidechain, strand, turn, water,
        within,
    };
    pub use molframe::query::{Builder, ColumnBuilder, col};
}

pub use molframe;
pub use molgfx_math::Camera;
