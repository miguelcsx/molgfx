//! World and entity relation insertion from contiguous `NumPy` columns.

use super::super::{PyRelationBatchHandle, PyScene};
use super::{
    PyRelationPattern, PyRowDomain,
    arrays::{ordered_rows, vec3_rows},
};
use crate::error::{core, value};
use numpy::{PyReadonlyArray1, PyReadonlyArray2};
use pyo3::prelude::*;
use std::sync::Arc;

#[derive(Clone, Copy)]
struct RelationStyleInput {
    width_pixels: f32,
    color: (u8, u8, u8, u8),
    opacity: f32,
    pattern: PyRelationPattern,
    endpoint_insets_pixels: (f32, f32),
    depth_behind_anchors: bool,
}

#[pymethods]
impl PyScene {
    #[pyo3(signature = (namespace, starts, ends, width_pixels=1.5, color=(255, 255, 255, 255), opacity=1.0, pattern=PyRelationPattern::Solid, endpoint_insets_pixels=(0.0, 0.0), depth_behind_anchors=false))]
    #[allow(clippy::too_many_arguments)]
    fn add_world_relations_from_numpy(
        &mut self,
        namespace: u64,
        starts: PyReadonlyArray2<'_, f32>,
        ends: PyReadonlyArray2<'_, f32>,
        width_pixels: f32,
        color: (u8, u8, u8, u8),
        opacity: f32,
        pattern: PyRelationPattern,
        endpoint_insets_pixels: (f32, f32),
        depth_behind_anchors: bool,
    ) -> PyResult<PyRelationBatchHandle> {
        let starts = vec3_rows("starts", &starts)?;
        let ends = vec3_rows("ends", &ends)?;
        if starts.len() != ends.len() {
            return Err(value(
                "relation endpoint columns must have the same row count",
            ));
        }
        let mut relations = Vec::with_capacity(starts.len());
        for (start, end) in starts.into_iter().zip(ends) {
            relations.push(molgfx::Relation {
                start: core(molgfx::SpatialAnchor::world(molgfx::Vec3::from_array(
                    start,
                )))?,
                end: core(molgfx::SpatialAnchor::world(molgfx::Vec3::from_array(
                    end,
                )))?,
            });
        }
        self.insert_relations(
            namespace,
            relations,
            RelationStyleInput {
                width_pixels,
                color,
                opacity,
                pattern,
                endpoint_insets_pixels,
                depth_behind_anchors,
            },
        )
    }

    #[pyo3(signature = (namespace, start_domain, start_rows, end_domain, end_rows, width_pixels=1.5, color=(255, 255, 255, 255), opacity=1.0, pattern=PyRelationPattern::Solid, endpoint_insets_pixels=(0.0, 0.0), depth_behind_anchors=false))]
    #[allow(clippy::too_many_arguments)]
    fn add_entity_relations_from_numpy(
        &mut self,
        namespace: u64,
        start_domain: PyRowDomain,
        start_rows: PyReadonlyArray1<'_, u32>,
        end_domain: PyRowDomain,
        end_rows: PyReadonlyArray1<'_, u32>,
        width_pixels: f32,
        color: (u8, u8, u8, u8),
        opacity: f32,
        pattern: PyRelationPattern,
        endpoint_insets_pixels: (f32, f32),
        depth_behind_anchors: bool,
    ) -> PyResult<PyRelationBatchHandle> {
        let start_rows = start_rows
            .as_slice()
            .map_err(|_| value("start_rows must be C-contiguous uint32"))?;
        let end_rows = end_rows
            .as_slice()
            .map_err(|_| value("end_rows must be C-contiguous uint32"))?;
        if start_rows.len() != end_rows.len() {
            return Err(value("relation row columns must have the same row count"));
        }
        let mut relations = Vec::with_capacity(start_rows.len());
        for (&start, &end) in start_rows.iter().zip(end_rows) {
            relations.push(molgfx::Relation {
                start: core(molgfx::SpatialAnchor::entity(molgfx::RowEntityRef::new(
                    start_domain.0,
                    start,
                )))?,
                end: core(molgfx::SpatialAnchor::entity(molgfx::RowEntityRef::new(
                    end_domain.0,
                    end,
                )))?,
            });
        }
        self.insert_relations(
            namespace,
            relations,
            RelationStyleInput {
                width_pixels,
                color,
                opacity,
                pattern,
                endpoint_insets_pixels,
                depth_behind_anchors,
            },
        )
    }
}

impl PyScene {
    fn insert_relations(
        &mut self,
        namespace: u64,
        relations: Vec<molgfx::Relation>,
        style: RelationStyleInput,
    ) -> PyResult<PyRelationBatchHandle> {
        let rows = ordered_rows(namespace, relations.len())?;
        let color = style.color;
        let insets = style.endpoint_insets_pixels;
        core(molgfx::RelationBatch::new(
            Arc::from(relations),
            rows,
            molgfx::RelationStyle {
                width_pixels: style.width_pixels,
                color: molgfx::Rgba8::new(color.0, color.1, color.2, color.3),
                opacity: style.opacity,
                pattern: style.pattern.into(),
                endpoint_insets_pixels: [insets.0, insets.1],
                depth_behind_anchors: style.depth_behind_anchors,
            },
        ))
        .and_then(|batch| core(self.inner.add_relation_batch(batch)))
        .map(Into::into)
    }
}
