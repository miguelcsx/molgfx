//! CPU-authored cross-section triangles for clipped raster meshes.

use molgfx_core::{ClipCap, ClipPlane, ClipSet};
use molgfx_geometry::RibbonVertex;
use molgfx_math::{Mat4, Vec3};
use std::ops::Range;

#[cfg(test)]
#[path = "mesh_caps_tests.rs"]
mod tests;

const EPSILON: f32 = 1.0e-4;

pub(super) fn append_caps(
    vertices: &mut Vec<RibbonVertex>,
    indices: &mut Vec<u32>,
    vertex_range: Range<usize>,
    index_range: Range<usize>,
    model_to_world: Mat4,
    clipping: ClipSet,
) {
    if clipping.cap() != ClipCap::Solid || clipping.planes().is_empty() {
        return;
    }
    let source_indices = indices[index_range].to_vec();
    let entity_id = vertices
        .get(vertex_range.start)
        .map_or(u32::MAX, |vertex| vertex.entity_id);
    for (plane_index, plane) in clipping.planes().iter().copied().enumerate() {
        let segments = plane_segments(
            &vertices[vertex_range.clone()],
            &source_indices,
            vertex_range.start,
            model_to_world,
            plane,
            clipping.planes(),
            plane_index,
        );
        for mut loop_points in connect_loops(segments) {
            if loop_points.len() < 3 {
                continue;
            }
            orient(&mut loop_points, -plane.normal);
            append_fan(
                vertices,
                indices,
                &loop_points,
                model_to_world,
                -plane.normal,
                entity_id,
            );
        }
    }
}

fn plane_segments(
    vertices: &[RibbonVertex],
    indices: &[u32],
    vertex_base: usize,
    model_to_world: Mat4,
    plane: ClipPlane,
    planes: &[ClipPlane],
    plane_index: usize,
) -> Vec<[Vec3; 2]> {
    let mut segments = Vec::new();
    for triangle in indices.chunks_exact(3) {
        let Some(local) = triangle_points(vertices, triangle, vertex_base) else {
            continue;
        };
        let world = local.map(|point| model_to_world.transform_point3(point));
        let distance = world.map(|point| plane.signed_distance(point));
        let mut crossings = Vec::with_capacity(2);
        for (left, right) in [(0, 1), (1, 2), (2, 0)] {
            if let Some(point) =
                crossing(world[left], world[right], distance[left], distance[right])
                && crossings
                    .iter()
                    .all(|other: &Vec3| other.distance_squared(point) > EPSILON * EPSILON)
            {
                crossings.push(point);
            }
        }
        if crossings.len() == 2
            && let Some(segment) =
                clip_to_other_planes([crossings[0], crossings[1]], planes, plane_index)
        {
            segments.push(segment);
        }
    }
    segments
}

fn triangle_points(
    vertices: &[RibbonVertex],
    triangle: &[u32],
    vertex_base: usize,
) -> Option<[Vec3; 3]> {
    let index = |value: u32| usize::try_from(value).ok()?.checked_sub(vertex_base);
    Some([
        Vec3::from_array(vertices.get(index(triangle[0])?)?.position),
        Vec3::from_array(vertices.get(index(triangle[1])?)?.position),
        Vec3::from_array(vertices.get(index(triangle[2])?)?.position),
    ])
}

fn crossing(a: Vec3, b: Vec3, da: f32, db: f32) -> Option<Vec3> {
    if da.abs() <= EPSILON && db.abs() <= EPSILON {
        return None;
    }
    if da.abs() <= EPSILON {
        return Some(a);
    }
    if db.abs() <= EPSILON {
        return Some(b);
    }
    if da.is_sign_positive() == db.is_sign_positive() {
        return None;
    }
    Some(a.lerp(b, da / (da - db)))
}

