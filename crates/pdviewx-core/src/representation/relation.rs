//! Generic spatial relations partitioned once into homogeneous anchor layouts.

#[cfg(test)]
#[path = "relation_tests.rs"]
mod tests;

use crate::{CoreError, InstanceBatchHandle, RowDomain, RowEntityRef, SourceRows};
use pdviewx_math::{Rgba8, Vec3};
use std::ops::Range;
use std::sync::Arc;

const ANCHOR_LAYOUT_COUNT: usize = 5;
const RELATION_LAYOUT_COUNT: usize = ANCHOR_LAYOUT_COUNT * ANCHOR_LAYOUT_COUNT;

/// Spatial endpoint that cannot target another relation.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SpatialAnchor {
    /// Immutable caller-supplied world position.
    World(Vec3),
    /// Entity whose current position is resolved by the GPU.
    Entity(RowEntityRef),
    /// One shared template part placed by one exact rigid instance.
    TemplatePart(TemplatePartRef),
}

/// Exact spatial occurrence of a shared analytic template part.
///
/// A template part alone is not spatial: the same local part can occur in
/// every transform row of an instance batch. Keeping both rows prevents
/// ambiguous anchors and picking identities.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TemplatePartRef {
    batch: InstanceBatchHandle,
    instance_row: u32,
    part_row: u32,
}

impl TemplatePartRef {
    /// Creates an unchecked occurrence. Scene insertion validates both rows and
    /// the batch generation atomically.
    #[must_use]
    pub const fn new(batch: InstanceBatchHandle, instance_row: u32, part_row: u32) -> Self {
        Self {
            batch,
            instance_row,
            part_row,
        }
    }

    /// Shared instance batch.
    #[must_use]
    pub const fn batch(self) -> InstanceBatchHandle {
        self.batch
    }

    /// Transform row placing the shared template.
    #[must_use]
    pub const fn instance_row(self) -> u32 {
        self.instance_row
    }

    /// Sphere-first local template part row.
    #[must_use]
    pub const fn part_row(self) -> u32 {
        self.part_row
    }
}

impl SpatialAnchor {
    /// Creates a finite world-space endpoint.
    ///
    /// # Errors
    ///
    /// World coordinates must be finite.
    pub fn world(position: Vec3) -> Result<Self, CoreError> {
        if !position.is_finite() {
            return Err(invalid("relation world anchor must be finite"));
        }
        Ok(Self::World(position))
    }

    /// Creates a dynamic endpoint only for spatial row domains.
    ///
    /// # Errors
    ///
    /// Relation rows are not spatial and cannot form chains or cycles.
    pub fn entity(entity: RowEntityRef) -> Result<Self, CoreError> {
        if matches!(entity.domain(), RowDomain::TemplateParts(_)) {
            return Err(invalid(
                "template part anchors require an instance row and a part row",
            ));
        }
        if !entity.domain().is_spatial() {
            return Err(invalid("relations cannot anchor to relation rows"));
        }
        Ok(Self::Entity(entity))
    }

    /// Creates an exact dynamic occurrence of a shared template part.
    #[must_use]
    pub const fn template_part(reference: TemplatePartRef) -> Self {
        Self::TemplatePart(reference)
    }

    /// Homogeneous lowering layout.
    #[must_use]
    pub const fn layout(self) -> AnchorLayout {
        match self {
            Self::World(_) => AnchorLayout::World,
            Self::Entity(entity) => match entity.domain() {
                RowDomain::Atoms(_) => AnchorLayout::Atom,
                RowDomain::Points(_) => AnchorLayout::Point,
                RowDomain::Instances(_) => AnchorLayout::Instance,
                RowDomain::TemplateParts(_) => AnchorLayout::TemplatePart,
                RowDomain::Relations(_) => AnchorLayout::World,
            },
            Self::TemplatePart(_) => AnchorLayout::TemplatePart,
        }
    }

    /// Whether coordinates, transforms or timeline can move this endpoint.
    #[must_use]
    pub const fn is_dynamic(self) -> bool {
        !matches!(self, Self::World(_))
    }

    /// Spatial source table used by branch-free GPU lowering.
    ///
    /// Template-part occurrences use their instance batch as the dynamic
    /// source because the immutable local part center is packed with the
    /// relation resolver row.
    #[must_use]
    pub const fn source_domain(self) -> Option<RowDomain> {
        match self {
            Self::World(_) => None,
            Self::Entity(entity) => Some(entity.domain()),
            Self::TemplatePart(reference) => Some(RowDomain::Instances(reference.batch())),
        }
    }
}

/// Endpoint layout used to select one branch-free compute kernel.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum AnchorLayout {
    /// Already resolved world coordinate.
    World = 0,
    /// Current structure atom coordinate.
    Atom = 1,
    /// Generic point row.
    Point = 2,
    /// Rigid instance origin.
    Instance = 3,
    /// One shared template part transformed by an instance.
    TemplatePart = 4,
}

