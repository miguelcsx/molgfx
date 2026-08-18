//! Public, process-independent records used by scene manifests.

use serde::{Deserialize, Serialize};

#[path = "display_descriptions.rs"]
mod display_descriptions;
#[path = "interaction_description.rs"]
mod interaction_description;
#[path = "primitive_description.rs"]
mod primitive_description;

pub use display_descriptions::*;
pub use interaction_description::InteractionDescription;
pub use primitive_description::{ParticleMotionDescription, PrimitiveDescription};

/// JSON-compatible, reproducible description of a scene composition.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct SceneDescription {
    /// Manifest schema version.
    pub schema: u16,
    /// Engine format identifier, kept separate from the package version.
    pub engine: String,
    /// Caller-supplied structures referenced by source id and coordinate hash.
    pub structures: Vec<StructureDescription>,
    /// Structure-scoped atom selections.
    pub selections: Vec<SelectionDescription>,
    /// Caller-owned per-atom scalar columns and their content fingerprints.
    pub atom_properties: Vec<AtomPropertyDescription>,
    /// Molecular and volume representations in draw order.
    pub representations: Vec<RepresentationDescription>,
    /// Resident scalar volume identities and ranges.
    pub volumes: Vec<VolumeDescription>,
    /// Resident categorical volume identities.
    pub segmentations: Vec<VolumeDescription>,
    /// Caller mesh fingerprints and presentation state.
    #[serde(default)]
    pub meshes: Vec<MeshDescription>,
    /// Transform-only occurrences of shared meshes.
    #[serde(default)]
    pub mesh_instances: Vec<MeshInstanceDescription>,
    /// Caller-authored analytic primitive payloads.
    #[serde(default, alias = "scientific_primitives")]
    pub primitives: Vec<PrimitiveDescription>,
    /// Depth-independent screen overlays.
    #[serde(default)]
    pub overlays: Vec<OverlayDescription>,
    /// Caller-authored analytic guide payloads.
    pub guides: Vec<GuideDescription>,
    /// Caller-computed interaction facts.
    pub interactions: Vec<InteractionDescription>,
    /// Persistent human-authored annotations.
    pub annotations: Vec<AnnotationDescription>,
    /// Persistent caller-computed measurements.
    pub measurements: Vec<MeasurementDescription>,
    /// Quick counts for dense or caller-owned tables.
    pub tables: TableCounts,
}

/// Alias used by applications that store the description as a figure manifest.
pub type SceneManifest = SceneDescription;

/// One caller mesh source and its scene-owned presentation state.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct MeshDescription {
    /// Stable mesh identity.
    pub row: u32,
    /// Stable mesh generation.
    pub generation: u32,
    /// Owning structure identity.
    pub owner: ObjectIdentity,
    /// Deterministic vertex/index fingerprint.
    pub content_hash: u64,
    /// Surface response.
    pub material: MaterialDescription,
    /// Clipping state.
    pub clipping: ClipDescription,
    /// `double`, `front` or `back`.
    pub face_visibility: String,
    /// Minimum provider component area.
    pub minimum_component_area: f64,
    /// Optional largest-component limit.
    pub maximum_components: Option<usize>,
    /// Whether the base occurrence draws.
    pub visible: bool,
}

/// One transform-only occurrence of a shared mesh.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct MeshInstanceDescription {
    /// Stable instance identity.
    pub row: u32,
    /// Stable instance generation.
    pub generation: u32,
    /// Shared source mesh identity.
    pub mesh: ObjectIdentity,
    /// Column-major instance transform.
    pub transform: [f32; 16],
    /// Whether this occurrence draws.
    pub visible: bool,
}

/// One typed depth-independent screen overlay.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct OverlayDescription {
    /// Stable overlay identity.
    pub row: u32,
    /// Stable overlay generation.
    pub generation: u32,
    /// `text`, `color-legend`, `scale-bar` or `coordinate-tripod`.
    pub kind: String,
    /// Normalized anchor coordinate.
    pub normalized: [f32; 2],
    /// Physical-pixel anchor offset.
    pub pixels: [f32; 2],
    /// Stable composition order.
    pub order: i16,
    /// Whether the overlay draws.
    pub visible: bool,
    /// Optional title or text.
    pub text: String,
    /// Primary and secondary colours.
    pub colors: [[u8; 4]; 2],
    /// Variant-specific scalar values.
    pub values: [f32; 4],
}

