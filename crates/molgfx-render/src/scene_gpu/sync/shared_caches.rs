//! The shared record and visibility caches, synced before any slot binds.

use super::GpuScene;
use super::properties;
use crate::error::RenderError;
use crate::scene_gpu::slots::GpuSlot;
use molgfx_core::Scene;
use molgfx_gpu::Device;

/// Fills the per-slot culling policies and the distinct record keys they need,
/// into caller-owned storage so a steady-state frame allocates nothing.
fn pack_slot_policies<D: Device>(
    gpu: &GpuScene<D>,
    scene: &Scene,
    detail: crate::scene_gpu::detail::TierDetail,
    needed: &mut std::collections::BTreeMap<crate::scene_gpu::record_cache::RecordKey, usize>,
    policies: &mut Vec<crate::scene_gpu::slot_types::CullPolicy>,
) {
    for (slot_index, slot) in gpu.slots.iter().enumerate() {
        let Some(representation) = scene.representation(slot.key.representation) else {
            continue;
        };
        let Some(selection_handle) = representation.selection() else {
            continue;
        };
        let Some(placed) = scene.structure(slot.key.structure) else {
            continue;
        };
        let Some(structure_gpu) = gpu.structures.get(slot.structure_index) else {
            continue;
        };
        let (_, property_revisions) =
            properties::resolve(scene, representation, slot.key.structure);
        let key = crate::scene_gpu::record_cache::RecordCache::<D>::key(
            scene,
            placed,
            slot.key.structure,
            representation,
            selection_handle,
            structure_gpu.asset_identity(),
            property_revisions,
        );
        let _ = needed.entry(key).or_insert(slot_index);
        policies.push(crate::scene_gpu::slot_types::CullPolicy::new(
            key,
            representation,
            placed,
            detail.lod_enabled,
        ));
    }
}

impl<D: Device> GpuScene<D> {
    /// Pass one: pack every distinct record set this frame needs.
    ///
    /// Walks the slots read-only, so the record cache is free to be borrowed
    /// mutably. `prepare` runs once per key, never once per representation.
    pub(super) fn sync_records(
        &mut self,
        device: &D,
        queue: &D::Queue,
        scene: &Scene,
        sampling: (bool, crate::scene_gpu::detail::TierDetail),
        derived_cache: &mut crate::DerivedCache,
        derived_frame: u64,
    ) -> Result<(), RenderError> {
        let (requires_bvh, detail) = sampling;
        // Rebuild into the scene's retained capacity: a steady-state frame
        // must not allocate a fresh map and policy vector per frame.
        let mut needed = std::mem::take(&mut self.sync_scratch.needed_records);
        needed.clear();
        let mut policies = std::mem::take(&mut self.sync_scratch.cull_policies);
        policies.clear();
        pack_slot_policies(self, scene, detail, &mut needed, &mut policies);
        {
            let slots: &[GpuSlot<D>] = &self.slots;
            let (records, acceleration) = (&mut self.records, &mut self.acceleration);
            let mut scratch = crate::scene_gpu::record_cache::RecordScratch {
                atoms: &mut self.atom_scratch,
                bonds: &mut self.bond_scratch,
                compaction: &mut self.compaction_scratch,
            };
            let mut budget = crate::scene_gpu::record_cache::RecordBudget::<D> {
                ledger: derived_cache,
                frame: derived_frame,
                _device: std::marker::PhantomData,
            };
            records.sync(
                acceleration,
                &mut budget,
                &needed,
                requires_bvh,
                &mut scratch,
                |slot_index, key| {
                    let Some(slot) = slots.get(slot_index) else {
                        return Ok(None);
                    };
                    let Some(representation) = scene.representation(slot.key.representation) else {
                        return Ok(None);
                    };
                    let Some(selection_handle) = representation.selection() else {
                        return Ok(None);
                    };
                    let Some(selection) =
                        scene.selection_for(selection_handle, key.selection.structure)
                    else {
                        return Ok(None);
                    };
                    let Some(placed) = scene.structure(key.selection.structure) else {
                        return Ok(None);
                    };
                    let (color, _) =
                        properties::resolve(scene, representation, key.selection.structure);
                    Ok(Some(crate::scene_gpu::record_cache::RecordPrepare {
                        device,
                        queue,
                        placed,
                        representation,
                        selection,
                        color,
                    }))
                },
            )?;
        }
        let result = (|| {
            self.sync_visibility(
                device,
                queue,
                scene,
                &policies,
                derived_cache,
                derived_frame,
            )?;
            self.sync_argument_slots(device, queue, &policies, derived_cache, derived_frame)
        })();
        // Return the buffers whatever the outcome so a failed frame keeps the
        // capacity for the next one.
        self.sync_scratch.needed_records = needed;
        self.sync_scratch.cull_policies = policies;
        result
    }

