//! Stable, source-referencing scene descriptions.
//!
//! A description is deliberately not a second structure format. It records
//! the scene-owned composition and hashes the caller-owned coordinates. The
//! caller must supply the same `pdbiox` structures again before rendering.
use super::coordinate_hash::coordinate_hash;
use super::records;
use super::types::{
    ClipDescription, ColorDescription, MaterialDescription, OccupancyDescription,
    RepresentationDescription, SceneDescription, SelectionDescription, SelectionMask,
    StructureDescription, SurfaceComponentDescription, TableCounts, TargetDescription,
    VisualInstructionDescription, VisualStyleDescription, VolumeDescription,
};
use crate::handle::{RawHandle, StructureHandle};
use crate::{
    ClipCap, ColorScheme, Material, MaterialModel, RepresentationTarget, Scene, SelectionHandle,
    VisualOutput, VisualStyle,
};
use pdviewx_math::Rgba8;
pub(crate) const SCHEMA_VERSION: u16 = 8;

#[cfg(test)]
#[path = "serialization_tests.rs"]
mod tests;

impl Scene {
    /// Captures the scene-owned composition without copying source coordinates.
    #[must_use]
    pub fn describe(&self) -> SceneDescription {
        SceneDescription {
            schema: SCHEMA_VERSION,
            engine: format!("pdviewx-scene-{SCHEMA_VERSION}"),
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
                .map(|(raw, stored)| stored_volume_description(self, raw, stored))
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
                    occupancy: None,
                })
                .collect(),
            meshes: records::meshes(self),
            mesh_instances: records::mesh_instances(self),
            primitives: records::primitives(self),
            ligand_pose_batches: super::ligand_pose_description::records(self),
            point_batches: super::generic_records::point_batches(self),
            instance_batches: super::generic_records::instance_batches(self),
            attributes: super::generic_records::attributes(self),
            relation_batches: super::generic_records::relation_batches(self),
            domain_visuals: super::generic_records::domain_visuals(self),
            overlays: records::overlays(self),
            guides: records::guides(self),
            interactions: records::interactions(self),
            annotations: records::annotations(self),
            measurements: records::measurements(self),
            tables: TableCounts {
                interactions: self.interaction_count() as u64,
                guides: self.guides().count() as u64,
                annotations: self.annotations().count() as u64,
                measurements: self.measurements().count() as u64,
                atom_properties: self.atom_properties().count() as u64,
                mesh_instances: self.mesh_instances().count() as u64,
                overlays: self.overlays().count() as u64,
                ligand_pose_batches: self.ligand_pose_batches().count() as u64,
                point_batches: self.point_batches().count() as u64,
                instance_batches: self.instance_batches().count() as u64,
                attributes: self.attributes().count() as u64,
                relation_batches: self.relation_batches().count() as u64,
                domain_visuals: self.domain_visuals().count() as u64,
            },
        }
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
        if current == *description {
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
        dataset_id: placed.dataset_id().get(),
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
        surface_components: match params.surface_components.threshold() {
            crate::SurfaceComponentThreshold::Disabled => SurfaceComponentDescription::Disabled,
            crate::SurfaceComponentThreshold::Area(minimum) => {
                SurfaceComponentDescription::Area(minimum)
            }
            crate::SurfaceComponentThreshold::Volume(minimum) => {
                SurfaceComponentDescription::Volume(minimum)
            }
            crate::SurfaceComponentThreshold::Voxels(minimum) => {
                SurfaceComponentDescription::Voxels(minimum)
            }
        },
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
        visual: representation.visual.as_ref().map(visual_description),
    }
}

