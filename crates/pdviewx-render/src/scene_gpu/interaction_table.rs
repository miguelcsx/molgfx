//! One revision-diffed GPU table for all visible molecular interactions.

use super::buffers::{count, upload_grow, write_draw_args};
use super::structure::GpuStructure;
use crate::error::RenderError;
use pdviewx_core::{InteractionGpu, Scene};
use pdviewx_gpu::{BindGroupDesc, BindGroupEntry, Device};

#[derive(Debug)]
pub(super) struct GpuInteractions<D: Device> {
    buffer: Option<D::Buffer>,
    args: Option<D::Buffer>,
    group: Option<D::BindGroup>,
    capacity: u64,
    synced: Option<(u64, u64, u64)>,
    count: u32,
    scratch: Vec<InteractionGpu>,
}

impl<D: Device> GpuInteractions<D> {
    pub(super) const fn new() -> Self {
        Self {
            buffer: None,
            args: None,
            group: None,
            capacity: 0,
            synced: None,
            count: 0,
            scratch: Vec::new(),
        }
    }

    pub(super) fn sync(
        &mut self,
        device: &D,
        queue: &D::Queue,
        layout: &D::BindGroupLayout,
        scene: &Scene,
        structures: &[GpuStructure<D>],
    ) -> Result<bool, RenderError> {
        let revision = (
            scene.interaction_revision(),
            scene.structure_revision(),
            scene.guide_revision(),
        );
        if self.synced == Some(revision) {
            return Ok(false);
        }
        self.scratch.clear();
        self.scratch.extend(
            scene
                .interactions()
                .filter(|(_, edge)| edge.visible())
                .filter_map(|(handle, edge)| {
                    let structure_id = structures
                        .iter()
                        .find(|structure| structure.handle == edge.owner())?
                        .structure_id();
                    Some(InteractionGpu::new(
                        edge,
                        Scene::interaction_row(handle),
                        structure_id,
                    ))
                }),
        );
        // Guides share the glyph table: same record, same indirect draw, so a
        // figure's arrows cost no extra pass.
        self.scratch.extend(
            scene
                .guides()
                .filter(|(_, guide)| guide.visible())
                .filter_map(|(handle, guide)| {
                    let structure_id = structures
                        .iter()
                        .find(|structure| structure.handle == guide.owner())?
                        .structure_id();
                    Some(InteractionGpu::from_guide(
                        guide,
                        Scene::guide_row(handle),
                        structure_id,
                    ))
                }),
        );
        let needed = (self.scratch.len() * std::mem::size_of::<InteractionGpu>()) as u64;
        let rebind = self.buffer.is_none() || needed > self.capacity;
        upload_grow(
            device,
            queue,
            "interaction glyph table",
            &self.scratch,
            &mut self.buffer,
            &mut self.capacity,
        )?;
        write_draw_args(
            device,
            queue,
            "interaction glyph indirect arguments",
            6,
            count(self.scratch.len()),
            &mut self.args,
        )?;
        if rebind {
            self.bind(device, layout);
        }
        self.count = count(self.scratch.len());
        self.synced = Some(revision);
        Ok(true)
    }

    fn bind(&mut self, device: &D, layout: &D::BindGroupLayout) {
        let Some(buffer) = &self.buffer else {
            return;
        };
        self.group = Some(device.create_bind_group(&BindGroupDesc {
            label: "group2: interaction glyph table",
            layout,
            entries: &[BindGroupEntry::Buffer { binding: 0, buffer }],
        }));
    }

    pub(super) fn draw(&self) -> Option<(&D::BindGroup, &D::Buffer)> {
        if self.count == 0 {
            return None;
        }
        Some((self.group.as_ref()?, self.args.as_ref()?))
    }

    pub(super) const fn has_visible(&self) -> bool {
        self.count > 0
    }
}
