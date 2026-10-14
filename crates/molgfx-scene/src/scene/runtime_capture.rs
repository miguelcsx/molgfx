use crate::{RuntimeBindings, Scene, SceneSpec};

impl Scene {
    /// Captures immutable scientific owners and authored metadata for an off-thread archive.
    /// Runtime capture costs O(descriptors), independent of atom and vertex counts.
    #[must_use]
    pub fn capture_runtime(&self) -> crate::snapshot::RuntimeCapture {
        let mut capture = self.capture_runtime_spec(&self.spec);
        capture.bindings.playback = self.playback_bindings().collect();
        capture
    }

    /// Includes assets still reachable through named snapshots in one shared frame archive.
    /// Capture clones descriptors and owners; bulk data is materialized on a worker.
    #[must_use]
    pub fn capture_runtime_with_snapshots<'a>(
        &self,
        snapshots: impl IntoIterator<Item = &'a crate::snapshot::SceneSnapshot>,
    ) -> crate::snapshot::RuntimeCapture {
        let mut capture = self.capture_runtime();
        capture.retained = snapshots
            .into_iter()
            .map(|snapshot| self.capture_runtime_spec(&snapshot.scene))
            .collect();
        capture
    }

    pub(crate) fn capture_runtime_spec(&self, spec: &SceneSpec) -> crate::snapshot::RuntimeCapture {
        crate::snapshot::RuntimeCapture {
            scene: spec.clone(),
            properties: spec
                .properties
                .iter()
                .filter_map(|(name, descriptor)| {
                    self.snapshot_property_binding(name, descriptor)
                        .map(|binding| (name.clone(), binding))
                })
                .collect(),
            retained: Vec::new(),
            bindings: RuntimeBindings {
                volumes: spec
                    .volumes
                    .values()
                    .filter_map(|spec| {
                        self.overlay_bindings
                            .volume(&spec.source.content_hash)
                            .cloned()
                    })
                    .collect(),
                hydrogen_bonds: spec
                    .interactions
                    .values()
                    .filter_map(|spec| {
                        let crate::InteractionSpec::HydrogenBonds(spec) = spec else {
                            return None;
                        };
                        self.overlay_bindings
                            .hydrogen_bonds(&spec.source.content_hash)
                            .cloned()
                    })
                    .collect(),
                molecular_surfaces: spec
                    .representations
                    .values()
                    .filter_map(|spec| {
                        let (source, _, _) = spec.molecular_surface_recipe()?;
                        self.overlay_bindings
                            .molecular_surface(&source.content_hash, spec.structure_id()?)
                            .cloned()
                    })
                    .collect(),
                segmentations: spec
                    .segmentations
                    .values()
                    .filter_map(|spec| {
                        self.overlay_bindings
                            .segmentation(&spec.source.content_hash)
                            .cloned()
                    })
                    .collect(),
                trajectories: spec
                    .trajectories
                    .values()
                    .filter_map(|spec| {
                        self.overlay_bindings
                            .trajectory(&spec.source.content_hash)
                            .cloned()
                    })
                    .collect(),
                playback: Vec::new(),
            },
        }
    }
}

impl Scene {
    pub(crate) fn absorb_archive_assets(&mut self, retained: Self) -> Result<(), crate::Error> {
        self.overlay_bindings.absorb(retained.overlay_bindings)?;
        for (name, binding) in retained.property_bindings {
            if let Some(existing) = self.property_bindings.get(&name) {
                if existing.spec().source.content_hash != binding.spec().source.content_hash {
                    self.retain_snapshot_property(binding)?;
                    continue;
                }
                if existing.spec().structure != binding.spec().structure
                    || existing.values().len() != binding.values().len()
                    || !existing
                        .values()
                        .iter()
                        .zip(binding.values().iter())
                        .all(|(a, b)| a.to_bits() == b.to_bits())
                {
                    return Err(crate::Error::InvalidSpec(
                        "retained scalar property contradicts its current input".to_owned(),
                    ));
                }
            } else {
                self.retain_snapshot_property(binding)?;
            }
        }
        Ok(())
    }
}
