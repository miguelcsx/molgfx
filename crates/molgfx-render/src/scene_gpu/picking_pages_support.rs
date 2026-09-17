// Cold helpers and engine-facing views for picking pages.

impl PickPages {
    fn add_owner_row(
        &mut self,
        scene: &Scene,
        owner: molgfx_core::StructureHandle,
        kind: EntityKind,
        row: u32,
    ) -> Result<(), RenderError> {
        let dataset = scene
            .structure(owner)
            .ok_or(RenderError::PickingOwnerMissing)?
            .dataset_id();
        let rows = row
            .checked_add(1)
            .ok_or(molgfx_core::PickingError::CapacityTooLarge)?;
        self.add_rows(dataset, kind, rows)
    }

    fn add_len(
        &mut self,
        dataset: DatasetId,
        kind: EntityKind,
        len: usize,
    ) -> Result<(), RenderError> {
        let rows = u32::try_from(len).map_err(|_| molgfx_core::PickingError::CapacityTooLarge)?;
        self.add_rows(dataset, kind, rows)
    }

    fn add_rows(
        &mut self,
        dataset: DatasetId,
        kind: EntityKind,
        rows: u32,
    ) -> Result<(), RenderError> {
        if rows == 0 {
            return Ok(());
        }
        if let Some(plan) = self
            .scene_plans
            .iter_mut()
            .find(|value| value.dataset == dataset && value.kind == kind)
        {
            if rows > plan.span.row_count() {
                plan.span = ChunkSpan::new(LogicalRow::new(0), rows)?;
            }
            return Ok(());
        }
        if self.scene_plans.len() == self.scene_plans.capacity() {
            return Err(molgfx_core::PickingError::WorkingSetFull.into());
        }
        self.scene_plans.push(PagePlan {
            dataset,
            chunk: MONOLITHIC_CHUNK,
            span: ChunkSpan::new(LogicalRow::new(0), rows)?,
            kind,
            source_keys: None,
        });
        Ok(())
    }
}

const STRUCTURE_ENTITY_KINDS: [EntityKind; 9] = [
    EntityKind::Atom,
    EntityKind::Bond,
    EntityKind::Edge,
    EntityKind::Label,
    EntityKind::Primitive,
    EntityKind::Mesh,
    EntityKind::LigandPoseBatch,
    EntityKind::Guide,
    EntityKind::DynamicBond,
];

pub(super) const fn kind_index(kind: EntityKind) -> usize {
    match kind {
        EntityKind::Atom => 0,
        EntityKind::Bond => 1,
        EntityKind::Edge => 2,
        EntityKind::Label => 3,
        EntityKind::Primitive => 4,
        EntityKind::Mesh => 5,
        EntityKind::LigandPoseBatch => 6,
        EntityKind::Guide => 7,
        EntityKind::DynamicBond => 8,
        EntityKind::Point => 9,
        EntityKind::Instance => 10,
        EntityKind::TemplatePart => 11,
        EntityKind::Relation => 12,
    }
}

fn revision(scene: &Scene) -> PickRevision {
    let topology = scene.structures().fold(0_u64, |value, (_, placed)| {
        value
            .wrapping_mul(0x9e37_79b9_7f4a_7c15)
            .wrapping_add(placed.bond_topology_pair_revision())
    });
    PickRevision {
        scene: scene.cache_identity(),
        structure: scene.structure_revision(),
        interaction: scene.interaction_revision(),
        guide: scene.guide_revision(),
        label: scene.label_revision(),
        primitive: scene.primitive_revision(),
        mesh: scene.mesh_revision(),
        topology,
        generic: scene.generic_batch_revision(),
    }
}

fn table_chunk(row: u32, generation: u32) -> ChunkId {
    ChunkId::new(u64::from(row) | (u64::from(generation) << 32))
}

impl<D: Device> GpuScene<D> {
    pub(crate) fn capture_pick_submission(
        &self,
        destination: &mut [Option<PickPageTicket>],
    ) -> Result<(), RenderError> {
        if destination.len() != self.picking_pages.capacity() {
            return Err(molgfx_core::PickingError::CapacityTooLarge.into());
        }
        self.picking_pages.capture(destination);
        Ok(())
    }

    pub(crate) fn resolve_global_pick(
        &self,
        token: GpuPickToken,
        submission: &[Option<PickPageTicket>],
    ) -> Result<GlobalPickIdentity, RenderError> {
        self.picking_pages.resolve(token, submission)
    }

    #[cfg(test)]
    pub(crate) fn test_pick_page(&self, dataset: DatasetId, kind: EntityKind) -> Option<u32> {
        self.picking_pages.page_for(dataset, kind)
    }

    #[cfg(test)]
    pub(crate) fn test_chunk_pick_page(
        &self,
        dataset: DatasetId,
        chunk: ChunkId,
        kind: EntityKind,
    ) -> Option<u32> {
        self.picking_pages.page_for_chunk(dataset, chunk, kind)
    }

    #[cfg(test)]
    pub(crate) fn test_table_pick_page(
        &self,
        dataset: DatasetId,
        chunk: ChunkId,
        kind: EntityKind,
    ) -> Option<u32> {
        self.picking_pages.page_for_table(dataset, chunk, kind)
    }
}
