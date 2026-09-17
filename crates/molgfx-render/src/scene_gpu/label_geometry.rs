//! Deterministic analytic measurement-guide generation.

use super::label_types::{line_record, LabelGpu};
use molgfx_core::{AnnotationAnchor, Measurement, MeasurementKind};
use molgfx_math::{Rgba8, Vec3};

const GUIDE_SEGMENTS: u16 = 20;

#[derive(Clone, Copy)]
struct ArcStyle {
    color: Rgba8,
    entity: u32,
    structure: u32,
}

pub(super) fn measurement_guides(
    output: &mut Vec<LabelGpu>,
    value: &Measurement,
    entity: u32,
    structure: u32,
) -> Vec3 {
    let anchors = value.anchors();
    let color = Rgba8::opaque(120, 211, 225);
    match value.kind() {
        MeasurementKind::Distance => {
            output.push(line_record(
                anchors[0].position(),
                anchors[1].position(),
                1.4,
                color,
                entity,
                structure,
            ));
            anchors[0].position().lerp(anchors[1].position(), 0.5)
        }
        MeasurementKind::Angle => angle_guides(output, anchors, color, entity, structure),
        MeasurementKind::Dihedral => dihedral_guides(output, anchors, color, entity, structure),
    }
}

fn angle_guides(
    output: &mut Vec<LabelGpu>,
    anchors: &[AnnotationAnchor],
    color: Rgba8,
    entity: u32,
    structure: u32,
) -> Vec3 {
    let center = anchors[1].position();
    let left = anchors[0].position() - center;
    let right = anchors[2].position() - center;
    let radius = left.length().min(right.length()) * 0.34;
    let from = left.normalize_or_zero();
    let to = right.normalize_or_zero();
    let axis = from.cross(to).normalize_or_zero();
    let angle = from.dot(to).clamp(-1.0, 1.0).acos();
    arc(
        output,
        center,
        from,
        axis,
        angle,
        radius,
        ArcStyle {
            color,
            entity,
            structure,
        },
    )
}

fn dihedral_guides(
    output: &mut Vec<LabelGpu>,
    anchors: &[AnnotationAnchor],
    color: Rgba8,
    entity: u32,
    structure: u32,
) -> Vec3 {
    let [a, b, c, d] = [
        anchors[0].position(),
        anchors[1].position(),
        anchors[2].position(),
        anchors[3].position(),
    ];
    for (start, end) in [(a, b), (b, c), (c, d)] {
        output.push(line_record(start, end, 1.0, color, entity, structure));
    }
    let axis = (c - b).normalize_or_zero();
    let from = (a - b).reject_from_normalized(axis).normalize_or_zero();
    let to = (d - c).reject_from_normalized(axis).normalize_or_zero();
    let angle = axis.dot(from.cross(to)).atan2(from.dot(to));
    let center = b.lerp(c, 0.5);
    let radius = (a.distance(b).min(d.distance(c)) * 0.32).max(0.2);
    arc(
        output,
        center,
        from,
        axis,
        angle,
        radius,
        ArcStyle {
            color,
            entity,
            structure,
        },
    )
}

fn arc(
    output: &mut Vec<LabelGpu>,
    center: Vec3,
    from: Vec3,
    axis: Vec3,
    angle: f32,
    radius: f32,
    style: ArcStyle,
) -> Vec3 {
    let mut previous = center + from * radius;
    let mut midpoint = previous;
    for step in 1..=GUIDE_SEGMENTS {
        let fraction = f32::from(step) / f32::from(GUIDE_SEGMENTS);
        let current = center + rotate_axis(from, axis, angle * fraction) * radius;
        output.push(line_record(
            previous,
            current,
            1.3,
            style.color,
            style.entity,
            style.structure,
        ));
        if step == GUIDE_SEGMENTS / 2 {
            midpoint = current;
        }
        previous = current;
    }
    midpoint
}

fn rotate_axis(vector: Vec3, axis: Vec3, angle: f32) -> Vec3 {
    if axis.length_squared() < 0.5 {
        return vector;
    }
    let cosine = angle.cos();
    vector * cosine + axis.cross(vector) * angle.sin() + axis * axis.dot(vector) * (1.0 - cosine)
}
