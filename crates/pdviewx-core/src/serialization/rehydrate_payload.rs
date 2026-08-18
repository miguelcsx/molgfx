//! Reconstruction of primitives, guides, interactions and labels.

use super::{Scene, insert, invalid, invalid_value, resolve_existing, resolve_raw};
use crate::annotation::LabelObject;
use crate::serialization::types;
use crate::serialization::types::{
    AnchorDescription, AnnotationDescription, EntityDescription, GuideDescription,
    InteractionDescription, MeasurementDescription, ParticleMotionDescription,
    PrimitiveDescription,
};
use crate::{
    AnisotropicEllipsoid, Annotation, AnnotationAnchor, CarbohydrateShape, CarbohydrateSymbol,
    EntityKind, EntityRef, Guide, GuideCap, GuideStyle, InteractionAnchor, InteractionDirection,
    InteractionEdge, InteractionGeometry, InteractionKind, InteractionPattern, MarkerShape,
    MarkerStyle, Measurement, Particle, ParticleBoundary, ParticleMotion, ParticleShape,
    PlanarRegion, Primitive,
};
use pdviewx_math::{Aabb, Quat, Rgba8, Vec3};

pub(crate) fn rehydrate_primitives(
    scene: &mut Scene,
    descriptions: &[PrimitiveDescription],
) -> Result<(), crate::CoreError> {
    for description in descriptions {
        let owner = owner(scene, description.owner)?;
        let value = match description.kind.as_str() {
            "ellipsoid" => {
                let tensor = description
                    .tensor
                    .ok_or_else(|| invalid_value("ellipsoid has no tensor"))?;
                Primitive::Ellipsoid {
                    owner,
                    value: AnisotropicEllipsoid::new(Vec3::from_array(description.center), tensor)?,
                    color: rgba(description.color),
                    opacity: description.opacity,
                    visible: description.visible,
                }
            }
            "carbohydrate" => {
                let shape = parse_carbohydrate(
                    description
                        .shape
                        .as_deref()
                        .ok_or_else(|| invalid_value("carbohydrate has no shape"))?,
                )?;
                let mut value = CarbohydrateSymbol::new(
                    owner,
                    Vec3::from_array(description.center),
                    Quat::from_array(description.orientation),
                    Vec3::from_array(description.size),
                    shape,
                    rgba(description.color),
                )?;
                value.visible = description.visible;
                Primitive::Carbohydrate(value)
            }
            "planar" => {
                let axes = description
                    .plane_axes
                    .ok_or_else(|| invalid_value("planar primitive has no axes"))?;
                Primitive::Planar {
                    value: PlanarRegion::new(
                        owner,
                        Vec3::from_array(description.center),
                        Vec3::from_array(axes[0]),
                        Vec3::from_array(axes[1]),
                        [description.size[0], description.size[1]],
                    )?,
                    color: rgba(description.color),
                    opacity: description.opacity,
                    visible: description.visible,
                }
            }
            "particle" => {
                let shape = parse_particle_shape(
                    description
                        .shape
                        .as_deref()
                        .ok_or_else(|| invalid_value("particle has no shape"))?,
                )?;
                let mut value = Particle::new(
                    owner,
                    Vec3::from_array(description.center),
                    Quat::from_array(description.orientation),
                    Vec3::from_array(description.size),
                    shape,
                    rgba(description.color),
                    description.opacity,
                )?;
                if shape == ParticleShape::Superquadric {
                    value = value.with_superquadric_exponents(
                        description.shape_parameters[0],
                        description.shape_parameters[1],
                    )?;
                }
                if let Some(motion) = description.motion.as_ref() {
                    value = value.with_motion(parse_particle_motion(motion)?);
                }
                value.visible = description.visible;
                Primitive::Particle(value)
            }
            _ => return invalid("unknown primitive kind"),
        };
        insert(
            scene
                .primitive
                .insert_at(super::raw(description.row, description.generation), value),
            "primitive identity collision",
        )?;
    }
    Ok(())
}

