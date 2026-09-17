//! Reconstruction of representation and material state.

use super::{Scene, insert, invalid, invalid_value, resolve_existing, resolve_raw};
use crate::CoreError;
use crate::scene::StoredRepresentation;
use crate::serialization::types::{self, TargetDescription};
use crate::serialization::types::{
    ColorDescription, MaterialDescription, RepresentationDescription, SegmentationStyleDescription,
    SurfaceScalarDescription, VisualStyleDescription, VolumeStyleDescription,
};
use crate::{
    AttributeKind, ClipCap, ClipPlane, ClipSet, ColorScheme, Material, MaterialModel,
    PropertyAppearance, Representation, RepresentationKind, RepresentationParams,
    RepresentationTarget, RowDomain, ScalarContours, ScalarRamp, SegmentStyle, SegmentStyleTable,
    SurfaceKind, SurfaceScalarOverlay, SurfaceStyle, TubeRadiusMapping, VisualAttributeRef,
    VisualCompatibility, VisualOutput, VisualProgram, VisualStyle, VolumeRegion, VolumeRendering,
    VolumeSlice, VolumeStyle, VolumeTransferFunction, VolumeTransferPoint,
};
use molgfx_math::{Rgba8, Vec3};

pub(crate) fn rehydrate_representations(
    scene: &mut Scene,
    descriptions: &[RepresentationDescription],
) -> Result<(), crate::CoreError> {
    for description in descriptions {
        let target = parse_target(scene, &description.target)?;
        let kind = RepresentationKind::from_stable_name(&description.kind)
            .ok_or_else(|| invalid_value("manifest references an unknown representation kind"))?;
        let mut value = Representation::new(target, kind);
        value.visible = description.visible;
        value.order = description.order;
        value.params = representation_params(description.params)?;
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

fn representation_params(values: [f32; 15]) -> Result<RepresentationParams, crate::CoreError> {
    if values.iter().any(|value| !value.is_finite()) {
        return invalid("representation parameters contain a non-finite value");
    }
    Ok(RepresentationParams {
        radius_scale: values[0],
        bond_radius: values[1],
        probe_radius: values[2],
        gaussian_sigma: values[3],
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

fn parse_color(scene: &Scene, value: &ColorDescription) -> Result<ColorScheme, crate::CoreError> {
    match value.mode.as_str() {
        "element" => Ok(ColorScheme::ByElement),
        "chain" => Ok(ColorScheme::ByChain),
        "residue" => Ok(ColorScheme::ByResidue),
        "secondary" => Ok(ColorScheme::BySecondaryStructure),
        "uniform" => Ok(ColorScheme::Uniform(parse_rgba(value.rgba)?)),
        "property" => {
            let row = value
                .property_row
                .ok_or_else(|| invalid_value("property colour has no property row"))?;
            let generation = value
                .property_generation
                .ok_or_else(|| invalid_value("property colour has no property generation"))?;
            let raw = super::resolve_raw(types::ObjectIdentity { row, generation });
            resolve_existing(scene.properties.get(raw))?;
            let values = value
                .ramp_values
                .ok_or_else(|| invalid_value("property colour has no ramp values"))?
                .map(f32::from_bits);
            let colors = value
                .ramp_colors
                .ok_or_else(|| invalid_value("property colour has no ramp colours"))?
                .map(rgba);
            Ok(ColorScheme::ByProperty {
                property: crate::AtomPropertyHandle(raw),
                ramp: ScalarRamp::new(values, colors)?,
                missing: parse_rgba(value.rgba)?,
            })
        }
        _ => invalid("unknown colour mode"),
    }
}

fn parse_rgba(value: Option<[u8; 4]>) -> Result<Rgba8, crate::CoreError> {
    value
        .map(rgba)
        .ok_or_else(|| invalid_value("manifest colour is missing its RGBA value"))
}

fn rgba(value: [u8; 4]) -> Rgba8 {
    Rgba8::new(value[0], value[1], value[2], value[3])
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

fn parse_volume_style(value: &VolumeStyleDescription) -> Result<VolumeStyle, crate::CoreError> {
    let points = value
        .transfer
        .iter()
        .map(|point| VolumeTransferPoint::new(point.value, rgba(point.color), point.opacity))
        .collect::<Vec<_>>();
    let transfer = VolumeTransferFunction::new(&points)?;
    Ok(VolumeStyle {
        rendering: parse_volume_rendering(&value.rendering)?,
        transfer,
        opacity_scale: finite_nonnegative(value.opacity_scale, "volume opacity scale")?,
        step_scale: positive_finite(value.step_scale, "volume step scale")?,
        slice: value.slice.map(parse_slice).transpose()?,
        region: value.region.map(parse_region).transpose()?,
    })
}

fn parse_volume_rendering(value: &str) -> Result<VolumeRendering, crate::CoreError> {
    match value {
        "direct" => Ok(VolumeRendering::Direct),
        "isosurface" => Ok(VolumeRendering::Isosurface),
        "medium" => Ok(VolumeRendering::Medium),
        "slice" => Ok(VolumeRendering::Slice),
        "liquid_surface" => Ok(VolumeRendering::LiquidSurface),
        _ => invalid("unknown volume rendering mode"),
    }
}

fn parse_segmentation_style(
    value: &SegmentationStyleDescription,
) -> Result<crate::SegmentationStyle, crate::CoreError> {
    let styles = value
        .styles
        .iter()
        .map(|style| SegmentStyle::new(style.label, rgba(style.color), style.opacity))
        .collect::<Vec<_>>();
    Ok(crate::SegmentationStyle {
        styles: SegmentStyleTable::new(&styles)?,
        opacity_scale: finite_nonnegative(value.opacity_scale, "segmentation opacity scale")?,
        step_scale: positive_finite(value.step_scale, "segmentation step scale")?,
        slice: value.slice.map(parse_slice).transpose()?,
        region: value.region.map(parse_region).transpose()?,
    })
}

fn parse_slice(value: [f32; 4]) -> Result<VolumeSlice, crate::CoreError> {
    let normal = Vec3::new(value[0], value[1], value[2]);
    if !normal.is_finite() || !value[3].is_finite() || normal.length_squared() <= 1.0e-12 {
        return invalid("volume slice plane is malformed");
    }
    Ok(VolumeSlice::new(ClipPlane {
        normal,
        offset: value[3],
    }))
}

fn parse_region(value: types::RegionDescription) -> Result<VolumeRegion, crate::CoreError> {
    if (0..3).any(|axis| value.minimum[axis] >= value.maximum[axis]) {
        return invalid("volume region is empty");
    }
    VolumeRegion::new(value.minimum, value.maximum, [u32::MAX; 3])
}

fn finite_nonnegative(value: f32, label: &'static str) -> Result<f32, crate::CoreError> {
    if value.is_finite() && value >= 0.0 {
        Ok(value)
    } else {
        invalid(label)
    }
}

fn positive_finite(value: f32, label: &'static str) -> Result<f32, crate::CoreError> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        invalid(label)
    }
}

include!("rehydrate_render_validation.rs");
