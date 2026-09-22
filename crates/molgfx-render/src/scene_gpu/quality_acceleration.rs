//! Persistent bond acceleration used by publication-quality ray traversal.

use super::buffers::upload_grow;
use crate::error::RenderError;
use molgfx_core::{AtomGpu, BondGpu, EntityKind, PlacedStructure};
use molgfx_gpu::Device;
use molgfx_math::{Aabb, Bvh, BvhBuildScratch, Vec3};

#[cfg(test)]
#[path = "quality_acceleration_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug)]
struct BondPrimitive {
    source_a: u32,
    source_b: u32,
    radius: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct HierarchyCounts {
    pub(super) nodes: u32,
    pub(super) indices: u32,
}

#[derive(Debug)]
pub(super) struct QualityAcceleration<D: Device> {
    nodes: Option<D::Buffer>,
    nodes_capacity: u64,
    primitives: Vec<BondPrimitive>,
    bounds: Vec<Aabb>,
    hierarchy: Bvh,
    build_scratch: BvhBuildScratch,
    upload_indices: Vec<u32>,
    upload_words: Vec<u32>,
    counts: HierarchyCounts,
}

impl<D: Device> QualityAcceleration<D> {
    pub(super) fn new() -> Self {
        Self {
            nodes: None,
            nodes_capacity: 0,
            primitives: Vec::new(),
            bounds: Vec::new(),
            hierarchy: Bvh::default(),
            build_scratch: BvhBuildScratch::default(),
            upload_indices: Vec::new(),
            upload_words: Vec::new(),
            counts: HierarchyCounts::default(),
        }
    }

    pub(super) fn nodes(&self) -> Option<&D::Buffer> {
        self.nodes.as_ref()
    }

    pub(super) const fn counts(&self) -> HierarchyCounts {
        self.counts
    }

    /// Resident device bytes of the packed bond hierarchy.
    #[must_use]
    pub(super) fn resident_bytes(&self) -> u64 {
        self.nodes_capacity
    }

    pub(super) fn sync_topology(
        &mut self,
        device: &D,
        queue: &D::Queue,
        atoms: &[AtomGpu],
        bonds: &[BondGpu],
        placed: &PlacedStructure,
    ) -> Result<(), RenderError> {
        self.primitives.clear();
        self.primitives.extend(bonds.iter().filter_map(|bond| {
            let atom_a = atoms.get(bond.atom_a as usize)?;
            let atom_b = atoms.get(bond.atom_b as usize)?;
            let (kind_a, source_a) = atom_a.entity_id.unpack()?;
            let (kind_b, source_b) = atom_b.entity_id.unpack()?;
            (kind_a == EntityKind::Atom && kind_b == EntityKind::Atom).then_some(BondPrimitive {
                source_a,
                source_b,
                radius: bond.radius.abs(),
            })
        }));
        self.rebuild(device, queue, bonds, placed)?;
        Ok(())
    }

    fn rebuild(
        &mut self,
        device: &D,
        queue: &D::Queue,
        bonds: &[BondGpu],
        placed: &PlacedStructure,
    ) -> Result<(), RenderError> {
        self.bounds.clear();
        self.bounds.extend(
            self.primitives
                .iter()
                .map(|primitive| primitive_bound(primitive, placed)),
        );
        self.hierarchy
            .rebuild(&self.bounds, &mut self.build_scratch)?;
        self.upload_indices.clear();
        if self.hierarchy.nodes.is_empty() {
            self.nodes = None;
            self.nodes_capacity = 0;
            self.counts = HierarchyCounts::default();
            return Ok(());
        }
        self.upload_indices
            .extend_from_slice(&self.hierarchy.primitive_indices);
        self.upload_indices
            .extend_from_slice(&self.hierarchy.escape);
        self.counts = hierarchy_counts(&self.hierarchy)?;
        self.upload_words.clear();
        self.upload_words
            .extend_from_slice(bytemuck::cast_slice(&self.hierarchy.nodes));
        self.upload_words.extend_from_slice(&self.upload_indices);
        self.upload_words
            .extend_from_slice(bytemuck::cast_slice(bonds));
        upload_grow(
            device,
            queue,
            "quality packed bond hierarchy",
            &self.upload_words,
            &mut self.nodes,
            &mut self.nodes_capacity,
        )
    }
}

pub(super) fn hierarchy_counts(hierarchy: &Bvh) -> Result<HierarchyCounts, RenderError> {
    let index_count = hierarchy
        .primitive_indices
        .len()
        .checked_add(hierarchy.escape.len())
        .ok_or(molgfx_gpu::GpuError::LimitExceeded {
            resource: "quality hierarchy index count",
            limit: u64::from(u32::MAX),
        })?;
    let nodes =
        u32::try_from(hierarchy.nodes.len()).map_err(|_| molgfx_gpu::GpuError::LimitExceeded {
            resource: "quality hierarchy node count",
            limit: u64::from(u32::MAX),
        })?;
    let indices = u32::try_from(index_count).map_err(|_| molgfx_gpu::GpuError::LimitExceeded {
        resource: "quality hierarchy index count",
        limit: u64::from(u32::MAX),
    })?;
    Ok(HierarchyCounts { nodes, indices })
}

fn primitive_bound(primitive: &BondPrimitive, placed: &PlacedStructure) -> Aabb {
    let mut bound = Aabb::EMPTY;
    if let Some(segment) = placed.trajectory() {
        for positions in [segment.start().positions(), segment.end().positions()] {
            extend_endpoint(&mut bound, positions, primitive.source_a, primitive.radius);
            extend_endpoint(&mut bound, positions, primitive.source_b, primitive.radius);
        }
    } else {
        let positions = placed.atoms.coords().slice();
        extend_endpoint(&mut bound, positions, primitive.source_a, primitive.radius);
        extend_endpoint(&mut bound, positions, primitive.source_b, primitive.radius);
    }
    bound
}

fn extend_endpoint(bound: &mut Aabb, positions: &[[f32; 3]], source: u32, radius: f32) {
    if let Some(position) = positions.get(source as usize).copied() {
        bound.extend_sphere(Vec3::from_array(position), radius);
    }
}
