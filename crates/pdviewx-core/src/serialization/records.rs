//! Conversion of scene-owned records into deterministic manifest payloads.
use super::types::{
    AnchorDescription, AnnotationDescription, AtomPropertyDescription, EntityDescription,
    GuideDescription, GuideStyleDescription, InteractionDescription, MarkerStyleDescription,
    MeasurementDescription, MeshDescription, MeshInstanceDescription, ObjectIdentity,
    OverlayDescription, PrimitiveDescription, PropertyAppearanceDescription, RegionDescription,
    ScalarSemanticsDescription, SegmentStyleDescription, SegmentationStyleDescription,
    SurfaceScalarDescription, VolumeStyleDescription, VolumeTransferPointDescription,
};
use crate::handle::StructureHandle;
use crate::{
    AnnotationKind, AtomPropertyMeaning, EntityKind, Guide, GuideCap, InteractionAnchor,
    InteractionDirection, InteractionKind, MarkerShape, MeasurementKind, ScalarFieldSemantics,
    Scene,
};
use pdviewx_math::Rgba8;
#[path = "hash.rs"]
mod hash;
#[path = "primitive_records.rs"]
mod primitive_records;
#[path = "record_values.rs"]
mod record_values;
pub(crate) use hash::{label_hash, mesh_hash, value_hash};
use primitive_records::primitive_description;
use record_values::volume_rendering;
pub(crate) fn primitives(scene: &Scene) -> Vec<PrimitiveDescription> {
    scene
        .primitives()
        .map(|(handle, value)| primitive_description(handle.0, value))
        .collect()
}
pub(crate) fn meshes(scene: &Scene) -> Vec<MeshDescription> {
    scene
        .meshes()
        .map(|(handle, mesh)| {
            let policy = mesh.component_policy();
            MeshDescription {
                row: handle.row(),
                generation: handle.generation(),
                owner: identity(mesh.owner()),
                content_hash: mesh_hash(mesh),
                material: super::manifest::material_description(mesh.material()),
                clipping: super::manifest::clip_description(mesh.clipping()),
                face_visibility: match mesh.face_visibility() {
                    crate::FaceVisibility::DoubleSided => "double",
                    crate::FaceVisibility::FrontOnly => "front",
                    crate::FaceVisibility::BackOnly => "back",
                }
                .to_owned(),
                minimum_component_area: match policy.threshold() {
                    crate::SurfaceComponentThreshold::Area(value) => value,
                    crate::SurfaceComponentThreshold::Disabled
                    | crate::SurfaceComponentThreshold::Volume(_)
                    | crate::SurfaceComponentThreshold::Voxels(_) => 0.0,
                },
                maximum_components: policy
                    .maximum_components()
                    .and_then(|value| usize::try_from(value).ok()),
                visible: mesh.visible(),
            }
        })
        .collect()
}
pub(crate) fn mesh_instances(scene: &Scene) -> Vec<MeshInstanceDescription> {
    scene
        .mesh_instances()
        .map(|(handle, instance)| MeshInstanceDescription {
            row: handle.row(),
            generation: handle.generation(),
            mesh: ObjectIdentity {
                row: instance.mesh().row(),
                generation: instance.mesh().generation(),
            },
            transform: instance.transform().to_cols_array(),
            visible: instance.visible(),
        })
        .collect()
}
pub(crate) fn overlays(scene: &Scene) -> Vec<OverlayDescription> {
    scene
        .overlays()
        .map(|(handle, overlay)| {
            let anchor = overlay.anchor();
            let (kind, text, colors, values) = match overlay.content() {
                crate::OverlayContent::Text {
                    text,
                    color,
                    size_pixels,
                } => (
                    "text",
                    text.clone(),
                    [rgba(*color), [0; 4]],
                    [*size_pixels, 0.0, 0.0, 0.0],
                ),
                crate::OverlayContent::ColorLegend {
                    title,
                    range,
                    colors,
                    size_pixels,
                } => (
                    "color-legend",
                    title.clone(),
                    [rgba(colors[0]), rgba(colors[1])],
                    [range[0], range[1], size_pixels[0], size_pixels[1]],
                ),
                crate::OverlayContent::ScaleBar {
                    length_angstrom,
                    color,
                    width_pixels,
                } => (
                    "scale-bar",
                    String::new(),
                    [rgba(*color), [0; 4]],
                    [*length_angstrom, *width_pixels, 0.0, 0.0],
                ),
                crate::OverlayContent::CoordinateTripod {
                    size_pixels,
                    width_pixels,
                } => (
                    "coordinate-tripod",
                    String::new(),
                    [[0; 4]; 2],
                    [*size_pixels, *width_pixels, 0.0, 0.0],
                ),
            };
            OverlayDescription {
                row: handle.row(),
                generation: handle.generation(),
                kind: kind.to_owned(),
                normalized: anchor.normalized,
                pixels: anchor.pixels,
                order: overlay.order(),
                visible: overlay.visible(),
                text,
                colors,
                values,
            }
        })
        .collect()
}
pub(crate) fn atom_properties(scene: &Scene) -> Vec<AtomPropertyDescription> {
    scene
        .atom_properties()
        .map(|(handle, property)| AtomPropertyDescription {
            row: handle.row(),
            generation: handle.generation(),
            owner: ObjectIdentity {
                row: property.owner().row(),
                generation: property.owner().generation(),
            },
            name: property.name().to_owned(),
            length: property.values().len() as u64,
            finite_domain: property.finite_domain(),
            meaning: property_meaning(property.meaning()).to_owned(),
            semantics: scalar_semantics(property.semantics()),
            content_hash: value_hash(property.values()),
        })
        .collect()
}

