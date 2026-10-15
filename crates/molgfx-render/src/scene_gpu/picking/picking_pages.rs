//! Bounded resident picking pages derived from the render working set.

use crate::RenderError;
use crate::scene_gpu::GpuScene;
use hashbrown::HashMap;
use molgfx_core::{
    ChunkId, ChunkSpan, DatasetId, EntityKind, GlobalPickIdentity, GpuPickToken, LogicalRow,
    PagedPickResolver, PickPageDescriptor, PickPageTicket, PickReadback, Scene,
};
use molgfx_gpu::Device;
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

pub(in crate::scene_gpu) type ChunkPickPlan = (DatasetId, ChunkId, ChunkSpan, EntityKind);

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
pub(in crate::scene_gpu) struct PickPages {
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
    pub(in crate::scene_gpu) fn new(capacity: u32) -> Result<Self, RenderError> {
        let resolver = PagedPickResolver::new(capacity)?;
        let capacity = resolver.capacity();
        let mut bindings = Vec::new();
        let mut source_keys = Vec::new();
        let mut binding_index = HashMap::new();
        let mut scene_plans = Vec::new();
        let mut chunk_plans = Vec::new();
        bindings
            .try_reserve_exact(capacity)
            .map_err(|_| molgfx_core::PickingError::AllocationFailed)?;
        source_keys
            .try_reserve_exact(capacity)
            .map_err(|_| molgfx_core::PickingError::AllocationFailed)?;
        source_keys.resize(capacity, None);
        binding_index
            .try_reserve(capacity)
            .map_err(|_| molgfx_core::PickingError::AllocationFailed)?;
        scene_plans
            .try_reserve_exact(capacity)
            .map_err(|_| molgfx_core::PickingError::AllocationFailed)?;
        chunk_plans
            .try_reserve_exact(capacity)
            .map_err(|_| molgfx_core::PickingError::AllocationFailed)?;
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

    pub(in crate::scene_gpu) fn capacity(&self) -> usize {
        self.resolver.capacity()
    }

    /// Makes room for `needed` simultaneous pages, growing the table on demand.
    ///
    /// The configured capacity is only the initial size. Existing pages and
    /// their generations survive growth, so a pick submitted before it still
    /// resolves. Past [`molgfx_core::MAX_PICK_PAGES`] the error names that limit.
    fn reserve_pages(&mut self, needed: usize) -> Result<(), RenderError> {
        let limit = molgfx_core::MAX_PICK_PAGES;
        if needed > limit as usize {
            return Err(molgfx_core::PickingError::PageLimit { limit }.into());
        }
        let current = self.resolver.capacity();
        if needed <= current {
            return Ok(());
        }
        let target = needed.next_power_of_two().min(limit as usize);
        let target_u32 =
            u32::try_from(target).map_err(|_| molgfx_core::PickingError::CapacityTooLarge)?;
        self.resolver.grow(target_u32)?;
        self.source_keys.resize(target, None);
        Ok(())
    }

    /// Rejects a plan list that could never fit, before any work is done.
    fn check_plan_room(len: usize) -> Result<(), RenderError> {
        let limit = molgfx_core::MAX_PICK_PAGES;
        if len >= limit as usize {
            return Err(molgfx_core::PickingError::PageLimit { limit }.into());
        }
        Ok(())
    }

    pub(in crate::scene_gpu) fn sync(&mut self, scene: &Scene) -> Result<bool, RenderError> {
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

    pub(in crate::scene_gpu) fn sync_draw_chunks(
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
        if retained.saturating_add(plans.len()) > molgfx_core::MAX_PICK_PAGES as usize {
            return Err(molgfx_core::PickingError::PageLimit {
                limit: molgfx_core::MAX_PICK_PAGES,
            }
            .into());
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

    pub(in crate::scene_gpu) fn sync_bond_chunks(
        &mut self,
        plans: &[ChunkPickPlan],
    ) -> Result<bool, RenderError> {
        self.sync_chunk_kind(plans, EntityKind::Bond)
    }

    pub(in crate::scene_gpu) fn sync_instance_chunks(
        &mut self,
        plans: &[ChunkPickPlan],
    ) -> Result<bool, RenderError> {
        self.sync_chunk_kind(plans, EntityKind::TemplatePart)
    }

    pub(in crate::scene_gpu) fn sync_relation_chunks(
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
        if retained.saturating_add(plans.len()) > molgfx_core::MAX_PICK_PAGES as usize {
            return Err(molgfx_core::PickingError::PageLimit {
                limit: molgfx_core::MAX_PICK_PAGES,
            }
            .into());
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
        self.release_pages()?;
        self.reserve_pages(self.scene_plans.len() + self.chunk_plans.len())?;
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

    pub(in crate::scene_gpu) const fn table_revision(&self) -> u64 {
        self.table_revision
    }

    pub(in crate::scene_gpu) const fn scene_table_revision(&self) -> u64 {
        self.scene_table_revision
    }

    pub(in crate::scene_gpu) fn pages_for(&self, dataset: DatasetId) -> [u32; 9] {
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

    pub(in crate::scene_gpu) fn page_for_chunk(
        &self,
        dataset: DatasetId,
        chunk: ChunkId,
        kind: EntityKind,
    ) -> Option<u32> {
        self.binding_index
            .get(&(dataset, chunk, kind, true))
            .copied()
    }

    pub(in crate::scene_gpu) fn page_for_table(
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
    pub(in crate::scene_gpu) fn page_for(
        &self,
        dataset: DatasetId,
        kind: EntityKind,
    ) -> Option<u32> {
        self.binding_index
            .get(&(dataset, MONOLITHIC_CHUNK, kind, false))
            .copied()
    }

    pub(in crate::scene_gpu) fn capture(&self, destination: &mut [Option<PickPageTicket>]) {
        destination.fill(None);
        for binding in &self.bindings {
            let index = binding.ticket.page().get() as usize;
            if let Some(slot) = destination.get_mut(index) {
                *slot = Some(binding.ticket);
            }
        }
    }

    pub(in crate::scene_gpu) fn resolve(
        &self,
        token: GpuPickToken,
        submission: &[Option<PickPageTicket>],
    ) -> Result<GlobalPickIdentity, RenderError> {
        let page = token.page().ok_or(molgfx_core::PickingError::EmptyToken)?;
        let ticket = submission
            .get(page.get() as usize)
            .copied()
            .flatten()
            .ok_or(molgfx_core::PickingError::PageNotResident)?;
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
            molgfx_core::PickingError::LocalRowOutsidePage {
                page: page.get(),
                row: token.local_row().get(),
                row_count: crate::fallback(u32::try_from(keys.len()), u32::MAX),
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
}

mod plans;

include!("picking_pages_support.rs");