/// One placed source structure and its mismatch-detection fingerprint.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct StructureDescription {
    /// Stable slot row and generation.
    pub row: u32,
    /// Generation of the placed-structure handle.
    pub generation: u32,
    /// `pdbiox` entry id, when the source carries one.
    pub source_id: Option<String>,
    /// Source title, when available.
    pub title: Option<String>,
    /// Source experimental method, when available.
    pub method: Option<String>,
    /// Source resolution, when available.
    pub resolution: Option<f32>,
    /// Number of active atom rows.
    pub atom_count: u32,
    /// FNV-1a fingerprint over source id and active coordinate bits.
    pub coordinate_hash: u64,
    /// Column-major model-to-world transform.
    pub model_to_world: [f32; 16],
    /// Caller- or `pdbiox`-supplied residue classifications.
    pub secondary_structure: Vec<String>,
}

/// One stored selection expanded into stable structure-local atom rows.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SelectionDescription {
    /// Stable selection slot row and generation.
    pub row: u32,
    /// Generation of the selection handle.
    pub generation: u32,
    /// Per-structure rows, sorted by structure identity.
    pub masks: Vec<SelectionMask>,
}

/// One structure-local selection mask.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SelectionMask {
    /// Placed-structure slot row.
    pub structure_row: u32,
    /// Selected atom rows in ascending order.
    pub atoms: Vec<u32>,
}

/// A representation target expressed without process-local handles.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct TargetDescription {
    /// `selection`, `volume` or `segmentation`.
    pub kind: String,
    /// Target slot row.
    pub row: u32,
    /// Target slot generation.
    pub generation: u32,
}

/// Serializable state that changes molecular appearance.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct RepresentationDescription {
    /// Stable representation slot row and generation.
    pub row: u32,
    /// Generation of the representation handle.
    pub generation: u32,
    /// Stable representation kind name.
    pub kind: String,
    /// Non-process-local target identity.
    pub target: TargetDescription,
    /// Visibility and deterministic order.
    pub visible: bool,
    /// Lower values draw first.
    pub order: u16,
    /// Colouring state.
    pub color: ColorDescription,
    /// Surface response state.
    pub material: MaterialDescription,
    /// Numeric geometry knobs in the same stable order as `RepresentationParams`.
    pub params: [f32; 15],
    /// Clipping state.
    pub clipping: ClipDescription,
    /// Optional reversible variable-radius tube mapping.
    pub tube_radius_mapping: Option<[f32; 4]>,
    /// Optional property-driven opacity and silhouette softness.
    pub appearance: Option<PropertyAppearanceDescription>,
    /// Scalar-volume sampling and transfer state.
    pub volume: VolumeStyleDescription,
    /// Categorical-volume sampling and label styles.
    pub segmentation: SegmentationStyleDescription,
    /// Optional scalar field sampled over a molecular surface.
    pub surface_scalar: Option<SurfaceScalarDescription>,
}

/// Stable description of a colour source and optional scalar ramp.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ColorDescription {
    /// `element`, `chain`, `residue`, `secondary`, `property` or `uniform`.
    pub mode: String,
    /// Uniform or missing colour, when applicable.
    pub rgba: Option<[u8; 4]>,
    /// Property slot row, when applicable.
    pub property_row: Option<u32>,
    /// Property slot generation, when applicable.
    pub property_generation: Option<u32>,
    /// Numeric ramp stops, when applicable.
    pub ramp_values: Option<[u32; 3]>,
    /// Ramp colours, when applicable.
    pub ramp_colors: Option<[[u8; 4]; 3]>,
}

/// Compact material description.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct MaterialDescription {
    /// Opacity, roughness and specular strength.
    pub response: [f32; 3],
    /// Stable material model name.
    pub model: String,
    /// Model parameter, such as metalness or anisotropy.
    pub model_parameter: f32,
}

/// World-space clipping and cap description.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ClipDescription {
    /// Active planes as normal xyz plus offset.
    pub planes: Vec<[f32; 4]>,
    /// `open` or `solid`.
    pub cap: String,
}

