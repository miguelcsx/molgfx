//! Compact stackless spatial index over sparse brick metadata.

use super::BrickDescriptor;
use crate::DatasetError;
use core::ops::ControlFlow;

const LEAF_SIZE: usize = 8;

#[derive(Clone, Copy, Debug)]
struct SpatialNode {
    min: [u64; 3],
    max: [u64; 3],
    first: u32,
    count: u32,
    escape: u32,
}

#[derive(Clone, Copy, Debug)]
struct MipRoot {
    mip: u16,
    node: u32,
}

#[derive(Clone, Debug, Default)]
pub(super) struct BrickSpatialIndex {
    nodes: Vec<SpatialNode>,
    roots: Vec<MipRoot>,
}

impl BrickSpatialIndex {
    pub(super) fn build(descriptors: &mut [BrickDescriptor]) -> Result<Self, DatasetError> {
        if u32::try_from(descriptors.len()).is_err() {
            return Err(DatasetError::CatalogTooLarge);
        }
        descriptors.sort_unstable_by_key(|value| value.metadata.address.mip);
        let mut index = Self::default();
        let mut first = 0;
        while first < descriptors.len() {
            let mip = descriptors[first].metadata.address.mip;
            let count =
                descriptors[first..].partition_point(|value| value.metadata.address.mip == mip);
            let root = build_subtree(
                &mut descriptors[first..first + count],
                first,
                &mut index.nodes,
            )?;
            index.roots.push(MipRoot { mip, node: root });
            first += count;
        }
        Ok(index)
    }

    pub(super) fn query<F>(
        &self,
        descriptors: &[BrickDescriptor],
        mip: u16,
        focus: [u64; 3],
        radius: u64,
        mut visitor: F,
    ) -> u32
    where
        F: FnMut(BrickDescriptor, Option<u64>) -> ControlFlow<()>,
    {
        let mut visited_nodes = 0;
        let Ok(root_position) = self.roots.binary_search_by_key(&mip, |root| root.mip) else {
            return visited_nodes;
        };
        let Some(root) = self.roots.get(root_position) else {
            return visited_nodes;
        };
        let mut cursor = root.node;
        let Some(end) = self.node(root.node).map(|node| node.escape) else {
            return visited_nodes;
        };
        while cursor < end {
            let Some(node) = self.node(cursor) else {
                break;
            };
            visited_nodes += 1;
            if !intersects(node.min, node.max, focus, radius) {
                cursor = node.escape;
                continue;
            }
            if node.count != 0
                && visit_leaf(node, descriptors, focus, radius, &mut visitor).is_break()
            {
                break;
            }
            cursor += 1;
        }
        visited_nodes
    }

    fn node(&self, index: u32) -> Option<&SpatialNode> {
        usize::try_from(index)
            .ok()
            .and_then(|position| self.nodes.get(position))
    }
}

fn build_subtree(
    descriptors: &mut [BrickDescriptor],
    base: usize,
    nodes: &mut Vec<SpatialNode>,
) -> Result<u32, DatasetError> {
    let bounds = descriptor_bounds(descriptors)?;
    let node_index = u32::try_from(nodes.len()).map_err(|_| DatasetError::CatalogTooLarge)?;
    nodes.push(SpatialNode {
        min: bounds.0,
        max: bounds.1,
        first: 0,
        count: 0,
        escape: 0,
    });
    if descriptors.len() <= LEAF_SIZE {
        let first = u32::try_from(base).map_err(|_| DatasetError::CatalogTooLarge)?;
        let count = u32::try_from(descriptors.len()).map_err(|_| DatasetError::CatalogTooLarge)?;
        let escape = u32::try_from(nodes.len()).map_err(|_| DatasetError::CatalogTooLarge)?;
        nodes[usize::try_from(node_index).map_err(|_| DatasetError::CatalogTooLarge)?] =
            SpatialNode {
                first,
                count,
                escape,
                min: bounds.0,
                max: bounds.1,
            };
        return Ok(node_index);
    }
    let axis = widest_axis(bounds);
    descriptors.sort_unstable_by_key(|value| value.metadata.address.origin[axis]);
    let middle = descriptors.len() / 2;
    let (left, right) = descriptors.split_at_mut(middle);
    build_subtree(left, base, nodes)?;
    build_subtree(right, base + middle, nodes)?;
    let escape = u32::try_from(nodes.len()).map_err(|_| DatasetError::CatalogTooLarge)?;
    let position = usize::try_from(node_index).map_err(|_| DatasetError::CatalogTooLarge)?;
    nodes[position].escape = escape;
    Ok(node_index)
}

