//! Deterministic biological LOD and caller-owned out-of-core scheduling.
//!
//! The renderer must not invent a storage format or perform file/network I/O.
//! This module owns the part that is general enough to reuse: it builds a
//! hierarchy of spatial/biological clusters, selects one detail level using a
//! projected screen error, and produces stable chunk requests for the caller's
//! cache. The selected chunks can therefore be backed by mmCIF, `BinaryCIF`,
//! memory maps or a remote service without a second renderer-side policy.
//! Index construction is `O(atoms + residues log residues)`; selection is
//! `O(clusters + visible clusters)` and can reuse all output storage.

use crate::{LodLevel, LodPolicy};
use pdviewx_core::{PlacedStructure, Scene, StructureHandle};
use pdviewx_math::{Aabb, Camera, Vec3};

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
        let capacity = scene.structures().fold(0usize, |count, (_, placed)| {
            count
                .saturating_add(placed.hierarchy.residue_count().saturating_mul(2))
                .saturating_add(1)
        });
        let mut clusters = Vec::with_capacity(capacity);
        for (structure, placed) in scene.structures() {
            append_structure_clusters(&mut clusters, structure, placed);
        }
        debug_assert!(clusters.windows(2).all(|rows| rows[0].key < rows[1].key));
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
        self.clusters
            .binary_search_by_key(&key, |cluster| cluster.key)
            .ok()
            .map(|row| &self.clusters[row])
    }

    /// Returns all clusters at one level without allocating.
    pub fn at_level(&self, level: LodLevel) -> impl Iterator<Item = &LodCluster> {
        self.clusters
            .iter()
            .filter(move |cluster| cluster.key.level == level)
    }

    fn clusters_for(&self, structure: StructureHandle, level: LodLevel) -> &[LodCluster] {
        let target = (structure, level);
        let start = self
            .clusters
            .partition_point(|cluster| (cluster.key.structure, cluster.key.level) < target);
        let count = self.clusters[start..]
            .partition_point(|cluster| (cluster.key.structure, cluster.key.level) == target);
        &self.clusters[start..start + count]
    }

    /// Computes the current detail selection into reusable caller storage.
    ///
    /// Once the three output columns have reached their high-water marks,
    /// repeated selection does not allocate. Keep two frames when hysteresis
    /// needs the preceding result: one is read as `previous` while the other is
    /// cleared and written as `output`.
    pub fn select_into(
        &self,
        camera: &Camera,
        viewport: [u32; 2],
        policy: LodPolicy,
        previous: Option<&LodFrame>,
        output: &mut LodFrame,
    ) {
        output.visible.clear();
        output.atom_structures.clear();
        output.levels.clear();
        for domain in self.at_level(LodLevel::Domain) {
            let previous_level = match previous.and_then(|frame| frame.level(domain.key.structure))
            {
                Some(level) => level,
                None => LodLevel::Atom,
            };
            let level = choose_level(domain, self, camera, viewport, policy, previous_level);
            emit_level(
                &mut output.visible,
                &mut output.atom_structures,
                self,
                domain.key.structure,
                level,
                camera,
            );
            output.levels.push((domain.key, level));
        }
        debug_assert!(output.visible.windows(2).all(|rows| rows[0] < rows[1]));
        debug_assert!(
            output
                .atom_structures
                .windows(2)
                .all(|rows| rows[0] < rows[1])
        );
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
            .binary_search_by_key(&structure, |(key, _)| key.structure)
            .ok()
            .map(|row| self.levels[row].1)
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
    let residue_start = output.len();
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
    let residue_end = output.len();
    append_secondary_clusters(output, structure, placed, residue_start..residue_end);
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

fn append_secondary_clusters(
    output: &mut Vec<LodCluster>,
    structure: StructureHandle,
    placed: &PlacedStructure,
    residue_rows: std::ops::Range<usize>,
) {
    let mut segment = 0u32;
    for chain in 0..placed.hierarchy.chain_count() {
        let mut bound = Aabb::EMPTY;
        let mut atom_count = 0u32;
        let mut kind = None;
        for residue in placed.hierarchy.chain_residues(chain) {
            let current = match placed.secondary_structure.values().get(residue as usize) {
                Some(value) => *value,
                None => pdviewx_core::SecondaryStructure::default(),
            };
            if kind.is_some_and(|previous| previous != current) {
                push_secondary(output, structure, segment, bound, atom_count);
                segment = segment.saturating_add(1);
                bound = Aabb::EMPTY;
                atom_count = 0;
            }
            kind = Some(current);
            let key = LodClusterKey {
                structure,
                level: LodLevel::Residue,
                index: residue,
            };
            let Ok(row) =
                output[residue_rows.clone()].binary_search_by_key(&key, |cluster| cluster.key)
            else {
                continue;
            };
            let cluster = output[residue_rows.start + row];
            bound.extend_sphere(cluster.center, cluster.radius);
            atom_count = atom_count.saturating_add(cluster.atom_count);
        }
        push_secondary(output, structure, segment, bound, atom_count);
        segment = segment.saturating_add(1);
    }
}

fn push_secondary(
    output: &mut Vec<LodCluster>,
    structure: StructureHandle,
    index: u32,
    bound: Aabb,
    atom_count: u32,
) {
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
    let pixels = |level| {
        index
            .clusters_for(domain.key.structure, level)
            .iter()
            .map(|cluster| {
                projected_radius_pixels(camera, cluster.center, cluster.radius, viewport) * 2.0
            })
            .fold(0.0_f32, f32::max)
            * domain.importance.clamp(0.25, 4.0)
    };
    choose_hierarchy_level(
        pixels(LodLevel::Residue),
        pixels(LodLevel::SecondaryStructure),
        policy,
        previous,
    )
}

fn choose_hierarchy_level(
    residue_pixels: f32,
    secondary_pixels: f32,
    policy: LodPolicy,
    previous: LodLevel,
) -> LodLevel {
    let candidate = if residue_pixels >= policy.atom_pixels {
        LodLevel::Atom
    } else if residue_pixels >= policy.residue_pixels {
        LodLevel::Residue
    } else if secondary_pixels >= policy.secondary_pixels {
        LodLevel::SecondaryStructure
    } else {
        LodLevel::Domain
    };
    if candidate == previous {
        return previous;
    }
    let boundary = candidate.min(previous);
    let (metric, threshold) = match boundary {
        LodLevel::Atom => (residue_pixels, policy.atom_pixels),
        LodLevel::Residue => (residue_pixels, policy.residue_pixels),
        LodLevel::SecondaryStructure | LodLevel::Domain => {
            (secondary_pixels, policy.secondary_pixels)
        }
    };
    let margin = threshold.max(0.0) * policy.hysteresis.clamp(0.0, 0.49);
    if candidate > previous && metric > threshold - margin
        || candidate < previous && metric < threshold + margin
    {
        previous
    } else {
        candidate
    }
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
            .clusters_for(structure, level)
            .iter()
            .filter(|cluster| cluster_visible(cluster, camera))
            .map(|cluster| cluster.key),
    );
}

fn cluster_visible(cluster: &LodCluster, camera: &Camera) -> bool {
    camera
        .frustum_planes()
        .into_iter()
        .all(|plane| plane.truncate().dot(cluster.center) + plane.w >= -cluster.radius)
}