pub(crate) fn rehydrate_guides(
    scene: &mut Scene,
    descriptions: &[GuideDescription],
) -> Result<(), crate::CoreError> {
    for description in descriptions {
        let owner = owner(scene, description.owner)?;
        let mut value = Guide::new(
            owner,
            Vec3::from_array(description.start),
            Vec3::from_array(description.end),
            parse_guide_style(&description.style)?,
        )?;
        value.set_visible(description.visible);
        insert(
            scene
                .guides
                .insert_at(super::raw(description.row, description.generation), value),
            "guide identity collision",
        )?;
    }
    Ok(())
}

pub(crate) fn rehydrate_interactions(
    scene: &mut Scene,
    descriptions: &[InteractionDescription],
) -> Result<(), crate::CoreError> {
    for description in descriptions {
        let owner = owner(scene, description.owner)?;
        let mut value = InteractionEdge::new(
            owner,
            parse_interaction_anchor(scene, &description.start)?,
            parse_interaction_anchor(scene, &description.end)?,
            parse_interaction_kind(&description.kind)?,
            InteractionGeometry::new(description.distance_angstrom, description.angle_degrees)?,
            description.provenance.clone(),
        )?
        .with_direction(parse_interaction_direction(&description.direction)?);
        if let Some(occupancy) = description.occupancy {
            value = value.with_occupancy(occupancy)?;
        }
        if let Some(strength) = description.normalized_strength {
            value = value.with_normalized_strength(strength)?;
        }
        value = value.with_phase_speed(description.phase_speed_pixels_per_frame)?;
        value = value.with_persistence(
            description.persistence_age_frames,
            description.persistence_half_life_frames,
        )?;
        value.set_visible(description.visible);
        insert(
            scene
                .interactions
                .insert_at(super::raw(description.row, description.generation), value),
            "interaction identity collision",
        )?;
    }
    Ok(())
}

pub(crate) fn rehydrate_labels(
    scene: &mut Scene,
    annotations: &[AnnotationDescription],
    measurements: &[MeasurementDescription],
) -> Result<(), crate::CoreError> {
    for description in annotations {
        let value = parse_annotation(scene, description)?;
        insert(
            scene.labels.insert_at(
                super::raw(description.row, description.generation),
                LabelObject::Annotation(value),
            ),
            "annotation identity collision",
        )?;
    }
    for description in measurements {
        let value = parse_measurement(scene, description)?;
        insert(
            scene.labels.insert_at(
                super::raw(description.row, description.generation),
                LabelObject::Measurement(value),
            ),
            "measurement identity collision",
        )?;
    }
    Ok(())
}

fn owner(
    scene: &Scene,
    identity: types::ObjectIdentity,
) -> Result<crate::StructureHandle, crate::CoreError> {
    let raw = resolve_raw(identity);
    resolve_existing(scene.structures.get(raw)).map(|_| crate::StructureHandle(raw))
}

fn parse_carbohydrate(value: &str) -> Result<CarbohydrateShape, crate::CoreError> {
    match value {
        "unknown" => Ok(CarbohydrateShape::Unknown),
        "glc" => Ok(CarbohydrateShape::Glc),
        "gal" => Ok(CarbohydrateShape::Gal),
        "man" => Ok(CarbohydrateShape::Man),
        "fuc" => Ok(CarbohydrateShape::Fuc),
        "xyl" => Ok(CarbohydrateShape::Xyl),
        "neu5ac" => Ok(CarbohydrateShape::Neu5Ac),
        _ => invalid("unknown carbohydrate shape"),
    }
}

fn parse_particle_shape(value: &str) -> Result<ParticleShape, crate::CoreError> {
    match value {
        "sphere" => Ok(ParticleShape::Sphere),
        "box" => Ok(ParticleShape::Box),
        "cylinder" => Ok(ParticleShape::Cylinder),
        "spherocylinder" => Ok(ParticleShape::Spherocylinder),
        "gaussian" => Ok(ParticleShape::Gaussian),
        "circle" => Ok(ParticleShape::Circle),
        "square" => Ok(ParticleShape::Square),
        "superquadric" => Ok(ParticleShape::Superquadric),
        _ => invalid("unknown particle shape"),
    }
}

