//! Molecular representation state carried by restorable scene descriptions.

use super::types::{
    ClipDescription, ColorDescription, MaterialDescription, PropertyAppearanceDescription,
    SegmentationStyleDescription, SurfaceScalarDescription, TargetDescription,
    VisualStyleDescription, VolumeStyleDescription,
};
use serde::{Deserialize, Serialize};

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
    /// Numeric geometry controls in the fixed order of `representation_params`.
    /// Material opacity and draw order are stored only in their named records.
    pub params: [f32; 14],
    /// Secondary-structure cross-section aspect ratio.
    pub cartoon_aspect_ratio: f32,
    /// Strand arrow shoulder scale.
    pub cartoon_arrow_factor: f32,
    /// Source-anchored polymer direction wedges.
    pub cartoon_direction_wedges: bool,
    /// Protein helix cross-section.
    pub cartoon_helix_profile: crate::CartoonProfile,
    /// Nucleic backbone cross-section.
    pub cartoon_nucleic_profile: crate::CartoonProfile,
    /// Sampled-field connected-component threshold.
    pub surface_components: SurfaceComponentDescription,
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
    /// Optional safe typed visual program and its current parameter block.
    pub visual: Option<VisualStyleDescription>,
}

/// Serializable sampled-surface connected-component policy.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "measure", content = "minimum", rename_all = "snake_case")]
pub enum SurfaceComponentDescription {
    /// Keep every component.
    Disabled,
    /// Minimum exposed-face area in square Angstrom.
    Area(f64),
    /// Minimum occupied volume in cubic Angstrom.
    Volume(f64),
    /// Minimum occupied voxel count.
    Voxels(u64),
}
