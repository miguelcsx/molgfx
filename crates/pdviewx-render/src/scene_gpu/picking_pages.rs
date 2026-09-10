//! Bounded resident picking pages derived from the render working set.

use super::GpuScene;
use crate::RenderError;
use hashbrown::HashMap;
use pdviewx_core::{
    ChunkId, ChunkSpan, DatasetId, EntityKind, GlobalPickIdentity, GpuPickToken, LogicalRow,
    PagedPickResolver, PickPageDescriptor, PickPageTicket, PickReadback, Scene, SourceRows,
};
use pdviewx_gpu::Device;
use std::sync::Arc;

const MONOLITHIC_CHUNK: ChunkId = ChunkId::new(0);
const EMPTY_PAGE: u32 = u32::MAX;

#[derive(Clone, PartialEq, Eq, Debug)]
struct PagePlan {
    dataset: DatasetId,
    chunk: ChunkId,
    span: ChunkSpan,
    kind: EntityKind,
    source_keys: Option<Arc<[u64]>>,
}

pub(super) type ChunkPickPlan = (DatasetId, ChunkId, ChunkSpan, EntityKind);

#[derive(Clone, Copy, Debug)]
struct PageBinding {
    ticket: PickPageTicket,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct PickRevision {
    scene: u64,
    structure: u64,
    interaction: u64,
    guide: u64,
    label: u64,
    primitive: u64,
    mesh: u64,
    topology: u64,
    generic: u64,
}

/// Fixed-capacity host page table paired with the core resolver.
#[derive(Debug)]
pub(super) struct PickPages {
    resolver: PagedPickResolver,
    bindings: Vec<PageBinding>,
    source_keys: Vec<Option<Arc<[u64]>>>,
    binding_index: HashMap<(DatasetId, ChunkId, EntityKind, bool), u32>,
    scene_plans: Vec<PagePlan>,
    chunk_plans: Vec<PagePlan>,
    revision: Option<PickRevision>,
    scene_table_revision: u64,
    table_revision: u64,
}

impl PickPages {
    pub(super) fn new(capacity: u32) -> Result<Self, RenderError> {
        let resolver = PagedPickResolver::new(capacity)?;
        let capacity = resolver.capacity();
        let mut bindings = Vec::new();
        let mut source_keys = Vec::new();
        let mut binding_index = HashMap::new();
        let mut scene_plans = Vec::new();
        let mut chunk_plans = Vec::new();
        bindings
            .try_reserve_exact(capacity)
            .map_err(|_| pdviewx_core::PickingError::AllocationFailed)?;
        source_keys
            .try_reserve_exact(capacity)
            .map_err(|_| pdviewx_core::PickingError::AllocationFailed)?;
        source_keys.resize(capacity, None);
        binding_index
            .try_reserve(capacity)
            .map_err(|_| pdviewx_core::PickingError::AllocationFailed)?;
        scene_plans
            .try_reserve_exact(capacity)
            .map_err(|_| pdviewx_core::PickingError::AllocationFailed)?;
        chunk_plans
            .try_reserve_exact(capacity)
            .map_err(|_| pdviewx_core::PickingError::AllocationFailed)?;
        Ok(Self {
            resolver,
            bindings,
            source_keys,
            binding_index,
            scene_plans,
            chunk_plans,
            revision: None,
            scene_table_revision: 1,
            table_revision: 1,
        })
    }

    pub(super) fn capacity(&self) -> usize {
        self.resolver.capacity()
    }

    pub(super) fn sync(&mut self, scene: &Scene) -> Result<bool, RenderError> {
        let revision = revision(scene);
        if self.revision == Some(revision) {
            return Ok(false);
        }
        self.build_plans(scene)?;
        self.rebuild_pages()?;
        self.revision = Some(revision);
        self.scene_table_revision = self.scene_table_revision.wrapping_add(1).max(1);
        Ok(true)
    }