pub(super) fn visual_description(style: &VisualStyle) -> VisualStyleDescription {
    let program = style.program();
    let outputs = [
        VisualOutput::BaseColor,
        VisualOutput::Opacity,
        VisualOutput::Emission,
        VisualOutput::Roughness,
        VisualOutput::Specular,
        VisualOutput::MaterialStrength,
        VisualOutput::Visibility,
        VisualOutput::SilhouetteSoftness,
        VisualOutput::RadiusScale,
        VisualOutput::WidthScale,
        VisualOutput::PositionOffset,
    ]
    .into_iter()
    .filter_map(|output| {
        program
            .output_register(output)
            .map(|register| [output.code(), register])
    })
    .collect();
    VisualStyleDescription {
        instructions: program
            .instructions()
            .iter()
            .map(|instruction| VisualInstructionDescription {
                opcode: instruction.opcode(),
                kind: instruction.kind_code(),
                operands: instruction.operands(),
                data: instruction.data(),
                stage: instruction.stage().code(),
            })
            .collect(),
        outputs,
        properties: program
            .properties()
            .iter()
            .map(|property| super::types::ObjectIdentity {
                row: property.row(),
                generation: property.generation(),
            })
            .collect(),
        attributes: program
            .attributes()
            .iter()
            .filter_map(|reference| {
                let crate::VisualAttributeRef::Attribute { handle, kind } = *reference else {
                    return None;
                };
                Some(super::types::VisualAttributeDescription {
                    identity: super::types::ObjectIdentity {
                        row: handle.row(),
                        generation: handle.generation(),
                    },
                    kind: match kind {
                        crate::AttributeKind::Scalar => "scalar",
                        crate::AttributeKind::Category => "category",
                        crate::AttributeKind::Vector => "vector",
                        crate::AttributeKind::Color => "color",
                    }
                    .into(),
                })
            })
            .collect(),
        parameter_kinds: program
            .parameter_kinds()
            .iter()
            .map(|kind| kind.code())
            .collect(),
        parameter_defaults: program.parameter_defaults().to_vec(),
        parameters: style.parameters().to_vec(),
        maximum_displacement: program.maximum_displacement(),
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

fn volume_description(raw: RawHandle, volume: &crate::ScalarVolume) -> VolumeDescription {
    VolumeDescription {
        row: raw.row(),
        generation: raw.generation(),
        dimensions: volume.dimensions(),
        range: volume.range(),
        voxel_to_world: volume.voxel_to_world().to_cols_array(),
        content_hash: records::value_hash(volume.values()),
        occupancy: None,
    }
}

fn stored_volume_description(
    scene: &Scene,
    raw: RawHandle,
    stored: &crate::scene::StoredVolume,
) -> VolumeDescription {
    if let Some(volume) = &stored.value {
        return volume_description(raw, volume);
    }
    let Some(bound) = &stored.occupancy else {
        return VolumeDescription {
            row: raw.row(),
            generation: raw.generation(),
            dimensions: [2; 3],
            range: [0.0, 1.0],
            voxel_to_world: pdviewx_math::Mat4::IDENTITY.to_cols_array(),
            content_hash: 0,
            occupancy: None,
        };
    };
    let transform = scene
        .structure(bound.structure)
        .map_or(bound.stream.voxel_to_model(), |placed| {
            placed.model_to_world * bound.stream.voxel_to_model()
        });
    VolumeDescription {
        row: raw.row(),
        generation: raw.generation(),
        dimensions: bound.stream.dimensions(),
        range: [0.0, bound.stream.maximum()],
        voxel_to_world: transform.to_cols_array(),
        content_hash: 0,
        occupancy: Some(OccupancyDescription {
            structure: crate::serialization::ObjectIdentity {
                row: bound.structure.row(),
                generation: bound.structure.generation(),
            },
            atom_rows: bound.atom_rows.to_vec(),
            voxel_to_model: bound.stream.voxel_to_model().to_cols_array(),
            decay: bound.stream.decay(),
            deposit: bound.stream.deposit(),
            maximum: bound.stream.maximum(),
        }),
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
        crate::SurfaceStyle::Mesh => 4.0,
        crate::SurfaceStyle::SoftUnion => 5.0,
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
