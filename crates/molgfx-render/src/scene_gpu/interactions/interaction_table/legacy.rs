//! Static scientific glyphs change coverage only when their molecular inputs change.
use super::{GpuInteractions, GpuStructure, RenderError};
use molgfx_core::{EntityKind, InteractionGpu, Scene, StructureHandle};
use molgfx_gpu::Device;
use std::hash::{Hash, Hasher};

/// Rebuilds dependencies only after authoring edits; each frame visits O(dependent models).
#[derive(Debug, Default)]
pub(super) struct CoordinateScopes {
    key: Option<(u64, u64)>,
    dependencies: Vec<StructureHandle>,
}

impl CoordinateScopes {
    pub(super) const fn new() -> Self {
        Self {
            key: None,
            dependencies: Vec::new(),
        }
    }

    pub(super) fn revision(&mut self, scene: &Scene) -> u64 {
        let key = (scene.cache_identity(), scene.interaction_revision());
        if self.key != Some(key) {
            self.dependencies.clear();
            self.dependencies
                .extend(scene.interactions().flat_map(|(_, edge)| {
                    edge.static_coordinates()
                        .into_iter()
                        .flatten()
                        .map(|(handle, _)| handle)
                }));
            self.dependencies.sort_unstable();
            self.dependencies.dedup();
            self.key = Some(key);
        }
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        for handle in &self.dependencies {
            handle.hash(&mut hash);
            if let Some(placed) = scene.structure(*handle) {
                placed.source.identity().hash(&mut hash);
                placed.source.coordinate_revision().hash(&mut hash);
                placed.trajectory().is_some().hash(&mut hash);
                placed
                    .model_to_world
                    .to_cols_array()
                    .map(f32::to_bits)
                    .hash(&mut hash);
            }
        }
        hash.finish()
    }
}

impl<D: Device> GpuInteractions<D> {
    pub(super) fn pack_legacy(
        &mut self,
        scene: &Scene,
        structures: &[GpuStructure<D>],
    ) -> Result<(), RenderError> {
        for (handle, edge) in scene
            .interactions()
            .filter(|(_, edge)| edge.visible() && edge.coordinates_match(scene))
        {
            let Some(page) = structures
                .iter()
                .find(|structure| structure.handle == edge.owner())
                .map(|structure| structure.pick_page(EntityKind::Edge))
            else {
                continue;
            };
            self.scratch.push(InteractionGpu::new(
                edge,
                u64::from(Scene::interaction_row(handle)),
                page,
            )?);
        }
        for (handle, guide) in scene.guides().filter(|(_, guide)| guide.visible()) {
            let Some(page) = structures
                .iter()
                .find(|structure| structure.handle == guide.owner())
                .map(|structure| structure.pick_page(EntityKind::Guide))
            else {
                continue;
            };
            self.scratch.push(InteractionGpu::from_guide(
                guide,
                u64::from(Scene::guide_row(handle)),
                page,
            )?);
        }
        Ok(())
    }
}
