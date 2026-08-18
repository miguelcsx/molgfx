//! Edit-time packing of semantic labels into fixed-schema GPU columns.

use super::label_geometry::measurement_guides;
use super::label_types::{
    GLYPH_ADVANCE, GLYPH_HEIGHT, GLYPH_WIDTH, LabelGpu, LabelHeaderGpu, glyph_bits, glyph_record,
    marker_record,
};
use super::structure::GpuStructure;
use pdviewx_core::{
    Annotation, AnnotationHandle, AnnotationKind, EntityId, EntityKind, Measurement,
    MeasurementHandle, Scene,
};
use pdviewx_gpu::Device;
use pdviewx_math::{Rgba8, Vec3};

#[derive(Clone, Copy)]
enum Source<'a> {
    Annotation(AnnotationHandle, &'a Annotation),
    Measurement(MeasurementHandle, &'a Measurement),
}

impl Source<'_> {
    fn priority(self) -> i16 {
        match self {
            Self::Annotation(_, value) => value.priority(),
            Self::Measurement(_, value) => value.priority(),
        }
    }

    fn row(self) -> u32 {
        match self {
            Self::Annotation(handle, _) => Scene::annotation_row(handle),
            Self::Measurement(handle, _) => Scene::measurement_row(handle),
        }
    }
}

pub(super) fn pack_labels<D: Device>(
    scene: &Scene,
    structures: &[GpuStructure<D>],
    headers: &mut Vec<LabelHeaderGpu>,
    records: &mut Vec<LabelGpu>,
) {
    headers.clear();
    records.clear();
    let mut sources = scene
        .annotations()
        .filter(|(_, value)| value.is_visible())
        .map(|(handle, value)| Source::Annotation(handle, value))
        .chain(
            scene
                .measurements()
                .filter(|(_, value)| value.is_visible())
                .map(|(handle, value)| Source::Measurement(handle, value)),
        )
        .collect::<Vec<_>>();
    sources.sort_by_key(|source| (std::cmp::Reverse(source.priority()), source.row()));
    for source in sources {
        let owner = match source {
            Source::Annotation(_, value) => value.owner(),
            Source::Measurement(_, value) => value.owner(),
        };
        let Some(structure_id) = structures
            .iter()
            .find(|structure| structure.handle == owner)
            .map(GpuStructure::structure_id)
        else {
            continue;
        };
        match source {
            Source::Annotation(handle, value) => {
                pack_annotation(scene, handle, value, structure_id, headers, records);
            }
            Source::Measurement(handle, value) => {
                pack_measurement(handle, value, structure_id, headers, records);
            }
        }
    }
}

fn pack_annotation(
    scene: &Scene,
    handle: AnnotationHandle,
    value: &Annotation,
    structure_id: u32,
    headers: &mut Vec<LabelHeaderGpu>,
    records: &mut Vec<LabelGpu>,
) {
    let anchor = value
        .anchor()
        .map(pdviewx_core::AnnotationAnchor::position)
        .or_else(|| {
            value
                .region_selection()
                .and_then(|selection| selection_centroid(scene, value.owner(), selection))
        });
    let Some(anchor) = anchor else {
        return;
    };
    let entity = EntityId::pack(EntityKind::Label, Scene::annotation_row(handle)).0;
    let first = count(records.len());
    match value.kind() {
        AnnotationKind::Marker => records.push(marker_record(
            anchor,
            value.marker_style().radius_pixels,
            value.marker_style().shape,
            value.marker_style().color,
            entity,
            structure_id,
        )),
        AnnotationKind::Note | AnnotationKind::Region | AnnotationKind::Hypothesis => {
            let color = match value.kind() {
                AnnotationKind::Hypothesis => Rgba8::opaque(255, 190, 72),
                AnnotationKind::Note | AnnotationKind::Region => Rgba8::opaque(238, 244, 246),
                AnnotationKind::Marker => value.marker_style().color,
            };
            let bounds = pack_text(
                records,
                anchor,
                value.text(),
                color,
                entity,
                structure_id,
                [0.0, 0.0],
            );
            push_header(headers, records.len(), anchor, first, bounds);
        }
    }
    if value.kind() == AnnotationKind::Marker {
        let radius = value.marker_style().radius_pixels;
        push_header(
            headers,
            records.len(),
            anchor,
            first,
            [-radius, -radius, radius, radius],
        );
    }
}

fn pack_measurement(
    handle: MeasurementHandle,
    value: &Measurement,
    structure_id: u32,
    headers: &mut Vec<LabelHeaderGpu>,
    records: &mut Vec<LabelGpu>,
) {
    let entity = EntityId::pack(EntityKind::Label, Scene::measurement_row(handle)).0;
    let first = count(records.len());
    let anchor = measurement_guides(records, value, entity, structure_id);
    let bounds = pack_text(
        records,
        anchor,
        value.label(),
        Rgba8::opaque(183, 232, 239),
        entity,
        structure_id,
        [0.0, -12.0],
    );
    push_header(headers, records.len(), anchor, first, bounds);
}

fn pack_text(
    records: &mut Vec<LabelGpu>,
    anchor: Vec3,
    text: &str,
    color: Rgba8,
    entity: u32,
    structure_id: u32,
    offset: [f32; 2],
) -> [f32; 4] {
    let width = GLYPH_ADVANCE * glyph_count_f32(text.chars().count());
    for (index, glyph) in text.chars().enumerate() {
        let glyph_offset = [
            offset[0] + GLYPH_ADVANCE * glyph_count_f32(index) - width * 0.5 + GLYPH_WIDTH * 0.5,
            offset[1],
        ];
        records.push(glyph_record(
            anchor,
            glyph_offset,
            glyph_bits(glyph),
            color,
            entity,
            structure_id,
        ));
    }
    [
        offset[0] - width * 0.5,
        offset[1] - GLYPH_HEIGHT * 0.5,
        offset[0] + width * 0.5,
        offset[1] + GLYPH_HEIGHT * 0.5,
    ]
}

fn push_header(
    headers: &mut Vec<LabelHeaderGpu>,
    record_len: usize,
    anchor: Vec3,
    first: u32,
    bounds: [f32; 4],
) {
    headers.push(LabelHeaderGpu {
        anchor_width: anchor
            .extend((bounds[2] - bounds[0]).max(GLYPH_WIDTH))
            .to_array(),
        bounds,
        range: [first, count(record_len).saturating_sub(first), 0, 0],
    });
}

fn selection_centroid(
    scene: &Scene,
    owner: pdviewx_core::StructureHandle,
    selection: pdviewx_core::SelectionHandle,
) -> Option<Vec3> {
    let placed = scene.structure(owner)?;
    let selection = scene.selection_for(selection, owner)?;
    let mut sum = Vec3::ZERO;
    let mut selected_count = 0u32;
    selection.for_each(placed.atoms.len(), |index| {
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let Some(position) = placed.atoms.coords().slice().get(index) else {
            return;
        };
        sum += placed
            .model_to_world
            .transform_point3(Vec3::from_array(*position));
        selected_count = selected_count.saturating_add(1);
    });
    (selected_count > 0).then_some(sum / count_f32(selected_count))
}

fn glyph_count_f32(value: usize) -> f32 {
    f32::from(u16::try_from(value).map_or(u16::MAX, |value| value))
}

fn count_f32(value: u32) -> f32 {
    f32::from(u16::try_from(value).map_or(u16::MAX, |value| value))
}

fn count(value: usize) -> u32 {
    u32::try_from(value).map_or(u32::MAX, |value| value)
}