/// Ordered start/end layout for one homogeneous relation stream.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct RelationLayout {
    /// Start endpoint layout.
    pub start: AnchorLayout,
    /// End endpoint layout.
    pub end: AnchorLayout,
}

impl RelationLayout {
    const fn index(self) -> usize {
        self.start as usize * ANCHOR_LAYOUT_COUNT + self.end as usize
    }

    const fn from_index(index: usize) -> Self {
        let start = anchor_from_index(index / ANCHOR_LAYOUT_COUNT);
        let end = anchor_from_index(index % ANCHOR_LAYOUT_COUNT);
        Self { start, end }
    }

    /// Whether this stream requires endpoint resolution after state changes.
    #[must_use]
    pub const fn is_dynamic(self) -> bool {
        !matches!(
            (self.start, self.end),
            (AnchorLayout::World, AnchorLayout::World)
        )
    }
}

/// Purely visual line pattern with no scientific interpretation.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum RelationPattern {
    /// Continuous analytic line or capsule.
    #[default]
    Solid = 0,
    /// Repeating dash pattern.
    Dashed = 1,
    /// Repeating dot pattern.
    Dotted = 2,
    /// Repeating helical spring pattern.
    Spring = 3,
}

/// Batch-wide visual fallback. Per-row variation is supplied by attributes.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RelationStyle {
    /// Physical-pixel width.
    pub width_pixels: f32,
    /// Packed linear-display fallback color.
    pub color: Rgba8,
    /// Coverage opacity.
    pub opacity: f32,
    /// Repeating visual pattern.
    pub pattern: RelationPattern,
    /// Screen-space distance trimmed from the start and end anchors.
    ///
    /// This lets a generic stroke meet the visible boundary of anchored
    /// glyphs instead of crossing their projected interiors.
    pub endpoint_insets_pixels: [f32; 2],
    /// Keeps the connector behind opaque scene geometry instead of interpolating depth.
    pub depth_behind_anchors: bool,
}

impl Default for RelationStyle {
    fn default() -> Self {
        Self {
            width_pixels: 1.5,
            color: Rgba8::WHITE,
            opacity: 1.0,
            pattern: RelationPattern::Solid,
            endpoint_insets_pixels: [0.0; 2],
            depth_behind_anchors: false,
        }
    }
}

/// One logical caller relation.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Relation {
    /// Start endpoint.
    pub start: SpatialAnchor,
    /// End endpoint.
    pub end: SpatialAnchor,
}

impl Relation {
    /// Ordered homogeneous layout.
    #[must_use]
    pub const fn layout(self) -> RelationLayout {
        RelationLayout {
            start: self.start.layout(),
            end: self.end.layout(),
        }
    }
}

/// Contiguous range in the optional logical-row remap.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RelationPartition {
    /// Branch-free endpoint layout.
    pub layout: RelationLayout,
    /// Rows in partitioned stream order.
    pub rows: Range<u32>,
}

/// Maximum referenced row for one external spatial domain.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RelationDependency {
    /// Referenced spatial table.
    pub domain: RowDomain,
    /// Highest row referenced in that table.
    pub maximum_row: u32,
}

/// Immutable generic relation payload with a precomputed stable partition.
#[derive(Clone, PartialEq, Debug)]
pub struct RelationBatch {
    relations: Arc<[Relation]>,
    source_rows: SourceRows,
    style: RelationStyle,
    partitions: Arc<[RelationPartition]>,
    remap: Option<Arc<[u32]>>,
    dependencies: Arc<[RelationDependency]>,
    visible: bool,
}

impl RelationBatch {
    /// Validates and partitions mixed anchors in stable logical-row order.
    ///
    /// The plan costs `O(n)` once. A remap is retained only when grouping
    /// changes row order; already homogeneous input adds no row-sized table.
    ///
    /// # Errors
    ///
    /// Relations must be non-empty and source-aligned; style values must be
    /// finite and bounded.
    pub fn new(
        relations: Arc<[Relation]>,
        source_rows: SourceRows,
        style: RelationStyle,
    ) -> Result<Self, CoreError> {
        if relations.is_empty() || relations.len() != source_rows.len() as usize {
            return Err(invalid(
                "relations must be non-empty and match their source rows",
            ));
        }
        if !style.width_pixels.is_finite()
            || style.width_pixels <= 0.0
            || !style.opacity.is_finite()
            || !(0.0..=1.0).contains(&style.opacity)
            || style
                .endpoint_insets_pixels
                .iter()
                .any(|value| !value.is_finite() || *value < 0.0)
        {
            return Err(invalid("relation style must be finite and bounded"));
        }
        let (partitions, remap) = partition_relations(&relations)?;
        let dependencies = relation_dependencies(&relations);
        Ok(Self {
            relations,
            source_rows,
            style,
            partitions,
            remap,
            dependencies,
            visible: true,
        })
    }

