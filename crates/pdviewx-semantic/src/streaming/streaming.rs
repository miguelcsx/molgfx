//! Deterministic biological LOD and caller-owned out-of-core scheduling.
//!
//! The renderer must not invent a storage format or perform file/network I/O.
//! This module owns the part that is general enough to reuse: it builds a
//! hierarchy of spatial/biological clusters, selects one detail level using a
//! projected screen error, and produces stable chunk requests for the caller's
//! cache. The selected chunks can therefore be backed by mmCIF, `BinaryCIF`,
//! memory maps or a remote service without a second renderer-side policy.

use crate::{LodLevel, LodPolicy};
use pdviewx_core::{PlacedStructure, Scene, StructureHandle};
use pdviewx_math::{Aabb, Camera, Vec3};
use std::collections::HashSet;

#[cfg(test)]
#[path = "streaming_tests.rs"]
mod tests;

/// Stable identity of one biological cluster.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct LodClusterKey {
    /// Owning placed structure.
    pub structure: StructureHandle,
    /// Biological detail represented by the cluster.
    pub level: LodLevel,
    /// Stable row within that level and structure.
    pub index: u32,
}

/// One spatial cluster used by LOD and streaming decisions.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct LodCluster {
    /// Stable cluster identity.
    pub key: LodClusterKey,
    /// World-space bound center.
    pub center: Vec3,
    /// Conservative world-space radius in ångström.
    pub radius: f32,
    /// Semantic multiplier; focused clusters can remain detailed longer.
    pub importance: f32,
    /// Number of source atoms represented by this cluster.
    pub atom_count: u32,
}

/// A complete, immutable biological hierarchy for one scene snapshot.
#[derive(Clone, Debug, Default)]
pub struct LodIndex {
    clusters: Vec<LodCluster>,
}

impl LodIndex {
    /// Builds residue, secondary-structure and domain clusters in stable order.
    ///
    /// The atom coordinates remain borrowed by the core scene. Only compact
    /// bounds and counts are retained here, so rebuilding the index does not
    /// duplicate the coordinate column.
    #[must_use]
    pub fn from_scene(scene: &Scene) -> Self {
        let mut clusters = Vec::new();
        for (structure, placed) in scene.structures() {
            append_structure_clusters(&mut clusters, structure, placed);
        }
        Self { clusters }
    }

    /// Stable cluster records, ordered by structure, level and row.
    #[must_use]
    pub fn clusters(&self) -> &[LodCluster] {
        &self.clusters
    }

    /// Resolves one stable cluster key without rebuilding the hierarchy.
    #[must_use]
    pub fn cluster(&self, key: LodClusterKey) -> Option<&LodCluster> {
        self.clusters.iter().find(|cluster| cluster.key == key)
    }

    /// Returns all clusters at one level without allocating.
    pub fn at_level(&self, level: LodLevel) -> impl Iterator<Item = &LodCluster> {
        self.clusters
            .iter()
            .filter(move |cluster| cluster.key.level == level)
    }

    /// Computes the current detail selection for a camera and viewport.
    #[must_use]
    pub fn select(
        &self,
        camera: &Camera,
        viewport: [u32; 2],
        policy: LodPolicy,
        previous: Option<&LodFrame>,
    ) -> LodFrame {
        let mut visible = Vec::new();
        let mut atom_structures = Vec::new();
        let mut levels = Vec::new();
        for domain in self.at_level(LodLevel::Domain) {
            let Some(previous_level) = previous.and_then(|frame| {
                frame
                    .levels
                    .iter()
                    .find(|(key, _)| key.structure == domain.key.structure)
                    .map(|(_, level)| *level)
            }) else {
                let level = choose_level(domain, self, camera, viewport, policy, LodLevel::Atom);
                emit_level(
                    &mut visible,
                    &mut atom_structures,
                    self,
                    domain.key.structure,
                    level,
                    camera,
                );
                levels.push((domain.key, level));
                continue;
            };
            let level = choose_level(domain, self, camera, viewport, policy, previous_level);
            emit_level(
                &mut visible,
                &mut atom_structures,
                self,
                domain.key.structure,
                level,
                camera,
            );
            levels.push((domain.key, level));
        }
        visible.sort_unstable();
        atom_structures.sort_unstable();
        atom_structures.dedup();
        LodFrame {
            visible,
            atom_structures,
            levels,
        }
    }
}

/// The selected detail for the current frame.
#[derive(Clone, Debug, Default)]
pub struct LodFrame {
    visible: Vec<LodClusterKey>,
    atom_structures: Vec<StructureHandle>,
    levels: Vec<(LodClusterKey, LodLevel)>,
}

impl LodFrame {
    /// Clusters to draw, already sorted for deterministic indirect generation.
    #[must_use]
    pub fn visible(&self) -> &[LodClusterKey] {
        &self.visible
    }