fn parse_particle_motion(
    value: &ParticleMotionDescription,
) -> Result<ParticleMotion, crate::CoreError> {
    let boundary = match value.boundary.as_str() {
        "bounce" => ParticleBoundary::Bounce,
        "wrap" => ParticleBoundary::Wrap,
        _ => return invalid("unknown particle boundary"),
    };
    let motion = ParticleMotion::new(
        Vec3::from_array(value.velocity),
        Aabb::new(
            Vec3::from_array(value.bounds[0]),
            Vec3::from_array(value.bounds[1]),
        ),
        value.fixed_timestep,
        value.seed,
        boundary,
    )?;
    Ok(motion.with_respawn_after_steps(value.respawn_after_steps))
}

fn parse_guide_style(value: &types::GuideStyleDescription) -> Result<GuideStyle, crate::CoreError> {
    Ok(GuideStyle {
        color: rgba(value.color),
        pattern: parse_pattern(&value.pattern)?,
        width_pixels: value.width_pixels,
        opacity: value.opacity,
        period_pixels: value.period_pixels,
        duty_cycle: value.duty_cycle,
        cap: parse_guide_cap(&value.cap)?,
        arrow_pixels: value.arrow_pixels,
    })
}

fn parse_pattern(value: &str) -> Result<InteractionPattern, crate::CoreError> {
    match value {
        "solid" => Ok(InteractionPattern::Solid),
        "dashes" => Ok(InteractionPattern::Dashes),
        "dots" => Ok(InteractionPattern::Dots),
        "spring" => Ok(InteractionPattern::Spring),
        _ => invalid("unknown guide pattern"),
    }
}

fn parse_guide_cap(value: &str) -> Result<GuideCap, crate::CoreError> {
    match value {
        "none" => Ok(GuideCap::None),
        "arrow" => Ok(GuideCap::Arrow),
        "double_arrow" => Ok(GuideCap::DoubleArrow),
        _ => invalid("unknown guide cap"),
    }
}

fn parse_interaction_anchor(
    scene: &Scene,
    value: &AnchorDescription,
) -> Result<InteractionAnchor, crate::CoreError> {
    match value.entity.as_ref() {
        Some(entity) => InteractionAnchor::entity(
            Vec3::from_array(value.position),
            parse_entity(scene, entity)?,
        ),
        None => InteractionAnchor::world(Vec3::from_array(value.position)),
    }
}

fn parse_entity(scene: &Scene, value: &EntityDescription) -> Result<EntityRef, crate::CoreError> {
    let structure = owner(scene, value.structure)?;
    let kind = match value.kind.as_str() {
        "atom" => EntityKind::Atom,
        "bond" => EntityKind::Bond,
        "edge" => EntityKind::Edge,
        "label" => EntityKind::Label,
        "primitive" => EntityKind::Primitive,
        "mesh" => EntityKind::Mesh,
        _ => return invalid("unknown entity provenance kind"),
    };
    Ok(EntityRef {
        structure,
        kind,
        index: value.index,
    })
}

fn parse_interaction_kind(value: &str) -> Result<InteractionKind, crate::CoreError> {
    match value {
        "hydrogen_bond" => Ok(InteractionKind::HydrogenBond),
        "salt_bridge" => Ok(InteractionKind::SaltBridge),
        "pi_stacking" => Ok(InteractionKind::PiStacking),
        "hydrophobic" => Ok(InteractionKind::Hydrophobic),
        "metal_coordination" => Ok(InteractionKind::MetalCoordination),
        _ => invalid("unknown interaction kind"),
    }
}

fn parse_interaction_direction(value: &str) -> Result<InteractionDirection, crate::CoreError> {
    match value {
        "undirected" => Ok(InteractionDirection::Undirected),
        "forward" => Ok(InteractionDirection::Forward),
        "reverse" => Ok(InteractionDirection::Reverse),
        _ => invalid("unknown interaction direction"),
    }
}

