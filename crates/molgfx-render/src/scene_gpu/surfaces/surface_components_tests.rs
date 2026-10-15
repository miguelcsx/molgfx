use super::*;
use crate::passes::SurfaceComponentPass;
use crate::testing::{MockBindGroupLayout, MockDevice};
use molgfx_gpu::{Device, TextureDesc, TextureDimension, TextureUsage, TextureViewDesc};
use std::collections::VecDeque;

#[test]
fn metric_lowering_is_exact_and_large_voxel_limits_discard_all() {
    let Ok(area) = SurfaceComponentPolicy::minimum_area(3.5) else {
        panic!("area policy validates")
    };
    let Ok(volume) = SurfaceComponentPolicy::minimum_volume(7.25) else {
        panic!("volume policy validates")
    };
    let Ok(voxels) = SurfaceComponentPolicy::minimum_voxels(u64::from(u32::MAX) + 1) else {
        panic!("voxel policy validates")
    };
    let Ok(area_config) = component_config([8, 7, 6], 0.5, 1.0, false, area) else {
        panic!("area lowers")
    };
    let Ok(volume_config) = component_config([8, 7, 6], 0.5, 1.0, true, volume) else {
        panic!("volume lowers")
    };
    let Ok(voxel_config) = component_config([8, 7, 6], 0.5, 1.0, false, voxels) else {
        panic!("voxel limit lowers")
    };
    assert_eq!(area_config.metric[0], 1);
    assert!((area_config.values[2] - 3.5).abs() < f32::EPSILON);
    assert_eq!(volume_config.metric[0], 2);
    assert_eq!(volume_config.metric[1], 1);
    assert_eq!(voxel_config.metric[0], 4);
}

#[test]
fn scratch_is_reused_and_pressure_is_typed() {
    let device = MockDevice::default();
    let queue = device.queue();
    SurfaceComponentPass::<MockDevice>::layout(&device);
    let texture = device
        .create_texture(&TextureDesc {
            label: "component test source",
            width: 4,
            height: 4,
            depth: 4,
            dimension: TextureDimension::D3,
            format: TextureFormat::R32Float,
            usage: TextureUsage::TEXTURE_BINDING,
        })
        .unwrap_or_else(|error| panic!("source allocates: {error}"));
    let view = device.create_texture_view(&texture, &TextureViewDesc {});
    let policy = SurfaceComponentPolicy::minimum_voxels(2)
        .unwrap_or_else(|error| panic!("policy validates: {error}"));
    let mut resources = SurfaceComponents::new();
    resources
        .sync(&SurfaceComponentSync {
            device: &device,
            queue: &queue,
            layout: &MockBindGroupLayout::default(),
            source: &view,
            dimensions: [4; 3],
            cell: 1.0,
            isolevel: 0.0,
            gaussian: false,
            policy,
        })
        .unwrap_or_else(|error| panic!("scratch allocates: {error}"));
    let allocations = device
        .log
        .buffers
        .lock()
        .unwrap_or_else(|error| panic!("buffer log locks: {error}"))
        .len();
    resources
        .sync(&SurfaceComponentSync {
            device: &device,
            queue: &queue,
            layout: &MockBindGroupLayout::default(),
            source: &view,
            dimensions: [4; 3],
            cell: 1.0,
            isolevel: 0.0,
            gaussian: false,
            policy,
        })
        .unwrap_or_else(|error| panic!("scratch reuses: {error}"));
    assert_eq!(
        device
            .log
            .buffers
            .lock()
            .unwrap_or_else(|error| panic!("buffer log locks: {error}"))
            .len(),
        allocations
    );

    let constrained = MockDevice::with_storage_limit(64);
    let error = storage_buffer::<MockDevice>(&constrained, "pressure", 32, 8);
    assert!(matches!(error, Err(RenderError::SurfaceComponents { .. })));
}

