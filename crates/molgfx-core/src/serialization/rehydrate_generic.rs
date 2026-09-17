//! Reconstruction of schema-8 generic row tables and visual descriptors.

use super::{
    GenericSceneDescriptionSources, SceneDescription, insert, invalid, raw, resolve_existing,
    resolve_raw,
};
use crate::scene::StoredAttribute;
use crate::serialization::{
    AttributeDescription, DomainVisualDescription, InstanceBatchDescription, PointBatchDescription,
    RelationBatchDescription, RowDomainDescription, generic_records,
};
use crate::{
    AttributeHandle, InstanceBatchHandle, PointBatchHandle, RelationBatchHandle, RowDomain, Scene,
    VisualDescriptor,
};

pub(super) fn rehydrate(
    scene: &mut Scene,
    description: &SceneDescription,
    sources: GenericSceneDescriptionSources<'_>,
) -> Result<(), crate::CoreError> {
    point_batches(scene, &description.point_batches, sources.point_batches)?;
    instance_batches(
        scene,
        &description.instance_batches,
        sources.instance_batches,
    )?;
    attributes(scene, &description.attributes, sources.attributes)?;
    relation_batches(
        scene,
        &description.relation_batches,
        sources.relation_batches,
    )?;
    domain_visuals(scene, &description.domain_visuals)
}

fn point_batches(
    scene: &mut Scene,
    descriptions: &[PointBatchDescription],
    sources: &[crate::PointBatch],
) -> Result<(), crate::CoreError> {
    for (description, source) in descriptions.iter().zip(sources) {
        let handle = PointBatchHandle(raw(description.row, description.generation));
        if generic_records::point_batch(handle, source) != *description {
            return invalid("supplied point batch does not match its manifest");
        }
        insert(
            scene.point_batches.insert_at(handle.0, source.clone()),
            "point batch identity collision",
        )?;
    }
    Ok(())
}

fn instance_batches(
    scene: &mut Scene,
    descriptions: &[InstanceBatchDescription],
    sources: &[crate::InstanceBatch],
) -> Result<(), crate::CoreError> {
    for (description, source) in descriptions.iter().zip(sources) {
        let handle = InstanceBatchHandle(raw(description.row, description.generation));
        if generic_records::instance_batch(handle, source) != *description {
            return invalid("supplied instance batch does not match its manifest");
        }
        insert(
            scene.instance_batches.insert_at(handle.0, source.clone()),
            "instance batch identity collision",
        )?;
    }
    Ok(())
}

fn attributes(
    scene: &mut Scene,
    descriptions: &[AttributeDescription],
    sources: &[crate::AttributeColumn],
) -> Result<(), crate::CoreError> {
    for (description, source) in descriptions.iter().zip(sources) {
        let handle = AttributeHandle(raw(description.row, description.generation));
        if generic_records::attribute(scene, handle, source) != *description {
            return invalid("supplied attribute does not match its manifest");
        }
        let end = u32::try_from(source.len())
            .map_err(|_| invalid_value("attribute row count exceeds u32"))?;
        insert(
            scene.attributes.insert_at(
                handle.0,
                StoredAttribute {
                    value: source.clone(),
                    revision: 0,
                    dirty_rows: 0..end,
                },
            ),
            "attribute identity collision",
        )?;
    }
    Ok(())
}

fn relation_batches(
    scene: &mut Scene,
    descriptions: &[RelationBatchDescription],
    sources: &[crate::RelationBatch],
) -> Result<(), crate::CoreError> {
    for (description, source) in descriptions.iter().zip(sources) {
        let handle = RelationBatchHandle(raw(description.row, description.generation));
        if generic_records::relation_batch(handle, source) != *description {
            return invalid("supplied relation batch does not match its manifest");
        }
        for dependency in source.dependencies().iter() {
            let rows = scene
                .row_count(dependency.domain)
                .ok_or(crate::CoreError::StaleHandle)?;
            if dependency.maximum_row >= rows {
                return invalid("relation anchor row is outside its live domain");
            }
        }
        insert(
            scene.relation_batches.insert_at(handle.0, source.clone()),
            "relation batch identity collision",
        )?;
    }
    Ok(())
}

fn domain_visuals(
    scene: &mut Scene,
    descriptions: &[DomainVisualDescription],
) -> Result<(), crate::CoreError> {
    for description in descriptions {
        let domain = parse_domain(scene, &description.domain)?;
        let style = super::render::parse_domain_visual(scene, &description.style, domain)?;
        let descriptor = VisualDescriptor::new(style).with_order(description.order);
        scene.set_domain_visual(domain, descriptor)?;
    }
    Ok(())
}

pub(super) fn parse_domain(
    scene: &Scene,
    value: &RowDomainDescription,
) -> Result<RowDomain, crate::CoreError> {
    let raw = resolve_raw(super::ObjectIdentity {
        row: value.row,
        generation: value.generation,
    });
    let domain = match value.kind.as_str() {
        "atoms" => {
            resolve_existing(scene.structures.get(raw))?;
            RowDomain::Atoms(crate::StructureHandle(raw))
        }
        "points" => {
            resolve_existing(scene.point_batches.get(raw))?;
            RowDomain::Points(PointBatchHandle(raw))
        }
        "instances" => {
            resolve_existing(scene.instance_batches.get(raw))?;
            RowDomain::Instances(InstanceBatchHandle(raw))
        }
        "template-parts" => {
            resolve_existing(scene.instance_batches.get(raw))?;
            RowDomain::TemplateParts(InstanceBatchHandle(raw))
        }
        "relations" => {
            resolve_existing(scene.relation_batches.get(raw))?;
            RowDomain::Relations(RelationBatchHandle(raw))
        }
        _ => return invalid("manifest references an unknown row domain"),
    };
    Ok(domain)
}

fn invalid_value(summary: &'static str) -> crate::CoreError {
    crate::CoreError::InvalidSceneDescription {
        summary: summary.into(),
    }
}