fn parse_annotation(
    scene: &Scene,
    value: &AnnotationDescription,
) -> Result<Annotation, crate::CoreError> {
    let owner = owner(scene, value.owner)?;
    let mut annotation = match value.kind.as_str() {
        "note" => Annotation::note(
            owner,
            parse_annotation_anchor(scene, value.anchor.as_ref())?,
            value.text.clone(),
        )?,
        "hypothesis" => Annotation::hypothesis(
            owner,
            parse_annotation_anchor(scene, value.anchor.as_ref())?,
            value.text.clone(),
        )?,
        "marker" => Annotation::marker(
            owner,
            parse_annotation_anchor(scene, value.anchor.as_ref())?,
            parse_marker_style(&value.marker)?,
        )?,
        "region" => Annotation::region(
            owner,
            selection_handle(scene, value.region)?,
            value.text.clone(),
        )?,
        _ => return invalid("unknown annotation kind"),
    };
    annotation = annotation.with_priority(value.priority);
    annotation.set_visible(value.visible);
    Ok(annotation)
}

fn parse_annotation_anchor(
    scene: &Scene,
    value: Option<&AnchorDescription>,
) -> Result<AnnotationAnchor, crate::CoreError> {
    let value = value.ok_or_else(|| invalid_value("annotation kind requires an anchor"))?;
    match value.entity.as_ref() {
        Some(entity) => AnnotationAnchor::entity(
            Vec3::from_array(value.position),
            parse_entity(scene, entity)?,
        ),
        None => AnnotationAnchor::world(Vec3::from_array(value.position)),
    }
}

fn parse_marker_style(
    value: &types::MarkerStyleDescription,
) -> Result<MarkerStyle, crate::CoreError> {
    Ok(MarkerStyle {
        color: rgba(value.color),
        radius_pixels: value.radius_pixels,
        shape: match value.shape.as_str() {
            "circle" => MarkerShape::Circle,
            "diamond" => MarkerShape::Diamond,
            "crosshair" => MarkerShape::Crosshair,
            _ => return invalid("unknown marker shape"),
        },
    })
}

fn selection_handle(
    scene: &Scene,
    value: Option<types::ObjectIdentity>,
) -> Result<crate::SelectionHandle, crate::CoreError> {
    let identity = value.ok_or_else(|| invalid_value("region annotation has no selection"))?;
    let raw = resolve_raw(identity);
    resolve_existing(scene.selections.get(raw))?;
    Ok(crate::SelectionHandle(raw))
}

fn parse_measurement(
    scene: &Scene,
    value: &MeasurementDescription,
) -> Result<Measurement, crate::CoreError> {
    let owner = owner(scene, value.owner)?;
    let anchors = value
        .anchors
        .iter()
        .map(|anchor| parse_annotation_anchor(scene, Some(anchor)))
        .collect::<Result<Vec<_>, _>>()?;
    let mut measurement = match value.kind.as_str() {
        "distance" => Measurement::distance(
            owner,
            [anchor(&anchors, 0)?, anchor(&anchors, 1)?],
            value.value,
            value.provenance.clone(),
        )?,
        "angle" => Measurement::angle(
            owner,
            [
                anchor(&anchors, 0)?,
                anchor(&anchors, 1)?,
                anchor(&anchors, 2)?,
            ],
            value.value,
            value.provenance.clone(),
        )?,
        "dihedral" => Measurement::dihedral(
            owner,
            [
                anchor(&anchors, 0)?,
                anchor(&anchors, 1)?,
                anchor(&anchors, 2)?,
                anchor(&anchors, 3)?,
            ],
            value.value,
            value.provenance.clone(),
        )?,
        _ => return invalid("unknown measurement kind"),
    };
    if measurement.label() != value.label {
        return invalid("measurement label does not match its computed value");
    }
    measurement = measurement.with_priority(value.priority);
    measurement.set_visible(value.visible);
    Ok(measurement)
}

fn anchor(values: &[AnnotationAnchor], index: usize) -> Result<AnnotationAnchor, crate::CoreError> {
    values
        .get(index)
        .copied()
        .ok_or_else(|| invalid_value("measurement has too few anchors"))
}

fn rgba(value: [u8; 4]) -> Rgba8 {
    Rgba8::new(value[0], value[1], value[2], value[3])
}