    /// Structures that remain at atom detail and therefore use the native
    /// analytic atom/bond streams.
    #[must_use]
    pub fn atom_structures(&self) -> &[StructureHandle] {
        &self.atom_structures
    }

    /// Selected level for one placed structure.
    #[must_use]
    pub fn level(&self, structure: StructureHandle) -> Option<LodLevel> {
        self.levels
            .iter()
            .find(|(key, _)| key.structure == structure)
            .map(|(_, level)| *level)
    }

    /// Selected level for every indexed structure, in stable index order.
    pub fn levels(&self) -> impl Iterator<Item = (StructureHandle, LodLevel)> + '_ {
        self.levels
            .iter()
            .map(|(key, level)| (key.structure, *level))
    }
}

fn append_structure_clusters(
    output: &mut Vec<LodCluster>,
    structure: StructureHandle,
    placed: &PlacedStructure,
) {
    for residue in 0..placed.hierarchy.residue_count() {
        let range = placed.hierarchy.residue_atoms(residue);
        let Ok(index) = u32::try_from(residue) else {
            continue;
        };
        if let Some(cluster) = cluster_for_range(placed, structure, LodLevel::Residue, index, range)
        {
            output.push(cluster);
        }
    }
    for chain in 0..placed.hierarchy.chain_count() {
        let Ok(index) = u32::try_from(chain) else {
            continue;
        };
        let residues = placed.hierarchy.chain_residues(chain);
        let mut bound = Aabb::EMPTY;
        let mut atom_count = 0u32;
        for residue in residues {
            let range = placed.hierarchy.residue_atoms(residue as usize);
            if let Some(cluster) = cluster_for_range(
                placed,
                structure,
                LodLevel::SecondaryStructure,
                residue,
                range,
            ) {
                bound.extend_sphere(cluster.center, cluster.radius);
                atom_count = atom_count.saturating_add(cluster.atom_count);
            }
        }
        push_bound(
            output,
            LodClusterKey {
                structure,
                level: LodLevel::SecondaryStructure,
                index,
            },
            bound,
            atom_count,
            1.0,
        );
    }
    push_bound(
        output,
        LodClusterKey {
            structure,
            level: LodLevel::Domain,
            index: 0,
        },
        placed.world_aabb(),
        placed.atoms.len(),
        1.0,
    );
}

fn cluster_for_range(
    placed: &PlacedStructure,
    structure: StructureHandle,
    level: LodLevel,
    index: u32,
    range: std::ops::Range<u32>,
) -> Option<LodCluster> {
    let mut bound = Aabb::EMPTY;
    let mut count = 0u32;
    for atom in range {
        let Some(position) = placed.atoms.coords().slice().get(atom as usize) else {
            continue;
        };
        let center = placed
            .model_to_world
            .transform_point3(Vec3::from_array(*position));
        let radius = match placed.atoms.radius().values().get(atom as usize).copied() {
            Some(radius) => radius,
            None => 0.0,
        };
        bound.extend_sphere(center, radius);
        count = count.saturating_add(1);
    }
    (count > 0).then(|| LodCluster {
        key: LodClusterKey {
            structure,
            level,
            index,
        },
        center: bound.center(),
        radius: bound.bounding_sphere().radius.max(0.01),
        importance: 1.0,
        atom_count: count,
    })
}

fn push_bound(
    output: &mut Vec<LodCluster>,
    key: LodClusterKey,
    bound: Aabb,
    atom_count: u32,
    importance: f32,
) {
    if bound.is_empty() || atom_count == 0 {
        return;
    }
    output.push(LodCluster {
        key,
        center: bound.center(),
        radius: bound.bounding_sphere().radius.max(0.01),
        importance,
        atom_count,
    });
}

fn choose_level(
    domain: &LodCluster,
    index: &LodIndex,
    camera: &Camera,
    viewport: [u32; 2],
    policy: LodPolicy,
    previous: LodLevel,
) -> LodLevel {
    let error = projected_radius_pixels(camera, domain.center, domain.radius, viewport) * 2.0;
    let level = policy.select(error, domain.importance, previous);
    if level == LodLevel::Atom {
        return level;
    }
    if level == LodLevel::Residue
        && index.at_level(LodLevel::Residue).all(|cluster| {
            cluster.key.structure != domain.key.structure
                || projected_radius_pixels(camera, cluster.center, cluster.radius, viewport) >= 0.25
        })
    {
        return LodLevel::Residue;
    }
    level
}

fn projected_radius_pixels(camera: &Camera, center: Vec3, radius: f32, viewport: [u32; 2]) -> f32 {
    let clip = camera.view_proj() * center.extend(1.0);
    if !clip.is_finite() || clip.w.abs() <= 1.0e-6 {
        return 0.0;
    }
    let matrix = camera.view_proj();
    let width = f32::from(u16::try_from(viewport[0]).map_or(u16::MAX, |value| value));
    let height = f32::from(u16::try_from(viewport[1]).map_or(u16::MAX, |value| value));
    let x_scale = matrix.x_axis.x.abs() * radius * width;
    let y_scale = matrix.y_axis.y.abs() * radius * height;
    ((x_scale + y_scale) * 0.25 / clip.w.abs()).max(0.0)
}