#[test]
fn union_model_matches_flood_fill_across_a_brick_halo() {
    let dimensions = [7, 5, 3];
    let mut occupied = vec![false; dimensions.iter().product()];
    for point in [
        [1, 2, 1],
        [2, 2, 1],
        [3, 2, 1],
        [4, 2, 1],
        [5, 2, 1],
        [1, 4, 1],
    ] {
        occupied[index(point, dimensions)] = true;
    }
    let union = union_components(&occupied, dimensions);
    let flood = flood_components(&occupied, dimensions);
    assert_eq!(component_sizes(&union), component_sizes(&flood));
    assert_eq!(component_sizes(&union), vec![1, 5]);
    assert_eq!(
        union[index([2, 2, 1], dimensions)],
        union[index([4, 2, 1], dimensions)]
    );
}

fn union_components(occupied: &[bool], dimensions: [usize; 3]) -> Vec<usize> {
    let mut parents: Vec<_> = (0..occupied.len()).collect();
    for z in 0..dimensions[2] {
        for y in 0..dimensions[1] {
            for x in 0..dimensions[0] {
                let point = [x, y, z];
                let current = index(point, dimensions);
                if !occupied[current] {
                    continue;
                }
                for neighbour in negative_neighbours(point) {
                    if inside(neighbour, dimensions) {
                        let other = index(neighbour.map(isize::cast_unsigned), dimensions);
                        if occupied[other] {
                            unite(&mut parents, current, other);
                        }
                    }
                }
            }
        }
    }
    (0..occupied.len())
        .map(|value| {
            if occupied[value] {
                root(&parents, value)
            } else {
                usize::MAX
            }
        })
        .collect()
}

fn flood_components(occupied: &[bool], dimensions: [usize; 3]) -> Vec<usize> {
    let mut labels = vec![usize::MAX; occupied.len()];
    for seed in 0..occupied.len() {
        if !occupied[seed] || labels[seed] != usize::MAX {
            continue;
        }
        labels[seed] = seed;
        let mut queue = VecDeque::from([seed]);
        while let Some(value) = queue.pop_front() {
            let point = coordinates(value, dimensions);
            for neighbour in all_neighbours(point) {
                if inside(neighbour, dimensions) {
                    let next = index(neighbour.map(isize::cast_unsigned), dimensions);
                    if occupied[next] && labels[next] == usize::MAX {
                        labels[next] = seed;
                        queue.push_back(next);
                    }
                }
            }
        }
    }
    labels
}

fn root(parents: &[usize], mut value: usize) -> usize {
    while parents[value] != value {
        value = parents[value];
    }
    value
}

fn unite(parents: &mut [usize], left: usize, right: usize) {
    let low = root(parents, left).min(root(parents, right));
    let high = root(parents, left).max(root(parents, right));
    parents[high] = low;
}

fn component_sizes(labels: &[usize]) -> Vec<usize> {
    let mut counts = std::collections::BTreeMap::new();
    for label in labels.iter().copied().filter(|label| *label != usize::MAX) {
        *counts.entry(label).or_insert(0) += 1;
    }
    let mut values: Vec<_> = counts.into_values().collect();
    values.sort_unstable();
    values
}

fn index(point: [usize; 3], dimensions: [usize; 3]) -> usize {
    point[0] + dimensions[0] * (point[1] + dimensions[1] * point[2])
}

fn coordinates(index: usize, dimensions: [usize; 3]) -> [usize; 3] {
    [
        index % dimensions[0],
        (index / dimensions[0]) % dimensions[1],
        index / (dimensions[0] * dimensions[1]),
    ]
}

fn negative_neighbours(point: [usize; 3]) -> [[isize; 3]; 3] {
    let [x, y, z] = point.map(usize::cast_signed);
    [[x - 1, y, z], [x, y - 1, z], [x, y, z - 1]]
}

fn all_neighbours(point: [usize; 3]) -> [[isize; 3]; 6] {
    let [x, y, z] = point.map(usize::cast_signed);
    [
        [x - 1, y, z],
        [x + 1, y, z],
        [x, y - 1, z],
        [x, y + 1, z],
        [x, y, z - 1],
        [x, y, z + 1],
    ]
}

fn inside(point: [isize; 3], dimensions: [usize; 3]) -> bool {
    point
        .into_iter()
        .zip(dimensions)
        .all(|(axis, limit)| axis >= 0 && usize::try_from(axis).is_ok_and(|axis| axis < limit))
}
