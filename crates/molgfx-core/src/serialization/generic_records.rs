//! Content-addressed schema-8 records for generic immutable tables.

use super::generic_description::{
    AttributeDescription, DomainVisualDescription, InstanceBatchDescription, PointBatchDescription,
    RelationBatchDescription, RowDomainDescription, SourceRowsDescription,
};
use super::manifest::visual_description;
use super::manifest_io::{ContentHasher, PayloadReference, ReferencedPayloadKind};
use crate::{
    AttributeColumn, AttributeKind, InstanceBatch, PointBatch, PointGlyph, RelationBatch,
    RelationPattern, RowDomain, Scene, SourceRows, SpatialAnchor,
};

pub(crate) fn point_batches(scene: &Scene) -> Vec<PointBatchDescription> {
    scene
        .point_batches()
        .map(|(handle, batch)| point_batch(handle, batch))
        .collect()
}

pub(crate) fn point_batch(
    handle: crate::PointBatchHandle,
    batch: &PointBatch,
) -> PointBatchDescription {
    PointBatchDescription {
        row: handle.row(),
        generation: handle.generation(),
        source_rows: source_rows(batch.source_rows()),
        glyph: match batch.glyph() {
            PointGlyph::Disc => "disc",
            PointGlyph::Sphere => "sphere",
        }
        .into(),
        radius_bits: batch.style().radius.to_bits(),
        color: rgba(batch.style().color),
        visible: batch.visible(),
        payload: point_payload(handle, batch),
    }
}

pub(crate) fn instance_batches(scene: &Scene) -> Vec<InstanceBatchDescription> {
    scene
        .instance_batches()
        .map(|(handle, batch)| instance_batch(handle, batch))
        .collect()
}

pub(crate) fn instance_batch(
    handle: crate::InstanceBatchHandle,
    batch: &InstanceBatch,
) -> InstanceBatchDescription {
    InstanceBatchDescription {
        row: handle.row(),
        generation: handle.generation(),
        source_rows: source_rows(batch.source_rows()),
        template_rows: source_rows(batch.template().source_rows()),
        sphere_count: count(batch.template().spheres().len()),
        capsule_count: count(batch.template().capsules().len()),
        color: rgba(batch.style().color),
        visible: batch.visible(),
        payload: instance_payload(handle, batch),
    }
}

pub(crate) fn attributes(scene: &Scene) -> Vec<AttributeDescription> {
    scene
        .attributes()
        .map(|(handle, value)| attribute(scene, handle, value))
        .collect()
}

pub(crate) fn attribute(
    scene: &Scene,
    handle: crate::AttributeHandle,
    value: &AttributeColumn,
) -> AttributeDescription {
    AttributeDescription {
        row: handle.row(),
        generation: handle.generation(),
        domain: row_domain(value.domain()),
        name: value.name().into(),
        quantity: value.descriptor().quantity().map(Into::into),
        unit: value.descriptor().unit().map(Into::into),
        provenance: value.descriptor().provenance().map(Into::into),
        kind: attribute_kind(value.kind()).into(),
        row_count: count(value.len()),
        fingerprint: value.fingerprint(),
        payload: attribute_payload(scene, handle, value),
    }
}

pub(crate) fn relation_batches(scene: &Scene) -> Vec<RelationBatchDescription> {
    scene
        .relation_batches()
        .map(|(handle, batch)| relation_batch(handle, batch))
        .collect()
}

pub(crate) fn relation_batch(
    handle: crate::RelationBatchHandle,
    batch: &RelationBatch,
) -> RelationBatchDescription {
    let style = batch.style();
    RelationBatchDescription {
        row: handle.row(),
        generation: handle.generation(),
        source_rows: source_rows(batch.source_rows()),
        width_bits: style.width_pixels.to_bits(),
        color: rgba(style.color),
        opacity_bits: style.opacity.to_bits(),
        endpoint_inset_bits: style.endpoint_insets_pixels.map(f32::to_bits),
        depth_behind_anchors: style.depth_behind_anchors,
        pattern: match style.pattern {
            RelationPattern::Solid => "solid",
            RelationPattern::Dashed => "dashed",
            RelationPattern::Dotted => "dotted",
            RelationPattern::Spring => "spring",
        }
        .into(),
        visible: batch.visible(),
        payload: relation_payload(handle, batch),
    }
}

pub(crate) fn domain_visuals(scene: &Scene) -> Vec<DomainVisualDescription> {
    scene
        .domain_visuals()
        .map(|(domain, descriptor)| DomainVisualDescription {
            domain: row_domain(domain),
            order: descriptor.order(),
            style: visual_description(descriptor.style()),
        })
        .collect()
}

pub(crate) fn row_domain(domain: RowDomain) -> RowDomainDescription {
    let (kind, row, generation) = match domain {
        RowDomain::Atoms(handle) => ("atoms", handle.row(), handle.generation()),
        RowDomain::Points(handle) => ("points", handle.row(), handle.generation()),
        RowDomain::Instances(handle) => ("instances", handle.row(), handle.generation()),
        RowDomain::TemplateParts(handle) => ("template-parts", handle.row(), handle.generation()),
        RowDomain::Relations(handle) => ("relations", handle.row(), handle.generation()),
    };
    RowDomainDescription {
        kind: kind.into(),
        row,
        generation,
    }
}