    pub(super) fn sync_draw_chunks(
        &mut self,
        plans: &[ChunkPickPlan],
    ) -> Result<bool, RenderError> {
        let is_draw = |kind| matches!(kind, EntityKind::Atom | EntityKind::Point);
        let current = self.chunk_plans.iter().filter(|value| is_draw(value.kind));
        let unchanged = current.clone().count() == plans.len()
            && current
                .zip(plans)
                .all(|(left, right)| (left.dataset, left.chunk, left.span, left.kind) == *right);
        if unchanged {
            return Ok(false);
        }
        let retained = self
            .chunk_plans
            .iter()
            .filter(|value| !is_draw(value.kind))
            .count();
        if retained.saturating_add(plans.len()) > self.chunk_plans.capacity() {
            return Err(pdviewx_core::PickingError::WorkingSetFull.into());
        }
        self.chunk_plans.retain(|value| !is_draw(value.kind));
        self.chunk_plans.extend(plans.iter().map(|plan| PagePlan {
            dataset: plan.0,
            chunk: plan.1,
            span: plan.2,
            kind: plan.3,
            source_keys: None,
        }));
        self.rebuild_pages()?;
        Ok(true)
    }

    pub(super) fn sync_bond_chunks(
        &mut self,
        plans: &[ChunkPickPlan],
    ) -> Result<bool, RenderError> {
        self.sync_chunk_kind(plans, EntityKind::Bond)
    }

    pub(super) fn sync_instance_chunks(
        &mut self,
        plans: &[ChunkPickPlan],
    ) -> Result<bool, RenderError> {
        self.sync_chunk_kind(plans, EntityKind::TemplatePart)
    }

    pub(super) fn sync_relation_chunks(
        &mut self,
        plans: &[ChunkPickPlan],
    ) -> Result<bool, RenderError> {
        self.sync_chunk_kind(plans, EntityKind::Relation)
    }

    fn sync_chunk_kind(
        &mut self,
        plans: &[ChunkPickPlan],
        kind: EntityKind,
    ) -> Result<bool, RenderError> {
        let current = self.chunk_plans.iter().filter(|value| value.kind == kind);
        let unchanged = current.clone().count() == plans.len()
            && current
                .zip(plans)
                .all(|(left, right)| (left.dataset, left.chunk, left.span, left.kind) == *right);
        if unchanged {
            return Ok(false);
        }
        let retained = self
            .chunk_plans
            .iter()
            .filter(|value| value.kind != kind)
            .count();
        if retained.saturating_add(plans.len()) > self.chunk_plans.capacity() {
            return Err(pdviewx_core::PickingError::WorkingSetFull.into());
        }
        self.chunk_plans.retain(|value| value.kind != kind);
        self.chunk_plans.extend(plans.iter().map(|plan| PagePlan {
            dataset: plan.0,
            chunk: plan.1,
            span: plan.2,
            kind: plan.3,
            source_keys: None,
        }));
        self.rebuild_pages()?;
        Ok(true)
    }

    fn rebuild_pages(&mut self) -> Result<(), RenderError> {
        if self.scene_plans.len() + self.chunk_plans.len() > self.resolver.capacity() {
            return Err(pdviewx_core::PickingError::WorkingSetFull.into());
        }
        self.release_pages()?;
        let scene_count = self.scene_plans.len();
        for (index, plan) in self.scene_plans.iter().chain(&self.chunk_plans).enumerate() {
            let descriptor =
                PickPageDescriptor::new(plan.dataset, plan.chunk, plan.span, plan.kind);
            let ticket = self.resolver.request(descriptor)?;
            self.resolver.complete(ticket)?;
            self.bindings.push(PageBinding { ticket });
            if let Some(slot) = self.source_keys.get_mut(ticket.page().get() as usize) {
                slot.clone_from(&plan.source_keys);
            }
            self.binding_index.insert(
                (plan.dataset, plan.chunk, plan.kind, index >= scene_count),
                ticket.page().get(),
            );
        }
        self.table_revision = self.table_revision.wrapping_add(1).max(1);
        Ok(())
    }

