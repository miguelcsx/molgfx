//! Stable, source-referencing scene descriptions.
//!
//! A description is deliberately not a second structure format. It records
//! the scene-owned composition and hashes the caller-owned coordinates. The
//! caller must supply the same `pdbiox` structures again before rendering.
use super::records;
use super::types::{
    ClipDescription, ColorDescription, MaterialDescription, RepresentationDescription,
    SceneDescription, SelectionDescription, SelectionMask, StructureDescription, TableCounts,
    TargetDescription, VolumeDescription,
};
use crate::handle::{RawHandle, StructureHandle};
use crate::{
    ClipCap, ColorScheme, Material, MaterialModel, RepresentationTarget, Scene, SelectionHandle,
};
use pdviewx_math::Rgba8;
pub(crate) const SCHEMA_VERSION: u16 = 4;

#[cfg(test)]
#[path = "serialization_tests.rs"]
mod tests;

impl SceneDescription {
    /// Encodes this description as stable pretty JSON.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CoreError::InvalidSceneDescription`] if serialization
    /// fails.
    pub fn to_json(&self) -> Result<String, crate::CoreError> {
        serde_json::to_string_pretty(self).map_err(|error| {
            crate::CoreError::InvalidSceneDescription {
                summary: error.to_string(),
            }
        })
    }

    /// Decodes a manifest without touching structure data.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CoreError::InvalidSceneDescription`] for malformed
    /// JSON or an unsupported schema version.
    pub fn from_json(source: &str) -> Result<Self, crate::CoreError> {
        let description: Self = serde_json::from_str(source).map_err(|error| {
            crate::CoreError::InvalidSceneDescription {
                summary: error.to_string(),
            }
        })?;
        matches!(description.schema, 3 | SCHEMA_VERSION)
            .then_some(description)
            .ok_or_else(|| crate::CoreError::InvalidSceneDescription {
                summary: format!("unsupported scene schema; expected 3 or {SCHEMA_VERSION}"),
            })
    }
}

impl Scene {
    /// Captures the scene-owned composition without copying source coordinates.
    #[must_use]
    pub fn describe(&self) -> SceneDescription {
        SceneDescription {
            schema: SCHEMA_VERSION,
            engine: "pdviewx-scene-4".to_owned(),
            structures: self
                .structures
                .iter()
                .map(|(raw, placed)| structure_description(StructureHandle(raw), placed))
                .collect(),
            selections: self
                .selections
                .iter()
                .map(|(raw, _)| selection_description(self, SelectionHandle(raw)))
                .collect(),
            representations: self
                .representations
                .iter()
                .map(|(raw, stored)| representation_description(raw, &stored.value))
                .collect(),
            atom_properties: records::atom_properties(self),
            volumes: self
                .volumes
                .iter()
                .map(|(raw, stored)| volume_description(raw, &stored.value))
                .collect(),
            segmentations: self
                .segmentations
                .iter()
                .map(|(raw, stored)| VolumeDescription {
                    row: raw.row(),
                    generation: raw.generation(),
                    dimensions: stored.value.dimensions(),
                    range: [0.0, 0.0],
                    voxel_to_world: stored.value.voxel_to_world().to_cols_array(),
                    content_hash: records::label_hash(stored.value.labels()),
                })
                .collect(),
            meshes: records::meshes(self),
            mesh_instances: records::mesh_instances(self),
            primitives: records::primitives(self),
            overlays: records::overlays(self),
            guides: records::guides(self),
            interactions: records::interactions(self),
            annotations: records::annotations(self),
            measurements: records::measurements(self),
            tables: TableCounts {
                interactions: saturating_u32(self.interaction_count()),
                guides: saturating_u32(self.guides().count()),
                annotations: saturating_u32(self.annotations().count()),
                measurements: saturating_u32(self.measurements().count()),
                atom_properties: saturating_u32(self.atom_properties().count()),
                mesh_instances: saturating_u32(self.mesh_instances().count()),
                overlays: saturating_u32(self.overlays().count()),
            },
        }
    }

    /// Encodes the current scene description as JSON.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CoreError::InvalidSceneDescription`] if serialization
    /// fails.
    pub fn to_json(&self) -> Result<String, crate::CoreError> {
        self.describe().to_json()
    }

