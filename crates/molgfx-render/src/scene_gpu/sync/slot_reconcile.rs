//! Slot reconciliation: which representations are resident, and what each draws.

use super::super::slot_types::{SlotKey, SlotPlan};
use super::super::slots::GpuSlot;
use super::GpuScene;
use molgfx_core::Scene;
use molgfx_gpu::Device;

impl<D: Device> GpuScene<D> {
    /// Rebuilds the resident slot list when representation membership or the
    /// placed structures change. Surviving slots keep every GPU resource.
    pub(super) fn reconcile_slots(&mut self, scene: &Scene) {
        let revision = scene.representation_membership_revision();
        if self.representation_membership_revision == Some(revision)
            && self.slot_structure_revision == Some(scene.structure_revision())
        {
            return;
        }
        self.representation_scratch.clear();
        self.representation_scratch.extend(
            scene
                .representations()
                .filter(|(_, rep)| rep.selection().is_some())
                .map(|(handle, rep)| (rep.order, handle)),
        );
        self.representation_scratch.sort_unstable();
        self.plan_scratch.clear();
        for (_, representation) in &self.representation_scratch {
            for (structure_index, structure) in self.structures.iter().enumerate() {
                let Some(value) = scene.representation(*representation) else {
                    continue;
                };
                let Some(selection_handle) = value.selection() else {
                    continue;
                };
                let Some(selection) = scene.selection_for(selection_handle, structure.handle)
                else {
                    continue;
                };
                let Some(placed) = scene.structure(structure.handle) else {
                    continue;
                };
                if selection.count(placed.atoms.len()) == 0 {
                    continue;
                }
                self.plan_scratch.push(SlotPlan {
                    key: SlotKey {
                        structure: structure.handle,
                        representation: *representation,
                    },
                    structure_index,
                    draw_order: self.plan_scratch.len(),
                    visible: value.visible,
                });
            }
        }
        self.adopt_planned_slots();
        self.representation_membership_revision = Some(revision);
        self.slot_structure_revision = Some(scene.structure_revision());
    }

    /// Keeps the resident slot for every surviving plan, creating the rest.
    fn adopt_planned_slots(&mut self) {
        let mut old = std::mem::take(&mut self.slots);
        old.sort_unstable_by_key(|slot| slot.key);
        self.plan_scratch.sort_unstable_by_key(|plan| plan.key);
        let mut old = old.into_iter().peekable();
        for plan in &self.plan_scratch {
            while old.peek().is_some_and(|slot| slot.key < plan.key) {
                let _ = old.next();
            }
            if old.peek().is_some_and(|slot| slot.key == plan.key) {
                let Some(mut slot) = old.next() else {
                    continue;
                };
                slot.structure_index = plan.structure_index;
                slot.draw_order = plan.draw_order;
                slot.visible = plan.visible;
                self.slots.push(slot);
            } else {
                self.slots.push(GpuSlot::new(*plan));
            }
        }
        self.slots.sort_unstable_by_key(|slot| slot.draw_order);
    }
}
