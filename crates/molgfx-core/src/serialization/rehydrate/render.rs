//! Reconstruction of representation and material state.

mod volume;
use volume::{parse_segmentation_style, parse_volume_style};

use super::color::{parse_color, rgba};
use super::{Scene, insert, invalid, invalid_value, resolve_existing, resolve_raw};
use crate::CoreError;
use crate::scene::StoredRepresentation;
use crate::serialization::types::{self, TargetDescription};
use crate::serialization::types::{
    MaterialDescription, RepresentationDescription, VisualStyleDescription,
};
use crate::{
    AttributeKind, ClipCap, ClipPlane, ClipSet, Material, MaterialModel, Representation,
    RepresentationKind, RepresentationParams, RepresentationTarget, RowDomain, SurfaceKind,
    SurfaceStyle, TubeRadiusMapping, VisualAttributeRef, VisualCompatibility, VisualOutput,
    VisualProgram, VisualStyle,
};
use molgfx_math::Vec3;

pub(crate) fn rehydrate_representations(
    scene: &mut Scene,
    descriptions: &[RepresentationDescription],
) -> Result<(), crate::CoreError> {
    for description in descriptions {
        let target = parse_target(scene, &description.target)?;
        let kind = RepresentationKind::from_stable_name(&description.kind)
            .ok_or_else(|| invalid_value("manifest references an unknown representation kind"))?;
        let mut value = Representation::new(target, kind);
        value.molecular_surface = description
            .molecular_surface
            .as_ref()
            .map(crate::serialization::MolecularSurfaceDescription::geometry)
            .transpose()?;
        if kind == RepresentationKind::MolecularSurface && value.molecular_surface.is_none() {
            return invalid("analytical molecular surface requires complete geometry");
        }
        value.visible = description.visible;
        value.order = description.order;
        value.params = representation_params(
            description.params,
            description.cartoon_aspect_ratio,
            description.cartoon_arrow_factor,
        )?;
        if !description.distance_cutoff.is_finite() || description.distance_cutoff <= 0.0 {
            return invalid("distance cutoff must be finite and positive");
        }
        value.params.distance_cutoff = description.distance_cutoff;
        if !(3..=12).contains(&description.max_ring_size) {
            return invalid("ring size must lie between three and twelve");
        }
        value.params.max_ring_size = description.max_ring_size;
        if !(1..=4096).contains(&description.point_samples) {
            return invalid("point samples must lie between one and 4096");
        }
        value.params.point_samples = description.point_samples;
        value.params.gaussian_cutoff = description.gaussian_cutoff;
        value.params.surface_spacing = description.surface_spacing;
        value.params.validate_surface()?;
        if !description.cartoon_helix_radius.is_finite()
            || description.cartoon_helix_radius <= 0.0
            || !(description.cartoon_helix_radius * 2.0).is_finite()
        {
            return invalid("cartoon helix radius must be finite and positive");
        }
        value.params.cartoon_helix_radius = description.cartoon_helix_radius;
        value.params.cartoon_direction_wedges = description.cartoon_direction_wedges;
        value.params.gaps = description.gaps;
        value.params.cartoon_helix_profile = description.cartoon_helix_profile;
        value.params.cartoon_nucleic_profile = description.cartoon_nucleic_profile;
        value.params.tube_radius_mapping = parse_tube_mapping(description.tube_radius_mapping)?;
        value.params.surface_components =
            parse_surface_components(&description.surface_components)?;
        value.color = parse_color(scene, &description.color)?;
        value.material = parse_material(&description.material)?;
        value.clipping = parse_clip(&description.clipping)?;
        value.appearance = description
            .appearance
            .as_ref()
            .map(|appearance| parse_appearance(scene, appearance))
            .transpose()?;
        value.volume = parse_volume_style(&description.volume)?;
        value.segmentation = parse_segmentation_style(&description.segmentation)?;
        value.surface_scalar = description
            .surface_scalar
            .as_ref()
            .map(|surface| parse_surface_scalar(scene, surface))
            .transpose()?;
        value.visual = description
            .visual
            .as_ref()
            .map(|visual| parse_visual(scene, visual, kind))
            .transpose()?;
        validate_regions(scene, target, &value)?;
        let raw = super::raw(description.row, description.generation);
        insert(
            scene
                .representations
                .insert_at(raw, StoredRepresentation { value, revision: 0 }),
            "representation identity collision",
        )?;
    }
    Ok(())
}