fn clip_to_other_planes(
    mut segment: [Vec3; 2],
    planes: &[ClipPlane],
    skipped: usize,
) -> Option<[Vec3; 2]> {
    for (index, plane) in planes.iter().copied().enumerate() {
        if index == skipped {
            continue;
        }
        let distances = segment.map(|point| plane.signed_distance(point));
        if distances[0] < -EPSILON && distances[1] < -EPSILON {
            return None;
        }
        if distances[0] < -EPSILON || distances[1] < -EPSILON {
            let hit = segment[0].lerp(segment[1], distances[0] / (distances[0] - distances[1]));
            if distances[0] < 0.0 {
                segment[0] = hit;
            } else {
                segment[1] = hit;
            }
        }
    }
    (segment[0].distance_squared(segment[1]) > EPSILON * EPSILON).then_some(segment)
}

fn connect_loops(mut segments: Vec<[Vec3; 2]>) -> Vec<Vec<Vec3>> {
    let mut loops = Vec::new();
    while let Some(segment) = segments.pop() {
        let mut points = vec![segment[0], segment[1]];
        loop {
            let Some(end) = points.last().copied() else {
                break;
            };
            let Some((index, reverse)) =
                segments.iter().enumerate().find_map(|(index, candidate)| {
                    if end.distance_squared(candidate[0]) <= EPSILON * EPSILON {
                        Some((index, false))
                    } else if end.distance_squared(candidate[1]) <= EPSILON * EPSILON {
                        Some((index, true))
                    } else {
                        None
                    }
                })
            else {
                break;
            };
            let next = segments.swap_remove(index);
            points.push(if reverse { next[0] } else { next[1] });
            let closes_loop = points
                .last()
                .is_some_and(|last| points[0].distance_squared(*last) <= EPSILON * EPSILON);
            if points.len() > 2 && closes_loop {
                points.pop();
                break;
            }
        }
        deduplicate(&mut points);
        loops.push(points);
    }
    loops
}

fn deduplicate(points: &mut Vec<Vec3>) {
    let mut unique = Vec::with_capacity(points.len());
    for point in points.drain(..) {
        if unique
            .iter()
            .all(|other: &Vec3| other.distance_squared(point) > EPSILON * EPSILON)
        {
            unique.push(point);
        }
    }
    *points = unique;
}

fn orient(points: &mut [Vec3], desired: Vec3) {
    let normal = points
        .iter()
        .enumerate()
        .fold(Vec3::ZERO, |sum, (index, point)| {
            let next = points[(index + 1) % points.len()];
            sum + point.cross(next)
        });
    if normal.dot(desired) < 0.0 {
        points.reverse();
    }
}

fn append_fan(
    vertices: &mut Vec<RibbonVertex>,
    indices: &mut Vec<u32>,
    points: &[Vec3],
    model_to_world: Mat4,
    world_normal: Vec3,
    entity_id: u32,
) {
    let inverse = model_to_world.inverse();
    let divisor = u16::try_from(points.len()).map_or(f32::from(u16::MAX), f32::from);
    let center = points.iter().copied().sum::<Vec3>() / divisor.max(1.0);
    let normal = inverse.transform_vector3(world_normal).normalize_or_zero();
    let color = molgfx_math::Rgba8::opaque(112, 128, 144);
    let base = u32::try_from(vertices.len()).map_or(u32::MAX, |value| value);
    vertices.push(RibbonVertex {
        position: inverse.transform_point3(center).to_array(),
        entity_id,
        normal: normal.to_array(),
        color,
    });
    vertices.extend(points.iter().map(|point| RibbonVertex {
        position: inverse.transform_point3(*point).to_array(),
        entity_id,
        normal: normal.to_array(),
        color,
    }));
    for index in 0..points.len() {
        let current = u32::try_from(index).map_or(u32::MAX, |value| value);
        let next = u32::try_from((index + 1) % points.len()).map_or(u32::MAX, |value| value);
        indices.extend([
            base,
            base.saturating_add(1).saturating_add(current),
            base.saturating_add(1).saturating_add(next),
        ]);
    }
}
