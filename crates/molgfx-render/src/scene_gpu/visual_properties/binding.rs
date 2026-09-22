//! Arena offsets handed to visual programs.
use molgfx_core::{RowDomain, Scene, StructureHandle, VisualStyle};
use molgfx_gpu::Device;

use super::upload::reference_matches_domain;
use super::{AttributeArenaBinding, VisualPropertyTable};

impl<D: Device> VisualPropertyTable<D> {
    pub(in crate::scene_gpu) fn offsets(
        &self,
        scene: &Scene,
        structure: StructureHandle,
        style: Option<&VisualStyle>,
    ) -> AttributeArenaBinding {
        self.offsets_for_domain(scene, RowDomain::Atoms(structure), style)
    }

    pub(in crate::scene_gpu) fn state_offset(&self, structure: StructureHandle) -> u32 {
        self.states
            .binary_search_by_key(&structure, |column| column.structure)
            .ok()
            .and_then(|index| self.states.get(index))
            .map_or(1, |column| column.offset)
    }

    pub(in crate::scene_gpu) fn offsets_for_domain(
        &self,
        scene: &Scene,
        domain: RowDomain,
        style: Option<&VisualStyle>,
    ) -> AttributeArenaBinding {
        let mut binding = AttributeArenaBinding::default();
        let Some(style) = style else { return binding };
        for (slot, reference) in style.program().attributes().iter().enumerate() {
            if slot == binding.offsets.len() {
                break;
            }
            if !reference_matches_domain(scene, domain, *reference) {
                continue;
            }
            let Ok(index) = self
                .columns
                .binary_search_by_key(reference, |column| column.reference)
            else {
                continue;
            };
            let column = self.columns[index];
            binding.offsets[slot] = crate::fallback(column.materialized_offset, column.offset);
            binding.layouts[slot] = column.stride_words
                | ((column.reference.kind() as u32) << 8)
                | (u32::from(column.temporal && column.materialized_offset.is_none()) << 16);
        }
        binding
    }
}