/// Source grid identity and dimensions. Values remain caller-owned.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct VolumeDescription {
    /// Stable slot row and generation.
    pub row: u32,
    /// Generation of the volume handle.
    pub generation: u32,
    /// Grid dimensions.
    pub dimensions: [u32; 3],
    /// Scalar range; categorical volumes use `[0, 0]`.
    pub range: [f32; 2],
    /// Column-major voxel-to-world transform.
    pub voxel_to_world: [f32; 16],
    /// FNV-1a fingerprint over caller-owned samples or labels.
    pub content_hash: u64,
}

/// One caller-owned atom property column and its mismatch fingerprint.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct AtomPropertyDescription {
    /// Stable property slot row and generation.
    pub row: u32,
    /// Property handle generation.
    pub generation: u32,
    /// Owning structure identity.
    pub owner: ObjectIdentity,
    /// Caller-defined property name.
    pub name: String,
    /// Number of source atom values, including missing values.
    pub length: u32,
    /// Finite display domain.
    pub finite_domain: [f32; 2],
    /// Stable primitive meaning name.
    pub meaning: String,
    /// Scalar-field semantic metadata.
    pub semantics: ScalarSemanticsDescription,
    /// FNV-1a fingerprint over value bit patterns.
    pub content_hash: u64,
}

/// Serializable scalar-field semantics carried by a caller-owned property.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ScalarSemanticsDescription {
    /// `rank` or `quantity`.
    pub kind: String,
    /// Quantity name, when calibrated.
    pub name: Option<String>,
    /// Quantity units, when calibrated.
    pub units: Option<String>,
    /// Upstream method or dataset identifier, when calibrated.
    pub provenance: Option<String>,
}

/// Reversible property-to-appearance mapping.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct PropertyAppearanceDescription {
    /// Property slot identity.
    pub property: ObjectIdentity,
    /// Primitive input domain.
    pub domain: [f32; 2],
    /// Opacity at the domain endpoints.
    pub opacity: [f32; 2],
    /// Silhouette softness at the domain endpoints.
    pub softness_pixels: [f32; 2],
    /// Missing-value response: opacity and softness.
    pub missing: [f32; 2],
}

/// One transfer-function stop in a volume representation.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct VolumeTransferPointDescription {
    /// Scalar value.
    pub value: f32,
    /// Stop colour.
    pub color: [u8; 4],
    /// Stop opacity.
    pub opacity: f32,
}

/// Serializable scalar-volume sampling state.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct VolumeStyleDescription {
    /// Stable [`crate::VolumeRendering`] name.
    pub rendering: String,
    /// Ordered transfer stops.
    pub transfer: Vec<VolumeTransferPointDescription>,
    /// Global optical-density multiplier.
    pub opacity_scale: f32,
    /// Ray-step scale.
    pub step_scale: f32,
    /// Optional world-space sampling plane as normal xyz plus offset.
    pub slice: Option<[f32; 4]>,
    /// Optional half-open voxel crop.
    pub region: Option<RegionDescription>,
}

/// Serializable categorical-volume sampling state.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct SegmentationStyleDescription {
    /// Exact label styles.
    pub styles: Vec<SegmentStyleDescription>,
    /// Global opacity multiplier.
    pub opacity_scale: f32,
    /// Ray-step scale.
    pub step_scale: f32,
    /// Optional world-space sampling plane as normal xyz plus offset.
    pub slice: Option<[f32; 4]>,
    /// Optional half-open voxel crop.
    pub region: Option<RegionDescription>,
}

/// One exact categorical label style.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct SegmentStyleDescription {
    /// Integer label.
    pub label: u32,
    /// Display colour.
    pub color: [u8; 4],
    /// Label opacity.
    pub opacity: f32,
}

/// Serializable half-open voxel region.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct RegionDescription {
    /// Inclusive minimum voxel index.
    pub minimum: [u32; 3],
    /// Exclusive maximum voxel index.
    pub maximum: [u32; 3],
}

/// Serializable scalar field sampled over a molecular boundary.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct SurfaceScalarDescription {
    /// Scalar volume slot identity.
    pub field: ObjectIdentity,
    /// Three ramp values encoded as IEEE-754 bits.
    pub ramp_values: [u32; 3],
    /// Three ramp colours.
    pub ramp_colors: [[u8; 4]; 3],
    /// Optional contour interval and width in pixels.
    pub contours: Option<[f32; 2]>,
    /// Sampling displacement along the surface normal.
    pub sample_offset_angstrom: f32,
}
