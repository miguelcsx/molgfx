impl<D: Device> GpuScene<D> {
    fn reconcile_volume_slots(&mut self, scene: &Scene) {
        let revision = (scene.volume_revision(), scene.representation_revision());
        if self.volume_slot_revision == Some(revision) {
            return;
        }
        self.representation_scratch.clear();
        self.representation_scratch.extend(
            scene
                .representations()
                .filter(|(_, representation)| {
                    representation.visible && representation.volume_handle().is_some()
                })
                .map(|(handle, representation)| (representation.order, handle)),
        );
        self.representation_scratch.sort_unstable();
        self.volume_handle_scratch.clear();
        self.volume_handle_scratch.extend(
            scene
                .representations()
                .filter(|(_, representation)| representation.visible)
                .filter_map(|(_, representation)| {
                    representation
                        .volume_handle()
                        .or_else(|| representation.surface_scalar.map(|overlay| overlay.field))
                }),
        );
        self.volume_handle_scratch.sort_unstable();
        self.volume_handle_scratch.dedup();
        let mut old_resources = std::mem::take(&mut self.volume_resources);
        for handle in &self.volume_handle_scratch {
            if let Some(index) = old_resources
                .iter()
                .position(|resource| resource.handle == *handle)
            {
                self.volume_resources.push(old_resources.swap_remove(index));
            } else {
                self.volume_resources.push(GpuVolumeResource::new(*handle));
            }
        }
        let mut old = std::mem::take(&mut self.volume_slots);
        for (_, handle) in &self.representation_scratch {
            if let Some(index) = old.iter().position(|slot| slot.representation == *handle) {
                self.volume_slots.push(old.swap_remove(index));
            } else {
                self.volume_slots.push(GpuVolumeSlot::new(*handle));
            }
        }
        self.volume_slot_revision = Some(revision);
    }
}