fn emit_level(
    visible: &mut Vec<LodClusterKey>,
    atom_structures: &mut Vec<StructureHandle>,
    index: &LodIndex,
    structure: StructureHandle,
    level: LodLevel,
    camera: &Camera,
) {
    if level == LodLevel::Atom {
        atom_structures.push(structure);
        return;
    }
    visible.extend(
        index
            .clusters
            .iter()
            .filter(|cluster| {
                cluster.key.structure == structure
                    && cluster.key.level == level
                    && cluster_visible(cluster, camera)
            })
            .map(|cluster| cluster.key),
    );
}

fn cluster_visible(cluster: &LodCluster, camera: &Camera) -> bool {
    camera
        .frustum_planes()
        .into_iter()
        .all(|plane| plane.truncate().dot(cluster.center) + plane.w >= -cluster.radius)
}

/// One caller-owned chunk request produced by the streaming planner.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ChunkRequest {
    /// Requested biological cluster.
    pub key: ChunkKey,
    /// Higher values are retained first under pressure.
    pub priority: f32,
    /// Resident byte estimate supplied by the caller's manifest.
    pub bytes: u64,
}

/// Stable stream key. It contains no file path and performs no I/O.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ChunkKey {
    /// Owning structure.
    pub structure: StructureHandle,
    /// Detail level.
    pub level: LodLevel,
    /// Cluster row.
    pub index: u32,
}

impl From<LodClusterKey> for ChunkKey {
    fn from(key: LodClusterKey) -> Self {
        Self {
            structure: key.structure,
            level: key.level,
            index: key.index,
        }
    }
}

/// Hard cap for caller-provided resident chunks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StreamingBudget {
    /// Maximum resident bytes.
    pub max_resident_bytes: u64,
    /// Maximum number of new requests emitted in one frame.
    pub max_requests_per_frame: usize,
}

impl Default for StreamingBudget {
    fn default() -> Self {
        Self {
            max_resident_bytes: 256 * 1024 * 1024,
            max_requests_per_frame: 64,
        }
    }
}

/// The result of one deterministic residency decision.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct StreamPlan {
    /// Chunks the caller should load or keep hot.
    pub retain: Vec<ChunkRequest>,
    /// Previously resident chunks that can be evicted.
    pub evict: Vec<ChunkKey>,
    /// Bytes retained after the decision.
    pub resident_bytes: u64,
}

/// Stable, allocation-reusing residency planner.
#[derive(Clone, Debug)]
pub struct StreamPlanner {
    budget: StreamingBudget,
    resident: Vec<ChunkRequest>,
}

impl StreamPlanner {
    /// Creates a planner with a bounded resident cache.
    #[must_use]
    pub fn new(budget: StreamingBudget) -> Self {
        Self {
            budget,
            resident: Vec::new(),
        }
    }

    /// Current budget.
    #[must_use]
    pub const fn budget(&self) -> StreamingBudget {
        self.budget
    }

    /// Reconciles visible requests with the bounded resident set.
    #[must_use]
    pub fn plan(&mut self, requests: &[ChunkRequest]) -> StreamPlan {
        let mut ordered = requests.to_vec();
        ordered.retain(|request| request.bytes > 0 && request.priority.is_finite());
        ordered.sort_unstable_by(|left, right| {
            right
                .priority
                .total_cmp(&left.priority)
                .then_with(|| left.key.cmp(&right.key))
        });
        let resident_keys = self
            .resident
            .iter()
            .map(|request| request.key)
            .collect::<HashSet<_>>();
        let mut selected_keys = HashSet::with_capacity(ordered.len());
        let mut retain = Vec::new();
        let mut bytes = 0u64;
        let mut new_requests = 0usize;
        for request in ordered.iter().copied() {
            if selected_keys.contains(&request.key)
                || bytes.saturating_add(request.bytes) > self.budget.max_resident_bytes
            {
                continue;
            }
            if !resident_keys.contains(&request.key) {
                if new_requests >= self.budget.max_requests_per_frame {
                    continue;
                }
                new_requests += 1;
            }
            selected_keys.insert(request.key);
            bytes = bytes.saturating_add(request.bytes);
            retain.push(request);
        }
        let mut evict = self
            .resident
            .iter()
            .filter(|old| !selected_keys.contains(&old.key))
            .map(|old| old.key)
            .collect::<Vec<_>>();
        evict.sort_unstable();
        self.resident.clone_from(&retain);
        StreamPlan {
            retain,
            evict,
            resident_bytes: bytes,
        }
    }
}
