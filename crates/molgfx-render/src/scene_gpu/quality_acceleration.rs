//! Persistent analytic acceleration over the atoms and bonds actually drawn.
//!
//! One compact hierarchy uses representation radii and compact indices, so a
//! ray can stop at either kind of occluder without walking a second hierarchy.
//! Rebuilding costs O(atoms + bonds) and runs only when packed records change.

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

    /// Resident device bytes of the packed analytic hierarchy.
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
        self.rebuild(device, queue, atoms, bonds, placed)?;
        Ok(())
    }

    fn rebuild(
        &mut self,
        device: &D,
        queue: &D::Queue,
        atoms: &[AtomGpu],
        bonds: &[BondGpu],
        placed: &PlacedStructure,
    ) -> Result<(), RenderError> {
        self.bounds.clear();
        self.bounds
            .extend(atoms.iter().map(|atom| atom_bound(atom, placed)));
        self.bounds
            .extend(bonds.iter().map(|bond| bond_bound(bond, atoms, placed)));
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
            "quality packed analytic hierarchy",
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

fn atom_bound(atom: &AtomGpu, placed: &PlacedStructure) -> Aabb {
    let Some((EntityKind::Atom, source)) = atom.entity_id.unpack() else {
        return Aabb::EMPTY;
    };
    let mut bound = Aabb::EMPTY;
    if let Some(segment) = placed.trajectory() {
        for positions in [segment.start().positions(), segment.end().positions()] {
            extend_endpoint(&mut bound, positions, source, atom.radius.abs());
        }
    } else {
        extend_endpoint(
            &mut bound,
            placed.atoms.coords().slice(),
            source,
            atom.radius.abs(),
        );
    }
    bound
}

fn bond_bound(bond: &BondGpu, atoms: &[AtomGpu], placed: &PlacedStructure) -> Aabb {
    let Some(atom_a) = atoms.get(bond.atom_a as usize) else {
        return Aabb::EMPTY;
    };
    let Some(atom_b) = atoms.get(bond.atom_b as usize) else {
        return Aabb::EMPTY;
    };
    let Some((EntityKind::Atom, source_a)) = atom_a.entity_id.unpack() else {
        return Aabb::EMPTY;
    };
    let Some((EntityKind::Atom, source_b)) = atom_b.entity_id.unpack() else {
        return Aabb::EMPTY;
    };
    primitive_bound(
        &BondPrimitive {
            source_a,
            source_b,
            radius: bond.radius.abs(),
        },
        placed,
    )
}
