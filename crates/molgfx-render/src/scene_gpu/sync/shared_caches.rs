//! The shared record and visibility caches, synced before any slot binds.

use super::GpuScene;
use super::properties;
use crate::error::RenderError;
use crate::scene_gpu::slots::GpuSlot;
use molgfx_core::Scene;
use molgfx_gpu::Device;
use std::collections::BTreeMap;

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
        requires_bvh: bool,
        derived_cache: &mut crate::DerivedCache,
        derived_frame: u64,
    ) -> Result<(), RenderError> {
        let mut needed: BTreeMap<crate::scene_gpu::record_cache::RecordKey, usize> =
            BTreeMap::new();
        let mut policies: Vec<crate::scene_gpu::slot_types::CullPolicy> =
            Vec::with_capacity(self.slots.len());
        for (slot_index, slot) in self.slots.iter().enumerate() {
            let Some(representation) = scene.representation(slot.key.representation) else {
                continue;
            };
            let Some(selection_handle) = representation.selection() else {
                continue;
            };
            let Some(placed) = scene.structure(slot.key.structure) else {
                continue;
            };
            let Some(structure_gpu) = self.structures.get(slot.structure_index) else {
                continue;
            };
            let (_, _, property_revisions) =
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
            policies.push(crate::scene_gpu::slot_types::CullPolicy {
                records: key,
                kind: representation.kind,
                visual_enabled: representation.visual.is_some(),
                bond_break_length: placed.bond_break_length(),
            });
        }
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
                let (color_property, appearance_property, _) =
                    properties::resolve(scene, representation, key.selection.structure);
                Ok(Some(crate::scene_gpu::record_cache::RecordPrepare {
                    device,
                    queue,
                    placed,
                    representation,
                    selection,
                    color_property,
                    appearance_property,
                }))
            },
        )?;
        self.sync_visibility(
            device,
            queue,
            scene,
            &policies,
            derived_cache,
            derived_frame,
        )?;
        self.sync_argument_slots(device, queue, &policies, derived_cache, derived_frame)
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
        let mut visibility_needed: BTreeMap<
            crate::scene_gpu::VisibilityKey,
            (u32, u32, crate::scene_gpu::buffers::CullCountInput),
        > = BTreeMap::new();
        for &crate::scene_gpu::slot_types::CullPolicy {
            records: record_key,
            kind,
            visual_enabled,
            bond_break_length,
        } in policies
        {
            let Some(set) = self.records.get(record_key) else {
                continue;
            };
            let bond_hierarchy = self.acceleration.get(record_key).map_or(
                crate::scene_gpu::quality_acceleration::HierarchyCounts::default(),
                |shared| shared.hierarchy().counts(),
            );
            let key = crate::scene_gpu::VisibilityKey {
                records: record_key,
                lod_mode: crate::scene_gpu::slot_types::lod_mode(
                    set.atom_count,
                    set.bond_count,
                    kind,
                ),
                visual_enabled,
                bond_break_length: bond_break_length.to_bits(),
            };
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
                bond_bvh_nodes: bond_hierarchy.nodes,
                bond_bvh_indices: bond_hierarchy.indices,
            };
            let _ =
                visibility_needed
                    .entry(key)
                    .or_insert((set.atom_count, set.bond_count, counts));
        }
        self.visibility.sync(device, queue, &visibility_needed)?;
        for key in self.visibility.keys().collect::<Vec<_>>() {
            let footprint = self.visibility.get(key).map_or(
                0,
                crate::scene_gpu::visibility_cache::VisibilitySet::resident_bytes,
            );
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
        self.argument_offsets.clear();
        let mut live = Vec::with_capacity(policies.len());
        for policy in policies {
            if self.argument_offsets.contains_key(&policy.records) {
                continue;
            }
            let Some(visibility) = self
                .visibility
                .keys()
                .find(|key| key.records == policy.records)
            else {
                continue;
            };
            let atom = self.indirect.write(
                device,
                queue,
                crate::scene_gpu::IndirectSlotKey::Atom(visibility),
                6,
                0,
            )?;
            let bond = self.indirect.write(
                device,
                queue,
                crate::scene_gpu::IndirectSlotKey::Bond(visibility),
                6,
                0,
            )?;
            let surface = self.indirect.write(
                device,
                queue,
                crate::scene_gpu::IndirectSlotKey::Surface(policy.records),
                6,
                u32::from(matches!(
                    policy.kind,
                    molgfx_core::RepresentationKind::Surface
                )),
            )?;
            live.push(crate::scene_gpu::IndirectSlotKey::Atom(visibility));
            live.push(crate::scene_gpu::IndirectSlotKey::Bond(visibility));
            live.push(crate::scene_gpu::IndirectSlotKey::Surface(policy.records));
            let _ = self
                .argument_offsets
                .insert(policy.records, (atom, bond, surface));
        }
        self.indirect.retain(&live);
        // The arena is one scene-wide allocation; it is charged against the
        // first key that needed it so the ledger can see it at all.
        if let Some((key, _)) = self.argument_offsets.iter().next() {
            let _ = derived_cache.retain(
                *key,
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
