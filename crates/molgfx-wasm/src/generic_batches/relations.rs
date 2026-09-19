//! Homogeneous world and entity relation insertion.

use super::{
    WebRelationBatchHandle, WebRowDomain,
    values::{core_error, js_value, ordered_rows, rgba8, vec3_rows},
};
use crate::browser::WebScene;
use std::sync::Arc;
use wasm_bindgen::prelude::*;

#[derive(Clone, Copy)]
struct RelationStyleInput<'a> {
    width_pixels: f32,
    rgba: &'a [u8],
    opacity: f32,
    endpoint_insets_pixels: [f32; 2],
    depth_behind_anchors: bool,
}

#[wasm_bindgen]
impl WebScene {
    /// Adds homogeneous world/world relations from flat xyz arrays.
    ///
    /// # Errors
    ///
    /// Rejects malformed endpoint columns, invalid anchors or an invalid style.
    #[wasm_bindgen(js_name = addWorldRelations)]
    #[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
    pub fn add_world_relations(
        &mut self,
        namespace: u64,
        starts: Vec<f32>,
        ends: Vec<f32>,
        width_pixels: f32,
        rgba: Vec<u8>,
        opacity: f32,
        start_inset_pixels: f32,
        end_inset_pixels: f32,
        depth_behind_anchors: bool,
    ) -> Result<WebRelationBatchHandle, JsValue> {
        let starts = vec3_rows("starts", starts)?;
        let ends = vec3_rows("ends", ends)?;
        if starts.len() != ends.len() {
            return Err(js_value("relation endpoint columns must be row-aligned"));
        }
        let mut relations = Vec::with_capacity(starts.len());
        for (start, end) in starts.into_iter().zip(ends) {
            relations.push(molgfx::core::Relation {
                start: molgfx::core::SpatialAnchor::world(molgfx::math::Vec3::from_array(start))
                    .map_err(core_error)?,
                end: molgfx::core::SpatialAnchor::world(molgfx::math::Vec3::from_array(end))
                    .map_err(core_error)?,
            });
        }
        insert_relations(
            self,
            namespace,
            relations,
            RelationStyleInput {
                width_pixels,
                rgba: &rgba,
                opacity,
                endpoint_insets_pixels: [start_inset_pixels, end_inset_pixels],
                depth_behind_anchors,
            },
        )
    }

    /// Adds one homogeneous dynamic entity/entity relation stream.
    ///
    /// # Errors
    ///
    /// Rejects mismatched row columns, stale entity rows or an invalid style.
    #[wasm_bindgen(js_name = addEntityRelations)]
    #[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
    pub fn add_entity_relations(
        &mut self,
        namespace: u64,
        start_domain: &WebRowDomain,
        start_rows: Vec<u32>,
        end_domain: &WebRowDomain,
        end_rows: Vec<u32>,
        width_pixels: f32,
        rgba: Vec<u8>,
        opacity: f32,
        start_inset_pixels: f32,
        end_inset_pixels: f32,
        depth_behind_anchors: bool,
    ) -> Result<WebRelationBatchHandle, JsValue> {
        if start_rows.len() != end_rows.len() {
            return Err(js_value("relation row columns must be row-aligned"));
        }
        let mut relations = Vec::with_capacity(start_rows.len());
        for (start, end) in start_rows.into_iter().zip(end_rows) {
            relations.push(molgfx::core::Relation {
                start: molgfx::core::SpatialAnchor::entity(molgfx::core::RowEntityRef::new(
                    start_domain.inner,
                    start,
                ))
                .map_err(core_error)?,
                end: molgfx::core::SpatialAnchor::entity(molgfx::core::RowEntityRef::new(
                    end_domain.inner,
                    end,
                ))
                .map_err(core_error)?,
            });
        }
        insert_relations(
            self,
            namespace,
            relations,
            RelationStyleInput {
                width_pixels,
                rgba: &rgba,
                opacity,
                endpoint_insets_pixels: [start_inset_pixels, end_inset_pixels],
                depth_behind_anchors,
            },
        )
    }
}

fn insert_relations(
    scene: &mut WebScene,
    namespace: u64,
    relations: Vec<molgfx::core::Relation>,
    style: RelationStyleInput<'_>,
) -> Result<WebRelationBatchHandle, JsValue> {
    let rows = ordered_rows(namespace, relations.len())?;
    let batch = molgfx::core::RelationBatch::new(
        Arc::from(relations),
        rows,
        molgfx::core::RelationStyle {
            width_pixels: style.width_pixels,
            color: rgba8(style.rgba)?,
            opacity: style.opacity,
            pattern: molgfx::core::RelationPattern::Solid,
            endpoint_insets_pixels: style.endpoint_insets_pixels,
            depth_behind_anchors: style.depth_behind_anchors,
        },
    )
    .map_err(core_error)?;
    Ok(WebRelationBatchHandle {
        inner: scene.inner.add_relation_batch(batch).map_err(core_error)?,
    })
}