fn point_payload(handle: crate::PointBatchHandle, batch: &PointBatch) -> PayloadReference {
    let mut hash = PayloadHash::new();
    hash.source_rows(batch.source_rows());
    hash.bytes(bytemuck::cast_slice(batch.positions().as_ref()));
    hash.finish(
        ReferencedPayloadKind::Points,
        batch.source_rows().namespace().0,
        table_chunk(handle.row(), handle.generation()),
    )
}

fn instance_payload(handle: crate::InstanceBatchHandle, batch: &InstanceBatch) -> PayloadReference {
    let mut hash = PayloadHash::new();
    hash.source_rows(batch.source_rows());
    hash.source_rows(batch.template().source_rows());
    hash.bytes(bytemuck::cast_slice(batch.template().spheres().as_ref()));
    hash.bytes(bytemuck::cast_slice(batch.template().capsules().as_ref()));
    hash.bytes(bytemuck::cast_slice(batch.transforms().as_ref()));
    hash.finish(
        ReferencedPayloadKind::Instances,
        batch.source_rows().namespace().0,
        table_chunk(handle.row(), handle.generation()),
    )
}

fn attribute_payload(
    scene: &Scene,
    handle: crate::AttributeHandle,
    attribute: &AttributeColumn,
) -> PayloadReference {
    let mut hash = PayloadHash::new();
    hash.bytes(attribute.values().as_bytes());
    hash.finish(
        ReferencedPayloadKind::Attribute,
        domain_namespace(scene, attribute.domain()),
        table_chunk(handle.row(), handle.generation()),
    )
}

fn relation_payload(handle: crate::RelationBatchHandle, batch: &RelationBatch) -> PayloadReference {
    let mut hash = PayloadHash::new();
    hash.source_rows(batch.source_rows());
    for relation in batch.relations().iter() {
        hash.anchor(relation.start);
        hash.anchor(relation.end);
    }
    hash.finish(
        ReferencedPayloadKind::Relations,
        batch.source_rows().namespace().0,
        table_chunk(handle.row(), handle.generation()),
    )
}

fn source_rows(rows: &SourceRows) -> SourceRowsDescription {
    SourceRowsDescription {
        namespace: rows.namespace().0,
        row_count: rows.len(),
        keyed: rows.keys().is_some(),
    }
}

struct PayloadHash {
    hasher: ContentHasher,
    byte_len: u64,
}

impl PayloadHash {
    fn new() -> Self {
        Self {
            hasher: ContentHasher::new(),
            byte_len: 0,
        }
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.hasher.update(bytes);
        self.byte_len = self.byte_len.saturating_add(bytes.len() as u64);
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn source_rows(&mut self, rows: &SourceRows) {
        self.u64(rows.namespace().0);
        self.u32(rows.len());
        self.u32(u32::from(rows.keys().is_some()));
        if let Some(keys) = rows.keys() {
            self.bytes(bytemuck::cast_slice(keys.as_ref()));
        }
    }

    fn anchor(&mut self, anchor: SpatialAnchor) {
        match anchor {
            SpatialAnchor::World(value) => {
                self.u32(0);
                self.bytes(bytemuck::cast_slice(&[value.to_array()]));
            }
            SpatialAnchor::Entity(entity) => {
                self.u32(1);
                self.domain(entity.domain());
                self.u32(entity.row());
            }
            SpatialAnchor::TemplatePart(reference) => {
                self.u32(2);
                self.u32(reference.batch().row());
                self.u32(reference.batch().generation());
                self.u32(reference.instance_row());
                self.u32(reference.part_row());
            }
        }
    }

    fn domain(&mut self, domain: RowDomain) {
        let value = row_domain(domain);
        self.bytes(value.kind.as_bytes());
        self.u32(value.row);
        self.u32(value.generation);
    }

    fn finish(self, kind: ReferencedPayloadKind, dataset: u64, chunk: u64) -> PayloadReference {
        PayloadReference {
            kind,
            dataset,
            chunk,
            byte_len: self.byte_len,
            address: self.hasher.finish(),
        }
    }
}

fn domain_namespace(scene: &Scene, domain: RowDomain) -> u64 {
    match domain {
        RowDomain::Atoms(handle) => scene
            .structure(handle)
            .map_or(0, |value| value.dataset_id().get()),
        RowDomain::Points(handle) => scene
            .point_batch(handle)
            .map_or(0, |value| value.source_rows().namespace().0),
        RowDomain::Instances(handle) => scene
            .instance_batch(handle)
            .map_or(0, |value| value.source_rows().namespace().0),
        RowDomain::TemplateParts(handle) => scene
            .instance_batch(handle)
            .map_or(0, |value| value.template().source_rows().namespace().0),
        RowDomain::Relations(handle) => scene
            .relation_batch(handle)
            .map_or(0, |value| value.source_rows().namespace().0),
    }
}

const fn attribute_kind(kind: AttributeKind) -> &'static str {
    match kind {
        AttributeKind::Scalar => "scalar",
        AttributeKind::Category => "category",
        AttributeKind::Vector => "vector",
        AttributeKind::Color => "color",
    }
}

const fn rgba(color: molgfx_math::Rgba8) -> [u8; 4] {
    [color.r, color.g, color.b, color.a]
}

fn count(value: usize) -> u32 {
    u32::try_from(value)
        .into_iter()
        .fold(u32::MAX, |_, value| value)
}

const fn table_chunk(row: u32, generation: u32) -> u64 {
    row as u64 | ((generation as u64) << 32)
}