fn descriptor_bounds(
    descriptors: &[BrickDescriptor],
) -> Result<([u64; 3], [u64; 3]), DatasetError> {
    let mut min = [u64::MAX; 3];
    let mut max = [0; 3];
    for descriptor in descriptors {
        for axis in 0..3 {
            let origin = descriptor.metadata.address.origin[axis];
            let end = origin
                .checked_add(u64::from(descriptor.metadata.shape.interior()[axis]))
                .ok_or(DatasetError::InvalidLogicalVolume)?;
            min[axis] = min[axis].min(origin);
            max[axis] = max[axis].max(end);
        }
    }
    Ok((min, max))
}

fn widest_axis(bounds: ([u64; 3], [u64; 3])) -> usize {
    let widths = [
        bounds.1[0] - bounds.0[0],
        bounds.1[1] - bounds.0[1],
        bounds.1[2] - bounds.0[2],
    ];
    if widths[1] > widths[0] && widths[1] >= widths[2] {
        1
    } else if widths[2] > widths[0] {
        2
    } else {
        0
    }
}

fn visit_leaf<F>(
    node: &SpatialNode,
    descriptors: &[BrickDescriptor],
    focus: [u64; 3],
    radius: u64,
    visitor: &mut F,
) -> ControlFlow<()>
where
    F: FnMut(BrickDescriptor, Option<u64>) -> ControlFlow<()>,
{
    let Ok(first) = usize::try_from(node.first) else {
        return ControlFlow::Break(());
    };
    let Ok(count) = usize::try_from(node.count) else {
        return ControlFlow::Break(());
    };
    let Some(end) = first.checked_add(count) else {
        return ControlFlow::Break(());
    };
    let Some(leaf) = descriptors.get(first..end) else {
        return ControlFlow::Break(());
    };
    for descriptor in leaf {
        let distance = descriptor_distance(*descriptor, focus);
        let matched_distance = (distance <= radius).then_some(distance);
        if visitor(*descriptor, matched_distance).is_break() {
            return ControlFlow::Break(());
        }
    }
    ControlFlow::Continue(())
}

fn intersects(min: [u64; 3], max: [u64; 3], focus: [u64; 3], radius: u64) -> bool {
    (0..3).all(|axis| axis_distance(min[axis], max[axis], focus[axis]) <= radius)
}

fn descriptor_distance(descriptor: BrickDescriptor, focus: [u64; 3]) -> u64 {
    let origin = descriptor.metadata.address.origin;
    let interior = descriptor.metadata.shape.interior();
    let Some(end_x) = origin[0].checked_add(u64::from(interior[0])) else {
        return u64::MAX;
    };
    let Some(end_y) = origin[1].checked_add(u64::from(interior[1])) else {
        return u64::MAX;
    };
    let Some(end_z) = origin[2].checked_add(u64::from(interior[2])) else {
        return u64::MAX;
    };
    axis_distance(origin[0], end_x, focus[0])
        .max(axis_distance(origin[1], end_y, focus[1]))
        .max(axis_distance(origin[2], end_z, focus[2]))
}

const fn axis_distance(start: u64, end: u64, point: u64) -> u64 {
    if point < start {
        start - point
    } else if point >= end {
        point - end + 1
    } else {
        0
    }
}