    /// Logical relations in caller order.
    #[must_use]
    pub const fn relations(&self) -> &Arc<[Relation]> {
        &self.relations
    }

    /// External logical-row identity.
    #[must_use]
    pub const fn source_rows(&self) -> &SourceRows {
        &self.source_rows
    }

    /// Batch-wide fallback style.
    #[must_use]
    pub const fn style(&self) -> RelationStyle {
        self.style
    }

    /// Homogeneous stream partitions in deterministic layout order.
    #[must_use]
    pub const fn partitions(&self) -> &Arc<[RelationPartition]> {
        &self.partitions
    }

    /// Partitioned-row to logical-row mapping, absent when identity ordered.
    #[must_use]
    pub const fn remap(&self) -> Option<&Arc<[u32]>> {
        self.remap.as_ref()
    }

    /// Deduplicated domains validated during the scene's atomic insertion.
    #[must_use]
    pub const fn dependencies(&self) -> &Arc<[RelationDependency]> {
        &self.dependencies
    }

    /// Current scene visibility.
    #[must_use]
    pub const fn visible(&self) -> bool {
        self.visible
    }

    pub(crate) fn set_visible(&mut self, visible: bool) -> bool {
        if self.visible == visible {
            return false;
        }
        self.visible = visible;
        true
    }
}

type PartitionPlan = (Arc<[RelationPartition]>, Option<Arc<[u32]>>);

fn partition_relations(relations: &[Relation]) -> Result<PartitionPlan, CoreError> {
    let mut counts = [0u32; RELATION_LAYOUT_COUNT];
    for relation in relations {
        let slot = relation.layout().index();
        counts[slot] = counts[slot]
            .checked_add(1)
            .ok_or_else(|| invalid("relation partition exceeds u32"))?;
    }
    let mut starts = [0u32; RELATION_LAYOUT_COUNT];
    let mut total = 0u32;
    let mut partitions = Vec::new();
    for (index, count) in counts.iter().copied().enumerate() {
        starts[index] = total;
        if count != 0 {
            let end = total
                .checked_add(count)
                .ok_or_else(|| invalid("relation partition exceeds u32"))?;
            partitions.push(RelationPartition {
                layout: RelationLayout::from_index(index),
                rows: total..end,
            });
            total = end;
        }
    }
    let mut cursors = starts;
    let mut remap = vec![0u32; relations.len()];
    for (logical, relation) in relations.iter().enumerate() {
        let slot = relation.layout().index();
        let output = cursors[slot] as usize;
        remap[output] =
            u32::try_from(logical).map_err(|_| invalid("relation logical row exceeds u32"))?;
        cursors[slot] = cursors[slot].saturating_add(1);
    }
    let identity = remap
        .iter()
        .enumerate()
        .all(|(row, logical)| usize::try_from(*logical) == Ok(row));
    Ok((
        partitions.into(),
        (!identity).then(|| Arc::from(remap.into_boxed_slice())),
    ))
}

fn relation_dependencies(relations: &[Relation]) -> Arc<[RelationDependency]> {
    let mut rows = std::collections::BTreeMap::<RowDomain, u32>::new();
    for anchor in relations
        .iter()
        .flat_map(|relation| [relation.start, relation.end])
    {
        match anchor {
            SpatialAnchor::World(_) => {}
            SpatialAnchor::Entity(entity) => {
                add_dependency(&mut rows, entity.domain(), entity.row());
            }
            SpatialAnchor::TemplatePart(reference) => {
                add_dependency(
                    &mut rows,
                    RowDomain::Instances(reference.batch()),
                    reference.instance_row(),
                );
                add_dependency(
                    &mut rows,
                    RowDomain::TemplateParts(reference.batch()),
                    reference.part_row(),
                );
            }
        }
    }
    rows.into_iter()
        .map(|(domain, maximum_row)| RelationDependency {
            domain,
            maximum_row,
        })
        .collect::<Vec<_>>()
        .into()
}

fn add_dependency(
    rows: &mut std::collections::BTreeMap<RowDomain, u32>,
    domain: RowDomain,
    row: u32,
) {
    rows.entry(domain)
        .and_modify(|maximum| *maximum = (*maximum).max(row))
        .or_insert(row);
}

const fn anchor_from_index(index: usize) -> AnchorLayout {
    match index {
        0 => AnchorLayout::World,
        1 => AnchorLayout::Atom,
        2 => AnchorLayout::Point,
        3 => AnchorLayout::Instance,
        _ => AnchorLayout::TemplatePart,
    }
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidBatch { reason }
}