    /// Checks source hashes and all scene-owned composition against a manifest.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CoreError::InvalidSceneDescription`] when any source
    /// fingerprint or scene-owned setting differs.
    pub fn validate_description(
        &self,
        description: &SceneDescription,
    ) -> Result<(), crate::CoreError> {
        let current = self.describe();
        let equivalent = if description.schema == 3 {
            current.structures == description.structures
                && current.selections == description.selections
                && current.atom_properties == description.atom_properties
                && current.representations == description.representations
                && current.volumes == description.volumes
                && current.segmentations == description.segmentations
                && current.primitives == description.primitives
                && current.guides == description.guides
                && current.interactions == description.interactions
                && current.annotations == description.annotations
                && current.measurements == description.measurements
        } else {
            current == *description
        };
        if equivalent {
            Ok(())
        } else {
            Err(crate::CoreError::InvalidSceneDescription {
                summary: "scene contents do not match the manifest".to_owned(),
            })
        }
    }
}

fn structure_description(
    handle: StructureHandle,
    placed: &crate::PlacedStructure,
) -> StructureDescription {
    let entry = &placed.structure.data().entry;
    StructureDescription {
        row: handle.row(),
        generation: handle.generation(),
        source_id: entry.id.as_deref().map(str::to_owned),
        title: entry.title.as_deref().map(str::to_owned),
        method: entry.method.as_deref().map(str::to_owned),
        resolution: entry.resolution,
        atom_count: placed.atoms.len(),
        coordinate_hash: coordinate_hash(placed),
        model_to_world: placed.model_to_world.to_cols_array(),
        secondary_structure: placed
            .secondary_structure
            .values()
            .iter()
            .map(|value| secondary_name(*value).to_owned())
            .collect(),
    }
}

fn selection_description(scene: &Scene, handle: SelectionHandle) -> SelectionDescription {
    let mut masks = Vec::new();
    for (raw, placed) in scene.structures.iter() {
        let structure = StructureHandle(raw);
        let Some(selection) = scene.selection_for(handle, structure) else {
            continue;
        };
        let mut atoms = Vec::new();
        selection.for_each(placed.atoms.len(), |atom| atoms.push(atom));
        masks.push(SelectionMask {
            structure_row: structure.row(),
            atoms,
        });
    }
    SelectionDescription {
        row: handle.row(),
        generation: handle.generation(),
        masks,
    }
}

fn representation_description(
    raw: RawHandle,
    representation: &crate::Representation,
) -> RepresentationDescription {
    let target = match representation.target {
        RepresentationTarget::Selection(handle) => TargetDescription {
            kind: "selection".to_owned(),
            row: handle.row(),
            generation: handle.generation(),
        },
        RepresentationTarget::Volume(handle) => TargetDescription {
            kind: "volume".to_owned(),
            row: handle.row(),
            generation: handle.generation(),
        },
        RepresentationTarget::SegmentedVolume(handle) => TargetDescription {
            kind: "segmentation".to_owned(),
            row: handle.row(),
            generation: handle.generation(),
        },
    };
    let params = representation.params;
    RepresentationDescription {
        row: raw.row(),
        generation: raw.generation(),
        kind: representation.kind.stable_name().to_owned(),
        target,
        visible: representation.visible,
        order: representation.order,
        color: color_description(representation.color),
        material: material_description(representation.material),
        params: [
            params.radius_scale,
            params.bond_radius,
            params.probe_radius,
            params.gaussian_sigma,
            params.isolevel,
            surface_kind_value(params.surface_kind),
            surface_style_value(params.surface_style),
            params.surface_pattern_spacing,
            params.surface_pattern_width_pixels,
            params.ribbon_width,
            params.tube_radius,
            params.point_size_pixels,
            params.line_width_pixels,
            f32::from(representation.material.opacity_unorm8()),
            f32::from(representation.order),
        ],
        clipping: clip_description(representation.clipping),
        tube_radius_mapping: params
            .tube_radius_mapping
            .b_factor_parameters()
            .map(|(domain, radii)| [domain[0], domain[1], radii[0], radii[1]]),
        appearance: representation
            .appearance
            .map(records::appearance_description),
        volume: records::volume_style_description(representation.volume),
        segmentation: records::segmentation_style_description(&representation.segmentation),
        surface_scalar: representation
            .surface_scalar
            .map(records::surface_scalar_description),
    }
}

