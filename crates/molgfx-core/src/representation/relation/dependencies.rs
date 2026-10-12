//! Referenced relation domains and their maximum addressed rows.

use super::{Relation, RelationDependency, SpatialAnchor};
use crate::RowDomain;
use std::sync::Arc;

pub(super) fn relation_dependencies(
    relations: &[Relation],
    geometry: Option<&[super::OrientedRelation]>,
) -> Arc<[RelationDependency]> {
    let mut rows = std::collections::BTreeMap::<RowDomain, u32>::new();
    for anchor in relations
        .iter()
        .flat_map(|relation| [relation.start, relation.end])
    {
        match anchor {
            SpatialAnchor::World(_) => {}
            SpatialAnchor::Entity(entity) => {
                add_dependency(&mut rows, entity.domain(), entity.row());
            }
            SpatialAnchor::TemplatePart(reference) => {
                add_dependency(
                    &mut rows,
                    RowDomain::Instances(reference.batch()),
                    reference.instance_row(),
                );
                add_dependency(
                    &mut rows,
                    RowDomain::TemplateParts(reference.batch()),
                    reference.part_row(),
                );
            }
        }
    }
    if let Some(geometry) = geometry {
        for constraint in geometry {
            add_dependency(&mut rows, constraint.pivot.domain(), constraint.pivot.row());
        }
    }
    rows.into_iter()
        .map(|(domain, maximum_row)| RelationDependency {
            domain,
            maximum_row,
        })
        .collect::<Vec<_>>()
        .into()
}

fn add_dependency(
    rows: &mut std::collections::BTreeMap<RowDomain, u32>,
    domain: RowDomain,
    row: u32,
) {
    rows.entry(domain)
        .and_modify(|maximum| *maximum = (*maximum).max(row))
        .or_insert(row);
}