pub(crate) fn appearance_description(
    value: crate::PropertyAppearance,
) -> PropertyAppearanceDescription {
    let (domain, opacity, softness_pixels, missing) = value.description_values();
    PropertyAppearanceDescription {
        property: ObjectIdentity {
            row: value.property.row(),
            generation: value.property.generation(),
        },
        domain,
        opacity,
        softness_pixels,
        missing,
    }
}

pub(crate) fn volume_style_description(value: crate::VolumeStyle) -> VolumeStyleDescription {
    VolumeStyleDescription {
        rendering: volume_rendering(value.rendering).to_owned(),
        transfer: value
            .transfer
            .points()
            .iter()
            .map(|point| VolumeTransferPointDescription {
                value: point.value,
                color: rgba(point.color),
                opacity: point.opacity,
            })
            .collect(),
        opacity_scale: value.opacity_scale,
        step_scale: value.step_scale,
        slice: value.slice.map(|slice| {
            [
                slice.plane.normal.x,
                slice.plane.normal.y,
                slice.plane.normal.z,
                slice.plane.offset,
            ]
        }),
        region: value.region.map(region_description),
    }
}

pub(crate) fn segmentation_style_description(
    value: &crate::SegmentationStyle,
) -> SegmentationStyleDescription {
    SegmentationStyleDescription {
        styles: value
            .styles
            .styles()
            .iter()
            .map(|style| SegmentStyleDescription {
                label: style.label,
                color: rgba(style.color),
                opacity: style.opacity,
            })
            .collect(),
        opacity_scale: value.opacity_scale,
        step_scale: value.step_scale,
        slice: value.slice.map(|slice| {
            [
                slice.plane.normal.x,
                slice.plane.normal.y,
                slice.plane.normal.z,
                slice.plane.offset,
            ]
        }),
        region: value.region.map(region_description),
    }
}

pub(crate) fn surface_scalar_description(
    value: crate::SurfaceScalarOverlay,
) -> SurfaceScalarDescription {
    SurfaceScalarDescription {
        field: ObjectIdentity {
            row: value.field.row(),
            generation: value.field.generation(),
        },
        ramp_values: value.ramp.values().map(f32::to_bits),
        ramp_colors: value.ramp.colors().map(rgba),
        contours: value
            .contours
            .map(|contours| [contours.interval, contours.width_pixels]),
        sample_offset_angstrom: value.sample_offset_angstrom,
    }
}