fn color_description(color: ColorScheme) -> ColorDescription {
    match color {
        ColorScheme::ByElement => simple_color("element"),
        ColorScheme::ByChain => simple_color("chain"),
        ColorScheme::ByResidue => simple_color("residue"),
        ColorScheme::BySecondaryStructure => simple_color("secondary"),
        ColorScheme::Uniform(value) => ColorDescription {
            mode: "uniform".to_owned(),
            rgba: Some([value.r, value.g, value.b, value.a]),
            property_row: None,
            property_generation: None,
            ramp_values: None,
            ramp_colors: None,
        },
        ColorScheme::ByProperty {
            property,
            ramp,
            missing,
        } => ColorDescription {
            mode: "property".to_owned(),
            rgba: Some([missing.r, missing.g, missing.b, missing.a]),
            property_row: Some(property.row()),
            property_generation: Some(property.generation()),
            ramp_values: Some(ramp.values().map(f32::to_bits)),
            ramp_colors: Some(ramp.colors().map(rgba_array)),
        },
    }
}

fn simple_color(mode: &str) -> ColorDescription {
    ColorDescription {
        mode: mode.to_owned(),
        rgba: None,
        property_row: None,
        property_generation: None,
        ramp_values: None,
        ramp_colors: None,
    }
}

pub(crate) fn material_description(material: Material) -> MaterialDescription {
    let (model, parameter) = match material.model {
        MaterialModel::Molecular => ("molecular", 0.0),
        MaterialModel::Principled { metallic } => ("principled", metallic),
        MaterialModel::AnisotropicRibbon { strength } => ("anisotropic_ribbon", strength),
        MaterialModel::Diffusion { strength } => ("diffusion", strength),
    };
    MaterialDescription {
        response: [material.opacity, material.roughness, material.specular],
        model: model.to_owned(),
        model_parameter: parameter,
    }
}

pub(crate) fn clip_description(clipping: crate::ClipSet) -> ClipDescription {
    ClipDescription {
        planes: clipping
            .planes()
            .iter()
            .map(|plane| [plane.normal.x, plane.normal.y, plane.normal.z, plane.offset])
            .collect(),
        cap: match clipping.cap() {
            ClipCap::Open => "open".to_owned(),
            ClipCap::Solid => "solid".to_owned(),
        },
    }
}

fn volume_description(raw: RawHandle, volume: &crate::DensityVolume) -> VolumeDescription {
    VolumeDescription {
        row: raw.row(),
        generation: raw.generation(),
        dimensions: volume.dimensions(),
        range: volume.range(),
        voxel_to_world: volume.voxel_to_world().to_cols_array(),
        content_hash: records::value_hash(volume.values()),
    }
}

fn rgba_array(color: Rgba8) -> [u8; 4] {
    [color.r, color.g, color.b, color.a]
}

fn surface_kind_value(kind: crate::SurfaceKind) -> f32 {
    match kind {
        crate::SurfaceKind::VanDerWaals => 0.0,
        crate::SurfaceKind::SolventAccessible => 1.0,
        crate::SurfaceKind::SolventExcluded => 2.0,
        crate::SurfaceKind::Gaussian => 3.0,
    }
}

fn surface_style_value(style: crate::SurfaceStyle) -> f32 {
    match style {
        crate::SurfaceStyle::Solid => 0.0,
        crate::SurfaceStyle::Contour => 1.0,
        crate::SurfaceStyle::Dots => 2.0,
        crate::SurfaceStyle::FilledContour => 3.0,
    }
}

fn secondary_name(value: crate::SecondaryStructure) -> &'static str {
    match value {
        crate::SecondaryStructure::Coil => "coil",
        crate::SecondaryStructure::Helix => "helix",
        crate::SecondaryStructure::Strand => "strand",
        crate::SecondaryStructure::Turn => "turn",
    }
}

pub(crate) fn coordinate_hash(placed: &crate::PlacedStructure) -> u64 {
    let mut hash = 14_695_981_039_346_656_037u64;
    let update = |hash: &mut u64, byte: u8| {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(1_099_511_628_211);
    };
    if let Some(id) = placed.structure.data().entry.id.as_deref() {
        for byte in id.as_bytes() {
            update(&mut hash, *byte);
        }
    }
    for coordinate in placed.atoms.coords().slice() {
        for component in coordinate {
            for byte in component.to_bits().to_le_bytes() {
                update(&mut hash, byte);
            }
        }
    }
    hash
}

fn saturating_u32(value: usize) -> u32 {
    match u32::try_from(value) {
        Ok(value) => value,
        Err(_) => u32::MAX,
    }
}