    /// Derives one shared visible set per distinct culling policy.
    ///
    /// Runs after the record sync, because a visible set is sized from the
    /// packed counts, and before any slot binds, because every slot's cull
    /// groups read this cache.
    pub(super) fn sync_visibility(
        &mut self,
        device: &D,
        queue: &D::Queue,
        scene: &Scene,
        policies: &[crate::scene_gpu::slot_types::CullPolicy],
        derived_cache: &mut crate::DerivedCache,
        derived_frame: u64,
    ) -> Result<(), RenderError> {
        let mut visibility_needed = std::mem::take(&mut self.sync_scratch.visibility_needed);
        visibility_needed.clear();
        for &policy in policies {
            let record_key = policy.records;
            let bond_break_length = policy.bond_break_length;
            let visual_enabled = policy.visual_enabled;
            let Some(set) = self.records.get(record_key) else {
                continue;
            };
            let quality_hierarchy = self.acceleration.get(record_key).map_or(
                crate::scene_gpu::quality_acceleration::HierarchyCounts::default(),
                |shared| shared.hierarchy().counts(),
            );
            let key = policy.visibility_key(set.atom_count, set.bond_count);
            let atom_hierarchy = match scene
                .structure(record_key.selection.structure)
                .and_then(|placed| placed.render_bvh().ok())
            {
                Some(bvh) => match crate::scene_gpu::quality_acceleration::hierarchy_counts(bvh) {
                    Ok(counts) => counts,
                    Err(_) => continue,
                },
                None => continue,
            };
            let counts = crate::scene_gpu::buffers::CullCountInput {
                atoms: set.atom_count,
                bonds: set.bond_count,
                lod_mode: key.lod_mode,
                bond_break_length,
                visual_enabled,
                atom_bvh_nodes: atom_hierarchy.nodes,
                atom_bvh_indices: atom_hierarchy.indices,
                quality_bvh_nodes: quality_hierarchy.nodes,
                quality_bvh_indices: quality_hierarchy.indices,
            };
            let _ =
                visibility_needed
                    .entry(key)
                    .or_insert((set.atom_count, set.bond_count, counts));
        }
        let synced = self.visibility.sync(device, queue, &visibility_needed);
        self.sync_scratch.visibility_needed = visibility_needed;
        synced?;
        for (key, footprint) in self.visibility.resident() {
            let _ = derived_cache.retain(
                key,
                crate::DerivedCacheClass::Visibility,
                crate::DerivedFootprint {
                    cpu_bytes: 0,
                    gpu_bytes: footprint,
                },
                derived_frame,
            );
        }
        Ok(())
    }

    /// Resolves each record key's slots in the scene-wide argument arena.
    ///
    /// Written here rather than per slot because the arena is scene state: one
    /// allocation serves every representation, and a key's slots are shared by
    /// every representation that draws it. The cull shader writes instance
    /// counts through the slot's own range binding and each draw reads it back.
    fn sync_argument_slots(
        &mut self,
        device: &D,
        queue: &D::Queue,
        policies: &[crate::scene_gpu::slot_types::CullPolicy],
        derived_cache: &mut crate::DerivedCache,
        derived_frame: u64,
    ) -> Result<(), RenderError> {
        let additional = policies
            .iter()
            .filter_map(|policy| {
                let records = self.records.get(policy.records)?;
                let visibility = policy.visibility_key(records.atom_count, records.bond_count);
                self.visibility.get(visibility)?;
                Some(
                    [
                        crate::scene_gpu::IndirectSlotKey::Atom(visibility, policy.atom_vertices),
                        crate::scene_gpu::IndirectSlotKey::Bond(visibility),
                        crate::scene_gpu::IndirectSlotKey::Surface(policy.records),
                    ]
                    .into_iter()
                    .filter(|key| !self.indirect.contains(*key))
                    .count(),
                )
            })
            .sum();
        if self.indirect.reserve(device, additional)? {
            self.cull_binding_revision = self.cull_binding_revision.wrapping_add(1);
        }
        self.argument_offsets.clear();
        let mut live = std::mem::take(&mut self.sync_scratch.live_argument_slots);
        live.clear();
        for policy in policies {
            let Some(records) = self.records.get(policy.records) else {
                continue;
            };
            let visibility = policy.visibility_key(records.atom_count, records.bond_count);
            if self
                .argument_offsets
                .contains_key(&(visibility, policy.atom_vertices))
                || self.visibility.get(visibility).is_none()
            {
                continue;
            }
            let atom = self.indirect.write(
                queue,
                crate::scene_gpu::IndirectSlotKey::Atom(visibility, policy.atom_vertices),
                policy.atom_vertices,
                0,
            )?;
            let bond = self.indirect.write(
                queue,
                crate::scene_gpu::IndirectSlotKey::Bond(visibility),
                6,
                0,
            )?;
            let surface = self.indirect.write(
                queue,
                crate::scene_gpu::IndirectSlotKey::Surface(policy.records),
                6,
                u32::from(matches!(
                    policy.kind,
                    molgfx_core::RepresentationKind::Surface
                )),
            )?;
            live.push(crate::scene_gpu::IndirectSlotKey::Atom(
                visibility,
                policy.atom_vertices,
            ));
            live.push(crate::scene_gpu::IndirectSlotKey::Bond(visibility));
            live.push(crate::scene_gpu::IndirectSlotKey::Surface(policy.records));
            let _ = self
                .argument_offsets
                .insert((visibility, policy.atom_vertices), (atom, bond, surface));
        }
        self.indirect.retain(&live);
        self.sync_scratch.live_argument_slots = live;
        // The arena is one scene-wide allocation; it is charged against the
        // first key that needed it so the ledger can see it at all.
        if let Some((key, _)) = self.argument_offsets.iter().next() {
            let _ = derived_cache.retain(
                key.0.records,
                crate::DerivedCacheClass::RecordSet,
                crate::DerivedFootprint {
                    cpu_bytes: 0,
                    gpu_bytes: self.indirect.resident_bytes(),
                },
                derived_frame,
            );
        }
        Ok(())
    }
}
