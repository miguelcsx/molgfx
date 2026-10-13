//! Page plans: which dataset rows of a scene need a resident pick page.
//!
//! One pass over the scene's structures, owned rows and generic batches,
//! linear in their count, appending to the scene plan list.

use super::{PagePlan, PickPages, table_chunk};
use crate::RenderError;
use molgfx_core::{ChunkId, ChunkSpan, DatasetId, EntityKind, LogicalRow, Scene, SourceRows};

impl PickPages {
    pub(super) fn build_plans(&mut self, scene: &Scene) -> Result<(), RenderError> {
        self.scene_plans.clear();
        self.build_structure_plans(scene)?;
        self.build_owned_plans(scene)?;
        self.build_generic_plans(scene)
    }

    fn build_structure_plans(&mut self, scene: &Scene) -> Result<(), RenderError> {
        for (_, placed) in scene.structures() {
            let dataset = placed.dataset_id();
            self.add_rows(dataset, EntityKind::Atom, placed.atoms.len())?;
            self.add_len(
                dataset,
                EntityKind::Bond,
                placed.source.topology().bonds.len(),
            )?;
            if let Some(topology) = placed.bond_topology() {
                self.add_len(dataset, EntityKind::DynamicBond, topology.bonds().len())?;
            }
        }
        Ok(())
    }

    fn build_owned_plans(&mut self, scene: &Scene) -> Result<(), RenderError> {
        for (handle, edge) in scene.interactions() {
            self.add_owner_row(
                scene,
                edge.owner(),
                EntityKind::Edge,
                Scene::interaction_row(handle),
            )?;
        }
        for (handle, guide) in scene.guides() {
            self.add_owner_row(
                scene,
                guide.owner(),
                EntityKind::Guide,
                Scene::guide_row(handle),
            )?;
        }
        for (handle, label) in scene.annotations() {
            self.add_owner_row(
                scene,
                label.owner(),
                EntityKind::Label,
                Scene::annotation_row(handle),
            )?;
        }
        for (handle, measurement) in scene.measurements() {
            self.add_owner_row(
                scene,
                measurement.owner(),
                EntityKind::Label,
                Scene::measurement_row(handle),
            )?;
        }
        for (handle, primitive) in scene.primitives() {
            self.add_owner_row(
                scene,
                primitive.owner(),
                EntityKind::Primitive,
                Scene::primitive_row(handle),
            )?;
        }
        for (handle, mesh) in scene.meshes() {
            self.add_owner_row(
                scene,
                mesh.owner(),
                EntityKind::Mesh,
                Scene::mesh_row(handle),
            )?;
        }
        for (handle, batch) in scene.ligand_pose_batches() {
            self.add_owner_row(
                scene,
                batch.owner(),
                EntityKind::LigandPoseBatch,
                Scene::ligand_pose_batch_entity_row(handle),
            )?;
        }
        Ok(())
    }

    fn build_generic_plans(&mut self, scene: &Scene) -> Result<(), RenderError> {
        for (handle, batch) in scene.point_batches() {
            self.add_source_rows(
                table_chunk(handle.row(), handle.generation()),
                EntityKind::Point,
                batch.source_rows(),
            )?;
        }
        for (handle, batch) in scene.instance_batches() {
            let chunk = table_chunk(handle.row(), handle.generation());
            self.add_source_rows(chunk, EntityKind::Instance, batch.source_rows())?;
            let part_count = u32::try_from(batch.template().part_count())
                .map_err(|_| molgfx_core::PickingError::CapacityTooLarge)?;
            let occurrence_count = batch
                .source_rows()
                .len()
                .checked_mul(part_count)
                .ok_or(molgfx_core::PickingError::CapacityTooLarge)?;
            self.add_table_rows(
                DatasetId::new(batch.template().source_rows().namespace().0),
                chunk,
                EntityKind::TemplatePart,
                occurrence_count,
            )?;
        }
        for (handle, batch) in scene.relation_batches() {
            self.add_source_rows(
                table_chunk(handle.row(), handle.generation()),
                EntityKind::Relation,
                batch.source_rows(),
            )?;
        }
        Ok(())
    }

    fn add_source_rows(
        &mut self,
        chunk: ChunkId,
        kind: EntityKind,
        rows: &SourceRows,
    ) -> Result<(), RenderError> {
        if rows.is_empty() {
            return Ok(());
        }
        Self::check_plan_room(self.scene_plans.len())?;
        self.scene_plans.push(PagePlan {
            dataset: DatasetId::new(rows.namespace().0),
            chunk,
            span: ChunkSpan::new(LogicalRow::new(0), rows.len())?,
            kind,
            source_keys: rows.keys().cloned(),
        });
        Ok(())
    }

    fn add_table_rows(
        &mut self,
        dataset: DatasetId,
        chunk: ChunkId,
        kind: EntityKind,
        row_count: u32,
    ) -> Result<(), RenderError> {
        if row_count == 0 {
            return Ok(());
        }
        Self::check_plan_room(self.scene_plans.len())?;
        self.scene_plans.push(PagePlan {
            dataset,
            chunk,
            span: ChunkSpan::new(LogicalRow::new(0), row_count)?,
            kind,
            source_keys: None,
        });
        Ok(())
    }
}