fn region_description(value: crate::VolumeRegion) -> RegionDescription {
    RegionDescription {
        minimum: value.minimum(),
        maximum: value.maximum(),
    }
}

pub(crate) fn guides(scene: &Scene) -> Vec<GuideDescription> {
    scene
        .guides()
        .map(|(handle, guide)| GuideDescription {
            row: handle.0.row(),
            generation: handle.0.generation(),
            owner: identity(guide.owner()),
            start: guide.start().to_array(),
            end: guide.end().to_array(),
            style: guide_style(guide),
            visible: guide.visible(),
        })
        .collect()
}

pub(crate) fn interactions(scene: &Scene) -> Vec<InteractionDescription> {
    scene
        .interactions()
        .map(|(handle, edge)| InteractionDescription {
            row: handle.0.row(),
            generation: handle.0.generation(),
            owner: identity(edge.owner()),
            start: interaction_anchor(edge.start()),
            end: interaction_anchor(edge.end()),
            kind: interaction_kind(edge.kind()).to_owned(),
            direction: interaction_direction(edge.direction()).to_owned(),
            distance_angstrom: edge.geometry().distance_angstrom(),
            angle_degrees: edge.geometry().angle_degrees(),
            occupancy: edge.occupancy(),
            normalized_strength: edge.normalized_strength(),
            phase_speed_pixels_per_frame: edge.phase_speed_pixels_per_frame(),
            persistence_age_frames: edge.persistence_age_frames(),
            persistence_half_life_frames: edge.persistence_half_life_frames(),
            provenance: edge.provenance().to_owned(),
            visible: edge.visible(),
        })
        .collect()
}

pub(crate) fn annotations(scene: &Scene) -> Vec<AnnotationDescription> {
    scene
        .annotations()
        .map(|(handle, annotation)| AnnotationDescription {
            row: handle.0.row(),
            generation: handle.0.generation(),
            owner: identity(annotation.owner()),
            kind: annotation_kind(annotation.kind()).to_owned(),
            anchor: annotation.anchor().map(annotation_anchor),
            region: annotation.region_selection().map(|handle| ObjectIdentity {
                row: handle.row(),
                generation: handle.generation(),
            }),
            text: annotation.text().to_owned(),
            marker: marker_style(annotation.marker_style()),
            priority: annotation.priority(),
            visible: annotation.is_visible(),
        })
        .collect()
}

pub(crate) fn measurements(scene: &Scene) -> Vec<MeasurementDescription> {
    scene
        .measurements()
        .map(|(handle, measurement)| MeasurementDescription {
            row: handle.0.row(),
            generation: handle.0.generation(),
            owner: identity(measurement.owner()),
            kind: measurement_kind(measurement.kind()).to_owned(),
            anchors: measurement
                .anchors()
                .iter()
                .copied()
                .map(annotation_anchor)
                .collect(),
            value: measurement.value(),
            label: measurement.label().to_owned(),
            provenance: measurement.provenance().to_owned(),
            priority: measurement.priority(),
            visible: measurement.is_visible(),
        })
        .collect()
}

fn guide_style(guide: &Guide) -> GuideStyleDescription {
    let style = guide.style();
    GuideStyleDescription {
        color: rgba(style.color),
        pattern: relation_pattern(style.pattern).to_owned(),
        width_pixels: style.width_pixels,
        opacity: style.opacity,
        period_pixels: style.period_pixels,
        duty_cycle: style.duty_cycle,
        cap: cap(style.cap).to_owned(),
        arrow_pixels: style.arrow_pixels,
    }
}

fn marker_style(style: crate::MarkerStyle) -> MarkerStyleDescription {
    MarkerStyleDescription {
        color: rgba(style.color),
        radius_pixels: style.radius_pixels,
        shape: marker_shape(style.shape).to_owned(),
    }
}

fn interaction_anchor(anchor: InteractionAnchor) -> AnchorDescription {
    AnchorDescription {
        position: anchor.position().to_array(),
        entity: anchor.source_entity().map(entity),
    }
}