    pub(super) const fn table_revision(&self) -> u64 {
        self.table_revision
    }

    pub(super) const fn scene_table_revision(&self) -> u64 {
        self.scene_table_revision
    }

    pub(super) fn pages_for(&self, dataset: DatasetId) -> [u32; 9] {
        let mut pages = [EMPTY_PAGE; 9];
        for kind in STRUCTURE_ENTITY_KINDS {
            if let Some(page) = self
                .binding_index
                .get(&(dataset, MONOLITHIC_CHUNK, kind, false))
            {
                pages[kind_index(kind)] = *page;
            }
        }
        pages
    }

    pub(super) fn page_for_chunk(
        &self,
        dataset: DatasetId,
        chunk: ChunkId,
        kind: EntityKind,
    ) -> Option<u32> {
        self.binding_index
            .get(&(dataset, chunk, kind, true))
            .copied()
    }

    pub(super) fn page_for_table(
        &self,
        dataset: DatasetId,
        chunk: ChunkId,
        kind: EntityKind,
    ) -> Option<u32> {
        self.binding_index
            .get(&(dataset, chunk, kind, false))
            .copied()
    }

    #[cfg(test)]
    pub(super) fn page_for(&self, dataset: DatasetId, kind: EntityKind) -> Option<u32> {
        self.binding_index
            .get(&(dataset, MONOLITHIC_CHUNK, kind, false))
            .copied()
    }

    pub(super) fn capture(&self, destination: &mut [Option<PickPageTicket>]) {
        destination.fill(None);
        for binding in &self.bindings {
            let index = binding.ticket.page().get() as usize;
            if let Some(slot) = destination.get_mut(index) {
                *slot = Some(binding.ticket);
            }
        }
    }

    pub(super) fn resolve(
        &self,
        token: GpuPickToken,
        submission: &[Option<PickPageTicket>],
    ) -> Result<GlobalPickIdentity, RenderError> {
        let page = token.page().ok_or(pdviewx_core::PickingError::EmptyToken)?;
        let ticket = submission
            .get(page.get() as usize)
            .copied()
            .flatten()
            .ok_or(pdviewx_core::PickingError::PageNotResident)?;
        let identity = self
            .resolver
            .resolve(PickReadback::new(token, ticket.generation()))?;
        let Some(keys) = self
            .source_keys
            .get(page.get() as usize)
            .and_then(Option::as_ref)
        else {
            return Ok(identity);
        };
        let key = keys.get(token.local_row().get() as usize).copied().ok_or(
            pdviewx_core::PickingError::LocalRowOutsidePage {
                page: page.get(),
                row: token.local_row().get(),
                row_count: u32::try_from(keys.len()).map_or(u32::MAX, |value| value),
            },
        )?;
        Ok(GlobalPickIdentity::new(
            identity.dataset(),
            identity.chunk(),
            LogicalRow::new(key),
            identity.kind(),
        ))
    }

    fn release_pages(&mut self) -> Result<(), RenderError> {
        self.binding_index.clear();
        for binding in self.bindings.drain(..) {
            if let Some(slot) = self
                .source_keys
                .get_mut(binding.ticket.page().get() as usize)
            {
                *slot = None;
            }
            self.resolver.release(binding.ticket)?;
        }
        Ok(())
    }

    fn build_plans(&mut self, scene: &Scene) -> Result<(), RenderError> {
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
                placed.structure.data().bonds.len(),
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
                .map_err(|_| pdviewx_core::PickingError::CapacityTooLarge)?;
            let occurrence_count = batch
                .source_rows()
                .len()
                .checked_mul(part_count)
                .ok_or(pdviewx_core::PickingError::CapacityTooLarge)?;
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
        if self.scene_plans.len() == self.scene_plans.capacity() {
            return Err(pdviewx_core::PickingError::WorkingSetFull.into());
        }
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
        if self.scene_plans.len() == self.scene_plans.capacity() {
            return Err(pdviewx_core::PickingError::WorkingSetFull.into());
        }
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

include!("picking_pages_support.rs");