fn parse_visual(
    scene: &Scene,
    value: &VisualStyleDescription,
    kind: RepresentationKind,
) -> Result<VisualStyle, CoreError> {
    if !value.attributes.is_empty() {
        return invalid("representation visual cannot reference generic row attributes");
    }
    parse_visual_with_compatibility(
        scene,
        value,
        VisualCompatibility::for_representation(kind),
        None,
    )
}

pub(super) fn parse_domain_visual(
    scene: &Scene,
    value: &VisualStyleDescription,
    domain: RowDomain,
) -> Result<VisualStyle, CoreError> {
    if !value.properties.is_empty() {
        return invalid("generic domain visual cannot reference legacy atom properties");
    }
    let compatibility = match domain {
        RowDomain::Points(_) => VisualCompatibility::POINTS,
        RowDomain::Instances(_) | RowDomain::TemplateParts(_) => VisualCompatibility::INSTANCES,
        RowDomain::Relations(_) => VisualCompatibility::RELATIONS,
        RowDomain::Atoms(_) => VisualCompatibility::DEFORMABLE,
    };
    parse_visual_with_compatibility(scene, value, compatibility, Some(domain))
}

fn parse_visual_with_compatibility(
    scene: &Scene,
    value: &VisualStyleDescription,
    compatibility: VisualCompatibility,
    domain: Option<RowDomain>,
) -> Result<VisualStyle, CoreError> {
    let instructions = value
        .instructions
        .iter()
        .map(|instruction| {
            crate::representation::visual::Instruction::from_serialized(
                instruction.opcode,
                instruction.kind,
                instruction.operands,
                instruction.data,
                instruction.stage,
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| visual_error(&error))?;
    let outputs = value
        .outputs
        .iter()
        .map(|[output, register]| {
            VisualOutput::from_code(*output)
                .map(|output| (output, *register))
                .ok_or_else(|| invalid_value("visual program contains an unknown output"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let properties = value
        .properties
        .iter()
        .map(|identity| {
            let raw = resolve_raw(*identity);
            resolve_existing(scene.properties.get(raw))?;
            Ok(crate::AtomPropertyHandle(raw))
        })
        .collect::<Result<Vec<_>, CoreError>>()?;
    let attributes = if let Some(domain) = domain {
        value
            .attributes
            .iter()
            .map(|description| {
                let raw = resolve_raw(description.identity);
                let handle = crate::AttributeHandle(raw);
                let attribute = scene.attribute(handle).ok_or_else(|| {
                    invalid_value("visual program references a stale generic attribute")
                })?;
                let kind = parse_attribute_kind(&description.kind)?;
                if attribute.domain() != domain || attribute.kind() != kind {
                    return invalid("visual attribute does not match its recorded domain or kind");
                }
                Ok(VisualAttributeRef::Attribute { handle, kind })
            })
            .collect::<Result<Vec<_>, CoreError>>()?
    } else {
        properties
            .iter()
            .copied()
            .map(VisualAttributeRef::LegacyScalar)
            .collect()
    };
    let parameter_kinds = value
        .parameter_kinds
        .iter()
        .map(|kind| {
            crate::representation::visual::ValueKind::from_code(*kind)
                .ok_or_else(|| invalid_value("visual program contains an unknown value kind"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let program = VisualProgram::from_serialized_attributes(
        instructions,
        &outputs,
        attributes,
        properties,
        parameter_kinds,
        value.parameter_defaults.clone(),
        value.maximum_displacement,
    )
    .map_err(|error| visual_error(&error))?;
    program
        .validate_compatibility(compatibility)
        .map_err(|error| visual_error(&error))?;
    VisualStyle::from_parameters(program, value.parameters.clone())
        .map_err(|error| visual_error(&error))
}

fn parse_attribute_kind(value: &str) -> Result<AttributeKind, CoreError> {
    match value {
        "scalar" => Ok(AttributeKind::Scalar),
        "category" => Ok(AttributeKind::Category),
        "vector" => Ok(AttributeKind::Vector),
        "color" => Ok(AttributeKind::Color),
        _ => invalid("visual program references an unknown attribute kind"),
    }
}

fn visual_error(error: &crate::VisualError) -> CoreError {
    CoreError::InvalidSceneDescription {
        summary: error.to_string(),
    }
}

fn parse_target(
    scene: &Scene,
    target: &TargetDescription,
) -> Result<RepresentationTarget, crate::CoreError> {
    let identity = types::ObjectIdentity {
        row: target.row,
        generation: target.generation,
    };
    let raw = resolve_raw(identity);
    match target.kind.as_str() {
        "selection" => {
            resolve_existing(scene.selections.get(raw))?;
            Ok(RepresentationTarget::Selection(crate::SelectionHandle(raw)))
        }
        "volume" => {
            resolve_existing(scene.volumes.get(raw))?;
            Ok(RepresentationTarget::Volume(crate::VolumeHandle(raw)))
        }
        "segmentation" => {
            resolve_existing(scene.segmentations.get(raw))?;
            Ok(RepresentationTarget::SegmentedVolume(
                crate::SegmentationHandle(raw),
            ))
        }
        _ => invalid("representation references an unknown target kind"),
    }
}

fn representation_params(
    values: [f32; 14],
    aspect_ratio: f32,
    arrow_factor: f32,
) -> Result<RepresentationParams, crate::CoreError> {
    if values.iter().any(|value| !value.is_finite()) {
        return invalid("representation parameters contain a non-finite value");
    }
    if !aspect_ratio.is_finite()
        || aspect_ratio <= 0.0
        || !arrow_factor.is_finite()
        || arrow_factor < 0.0
    {
        return invalid(
            "cartoon aspect ratio must be positive and arrow factor non-negative, both finite",
        );
    }
    Ok(RepresentationParams {
        cartoon_aspect_ratio: aspect_ratio,
        cartoon_helix_radius: RepresentationParams::default().cartoon_helix_radius,
        cartoon_arrow_factor: arrow_factor,
        cartoon_direction_wedges: false,
        gaps: crate::GapStyle::Hidden,
        cartoon_helix_profile: crate::CartoonProfile::Elliptical,
        cartoon_nucleic_profile: crate::CartoonProfile::Elliptical,
        radius_scale: values[0],
        bond_radius: values[1],
        distance_cutoff: RepresentationParams::default().distance_cutoff,
        max_ring_size: RepresentationParams::default().max_ring_size,
        point_samples: RepresentationParams::default().point_samples,
        probe_radius: values[2],
        gaussian_sigma: values[3],
        gaussian_cutoff: RepresentationParams::default().gaussian_cutoff,
        surface_spacing: None,
        isolevel: values[4],
        surface_kind: parse_surface_kind(values[5])?,
        surface_style: parse_surface_style(values[6])?,
        surface_components: crate::SurfaceComponentPolicy::default(),
        surface_pattern_spacing: values[7],
        surface_pattern_width_pixels: values[8],
        ribbon_width: values[9],
        tube_radius: values[10],
        tube_radius_mapping: TubeRadiusMapping::Constant,
        point_size_pixels: values[11],
        line_width_pixels: values[12],
        blob_spread: values[13],
    })
}

fn parse_surface_components(
    value: &crate::serialization::SurfaceComponentDescription,
) -> Result<crate::SurfaceComponentPolicy, crate::CoreError> {
    use crate::serialization::SurfaceComponentDescription;
    Ok(match value {
        SurfaceComponentDescription::Disabled => crate::SurfaceComponentPolicy::keep_all(),
        SurfaceComponentDescription::Area(minimum) => {
            crate::SurfaceComponentPolicy::minimum_area(*minimum)?
        }
        SurfaceComponentDescription::Volume(minimum) => {
            crate::SurfaceComponentPolicy::minimum_volume(*minimum)?
        }
        SurfaceComponentDescription::Voxels(minimum) => {
            crate::SurfaceComponentPolicy::minimum_voxels(*minimum)?
        }
    })
}

fn parse_surface_kind(value: f32) -> Result<SurfaceKind, crate::CoreError> {
    match value.to_bits() {
        value if value == 0.0f32.to_bits() => Ok(SurfaceKind::VanDerWaals),
        value if value == 1.0f32.to_bits() => Ok(SurfaceKind::SolventAccessible),
        value if value == 2.0f32.to_bits() => Ok(SurfaceKind::SolventExcluded),
        value if value == 3.0f32.to_bits() => Ok(SurfaceKind::Gaussian),
        _ => invalid("unknown surface field kind"),
    }
}

fn parse_surface_style(value: f32) -> Result<SurfaceStyle, crate::CoreError> {
    match value.to_bits() {
        value if value == 0.0f32.to_bits() => Ok(SurfaceStyle::Solid),
        value if value == 1.0f32.to_bits() => Ok(SurfaceStyle::Contour),
        value if value == 2.0f32.to_bits() => Ok(SurfaceStyle::Dots),
        value if value == 3.0f32.to_bits() => Ok(SurfaceStyle::FilledContour),
        value if value == 4.0f32.to_bits() => Ok(SurfaceStyle::Mesh),
        value if value == 5.0f32.to_bits() => Ok(SurfaceStyle::SoftUnion),
        _ => invalid("unknown surface presentation style"),
    }
}

fn parse_tube_mapping(value: Option<[f32; 4]>) -> Result<TubeRadiusMapping, crate::CoreError> {
    match value {
        None => Ok(TubeRadiusMapping::Constant),
        Some([domain_low, domain_high, radius_low, radius_high]) => {
            TubeRadiusMapping::b_factor([domain_low, domain_high], [radius_low, radius_high])
        }
    }
}

fn parse_material(value: &MaterialDescription) -> Result<Material, crate::CoreError> {
    if !value.response.iter().all(|value: &f32| value.is_finite())
        || !(0.0..=1.0).contains(&value.response[0])
        || !(0.0..=1.0).contains(&value.response[1])
        || !(0.0..=1.0).contains(&value.response[2])
        || !value.model_parameter.is_finite()
        || !(0.0..=1.0).contains(&value.model_parameter)
    {
        return invalid("material response is outside its finite range");
    }
    let model = match value.model.as_str() {
        "molecular" => MaterialModel::Molecular,
        "principled" => MaterialModel::Principled {
            metallic: value.model_parameter,
        },
        "anisotropic_ribbon" => MaterialModel::AnisotropicRibbon {
            strength: value.model_parameter,
        },
        "diffusion" => MaterialModel::Diffusion {
            strength: value.model_parameter,
        },
        _ => return invalid("unknown material model"),
    };
    Ok(Material {
        opacity: value.response[0],
        roughness: value.response[1],
        specular: value.response[2],
        model,
    })
}

fn parse_clip(value: &types::ClipDescription) -> Result<ClipSet, crate::CoreError> {
    let mut planes = Vec::with_capacity(value.planes.len());
    for plane in &value.planes {
        let normal = Vec3::new(plane[0], plane[1], plane[2]);
        if !normal.is_finite() || !plane[3].is_finite() || normal.length_squared() <= 1.0e-12 {
            return invalid("clip plane is malformed");
        }
        planes.push(ClipPlane {
            normal,
            offset: plane[3],
        });
    }
    let cap = match value.cap.as_str() {
        "open" => ClipCap::Open,
        "solid" => ClipCap::Solid,
        _ => return invalid("unknown clip cap mode"),
    };
    Ok(ClipSet::new(&planes)?.with_cap(cap))
}

mod overlays;
mod validation;
use overlays::{parse_appearance, parse_surface_scalar};
use validation::validate_regions;