fn annotation_anchor(anchor: crate::AnnotationAnchor) -> AnchorDescription {
    AnchorDescription {
        position: anchor.position().to_array(),
        entity: anchor.source_entity().map(entity),
    }
}

fn entity(value: crate::EntityRef) -> EntityDescription {
    EntityDescription {
        structure: identity(value.structure),
        kind: entity_kind(value.kind).to_owned(),
        index: value.index,
    }
}

pub(crate) fn identity(handle: StructureHandle) -> ObjectIdentity {
    ObjectIdentity {
        row: handle.0.row(),
        generation: handle.0.generation(),
    }
}

pub(crate) fn rgba(color: Rgba8) -> [u8; 4] {
    [color.r, color.g, color.b, color.a]
}

fn entity_kind(kind: EntityKind) -> &'static str {
    match kind {
        EntityKind::Atom => "atom",
        EntityKind::Bond => "bond",
        EntityKind::Edge => "edge",
        EntityKind::Label => "label",
        EntityKind::Primitive => "primitive",
        EntityKind::Mesh => "mesh",
        EntityKind::LigandPoseBatch => "ligand_pose_batch",
        EntityKind::Guide => "guide",
        EntityKind::DynamicBond => "dynamic_bond",
        EntityKind::Point => "point",
        EntityKind::Instance => "instance",
        EntityKind::TemplatePart => "template_part",
        EntityKind::Relation => "relation",
    }
}

fn interaction_kind(kind: InteractionKind) -> &'static str {
    match kind {
        InteractionKind::HydrogenBond => "hydrogen_bond",
        InteractionKind::SaltBridge => "salt_bridge",
        InteractionKind::PiStacking => "pi_stacking",
        InteractionKind::Hydrophobic => "hydrophobic",
        InteractionKind::MetalCoordination => "metal_coordination",
    }
}

fn interaction_direction(direction: InteractionDirection) -> &'static str {
    match direction {
        InteractionDirection::Undirected => "undirected",
        InteractionDirection::Forward => "forward",
        InteractionDirection::Reverse => "reverse",
    }
}

fn relation_pattern(pattern: crate::RelationPattern) -> &'static str {
    match pattern {
        crate::RelationPattern::Solid => "solid",
        crate::RelationPattern::Dashed => "dashes",
        crate::RelationPattern::Dotted => "dots",
        crate::RelationPattern::Spring => "spring",
    }
}

fn cap(cap: GuideCap) -> &'static str {
    match cap {
        GuideCap::None => "none",
        GuideCap::Arrow => "arrow",
        GuideCap::DoubleArrow => "double_arrow",
    }
}

fn marker_shape(shape: MarkerShape) -> &'static str {
    match shape {
        MarkerShape::Circle => "circle",
        MarkerShape::Diamond => "diamond",
        MarkerShape::Crosshair => "crosshair",
    }
}

fn annotation_kind(kind: AnnotationKind) -> &'static str {
    match kind {
        AnnotationKind::Note => "note",
        AnnotationKind::Marker => "marker",
        AnnotationKind::Region => "region",
        AnnotationKind::Hypothesis => "hypothesis",
    }
}

fn measurement_kind(kind: MeasurementKind) -> &'static str {
    match kind {
        MeasurementKind::Distance => "distance",
        MeasurementKind::Angle => "angle",
        MeasurementKind::Dihedral => "dihedral",
    }
}

pub(crate) fn property_meaning(value: AtomPropertyMeaning) -> &'static str {
    match value {
        AtomPropertyMeaning::Generic => "generic",
        AtomPropertyMeaning::Confidence => "confidence",
        AtomPropertyMeaning::Occupancy => "occupancy",
        AtomPropertyMeaning::LocalResolution => "local_resolution",
        AtomPropertyMeaning::Flexibility => "flexibility",
        AtomPropertyMeaning::Charge => "charge",
        AtomPropertyMeaning::Hydrophobicity => "hydrophobicity",
        AtomPropertyMeaning::Exposure => "exposure",
    }
}

include!("records_scalar.rs");
