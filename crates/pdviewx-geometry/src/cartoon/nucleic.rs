//! Nucleotide base slabs: the rungs of a nucleic-acid ladder.
//!
//! A cartoon spline already follows the nucleic backbone through `C4'`, but a
//! bare tube tells the reader nothing about which way a base points or how two
//! strands pair. Each base is emitted as a flat slab covering the real ring
//! footprint, plus a short connector from the sugar, so the ladder reads the
//! way it does in the literature.
//!
//! Geometry is generated into the same vertex and index buffers the cartoon
//! already uses, so slabs draw in the existing pass with the existing shader,
//! clipping, transparency and picking. Vertices carry the residue's guide-atom
//! id, so every colour scheme recolours them with no extra code.

use crate::RibbonVertex;
use pdviewx_core::{AtomSelection, EntityId, EntityKind};
use pdviewx_math::{Rgba8, Vec3};

#[cfg(test)]
#[path = "nucleic_tests.rs"]
mod tests;

/// Ring atoms of the two base families. Amino acids never carry these names —
/// their side chains use Greek suffixes — so their presence, together with the
/// sugar, identifies a nucleotide without consulting a residue-name list.
const RING_ATOMS: [&str; 9] = ["N1", "C2", "N3", "C4", "C5", "C6", "N7", "C8", "N9"];
const PYRIMIDINE_RING: [&str; 6] = ["N1", "C2", "N3", "C4", "C5", "C6"];
const PURINE_OUTLINE: [&str; 9] = ["N9", "C8", "N7", "C5", "C6", "N1", "C2", "N3", "C4"];
/// Sugar atom the base hangs from.
const SUGAR_ATTACHMENT: &str = "C1'";
/// Ring nitrogen bonded to the sugar: `N9` in purines, `N1` in pyrimidines.
const GLYCOSIDIC_ATOMS: [&str; 2] = ["N9", "N1"];
/// Minimum ring atoms needed to define a plane.
const MIN_RING_ATOMS: usize = 3;
/// Half-width of the sugar connector, Ångström.
const CONNECTOR_HALF_WIDTH: f32 = 0.16;
/// Padding around the ring footprint so the slab encloses the atoms, Ångström.
const FOOTPRINT_MARGIN: f32 = 0.34;

#[derive(Clone, Copy)]
struct RoundEdgeStyle {
    normal: Vec3,
    radius: f32,
    entity_id: u32,
    color: Rgba8,
}

/// Appends one slab per selected nucleotide, plus its sugar connector.
///
/// `thickness` is the full slab depth in Ångström. Residues without a ring or
/// without positioned atoms are skipped rather than approximated.
pub fn append_base_slabs(
    structure: &pdbiox::Structure,
    selection: &AtomSelection,
    thickness: f32,
    vertices: &mut Vec<RibbonVertex>,
    indices: &mut Vec<u32>,
) {
    let half_thickness = (thickness.max(0.02)) * 0.5;
    for chain in structure.data().chains() {
        for residue in chain.residues() {
            let Some(sugar) = residue.atom(SUGAR_ATTACHMENT) else {
                continue;
            };
            if !selection.contains(sugar.index().get()) {
                continue;
            }
            let Some(sugar_position) = sugar.position().map(Vec3::from) else {
                continue;
            };
            let mut ring = Vec::with_capacity(RING_ATOMS.len());
            for name in RING_ATOMS {
                if let Some(position) = residue.atom(name).and_then(pdbiox::AtomRef::position) {
                    ring.push(Vec3::from(position));
                }
            }
            if ring.len() < MIN_RING_ATOMS {
                continue;
            }
            let entity_id = EntityId::pack(EntityKind::Atom, sugar.index().get()).0;
            let glycosidic = GLYCOSIDIC_ATOMS
                .iter()
                .find_map(|name| residue.atom(name).and_then(pdbiox::AtomRef::position))
                .map(Vec3::from);
            emit_base(
                &ring,
                sugar_position,
                glycosidic,
                half_thickness,
                entity_id,
                vertices,
                indices,
            );
        }
    }
}

/// Appends true ring-footprint prisms with rounded boundary tubes.
///
/// Atom order follows the stable purine/pyrimidine topology rather than a
/// convex bounding rectangle. The caller's property colouring is applied by
/// the ribbon recolouring step after this geometry is appended.
pub fn append_base_polygons(
    structure: &pdbiox::Structure,
    selection: &AtomSelection,
    thickness: f32,
    outline_radius: f32,
    vertices: &mut Vec<RibbonVertex>,
    indices: &mut Vec<u32>,
) {
    let half_thickness = thickness.max(0.02) * 0.5;
    for chain in structure.data().chains() {
        for residue in chain.residues() {
            let Some(sugar) = residue.atom(SUGAR_ATTACHMENT) else {
                continue;
            };
            if !selection.contains(sugar.index().get()) {
                continue;
            }
            let Some(sugar_position) = sugar.position().map(Vec3::from) else {
                continue;
            };
            let names: &[&str] = if residue.atom("N9").is_some() {
                &PURINE_OUTLINE
            } else {
                &PYRIMIDINE_RING
            };
            let ring: Vec<_> = names
                .iter()
                .filter_map(|name| {
                    residue
                        .atom(name)
                        .and_then(pdbiox::AtomRef::position)
                        .map(Vec3::from)
                })
                .collect();
            if ring.len() < MIN_RING_ATOMS {
                continue;
            }
            let entity_id = EntityId::pack(EntityKind::Atom, sugar.index().get()).0;
            emit_polygon(
                &ring,
                sugar_position,
                half_thickness,
                outline_radius.max(0.01),
                entity_id,
                vertices,
                indices,
            );
        }
    }
}

fn emit_polygon(
    ring: &[Vec3],
    sugar: Vec3,
    half_thickness: f32,
    outline_radius: f32,
    entity_id: u32,
    vertices: &mut Vec<RibbonVertex>,
    indices: &mut Vec<u32>,
) {
    let count = u16::try_from(ring.len()).map_or(f32::from(u16::MAX), f32::from);
    let center = ring.iter().copied().sum::<Vec3>() / count.max(1.0);
    let Some(normal) = plane_normal(ring, center) else {
        return;
    };
    let color = Rgba8::opaque(200, 200, 200);
    let Ok(base) = u32::try_from(vertices.len()) else {
        return;
    };
    vertices.push(vertex(
        center + normal * half_thickness,
        normal,
        entity_id,
        color,
    ));
    vertices.push(vertex(
        center - normal * half_thickness,
        -normal,
        entity_id,
        color,
    ));
    for point in ring {
        vertices.push(vertex(
            *point + normal * half_thickness,
            normal,
            entity_id,
            color,
        ));
        vertices.push(vertex(
            *point - normal * half_thickness,
            -normal,
            entity_id,
            color,
        ));
    }
    let ring_count = ring.len();
    for index in 0..ring_count {
        let next = (index + 1) % ring_count;
        let top = base
            .saturating_add(2)
            .saturating_add(u32_index(index).saturating_mul(2));
        let top_next = base
            .saturating_add(2)
            .saturating_add(u32_index(next).saturating_mul(2));
        let bottom = top.saturating_add(1);
        let bottom_next = top_next.saturating_add(1);
        indices.extend([base, top, top_next]);
        indices.extend([base.saturating_add(1), bottom_next, bottom]);
        indices.extend([top, bottom, bottom_next, top, bottom_next, top_next]);
        push_round_edge(
            ring[index],
            ring[next],
            RoundEdgeStyle {
                normal,
                radius: outline_radius,
                entity_id,
                color,
            },
            vertices,
            indices,
        );
    }
    let attachment = ring.iter().copied().min_by(|left, right| {
        left.distance_squared(sugar)
            .total_cmp(&right.distance_squared(sugar))
    });
    if let Some(attachment) = attachment {
        let span = attachment - sugar;
        if let Some(direction) = span.try_normalize() {
            push_box(
                sugar + span * 0.5,
                direction * span.length() * 0.5,
                normal.cross(direction) * CONNECTOR_HALF_WIDTH,
                normal * half_thickness.min(CONNECTOR_HALF_WIDTH),
                entity_id,
                vertices,
                indices,
            );
        }
    }
}

fn push_round_edge(
    start: Vec3,
    end: Vec3,
    style: RoundEdgeStyle,
    vertices: &mut Vec<RibbonVertex>,
    indices: &mut Vec<u32>,
) {
    const SIDES: usize = 8;
    let axis = (end - start).normalize_or_zero();
    let side = axis.cross(style.normal).normalize_or_zero();
    let Ok(base) = u32::try_from(vertices.len()) else {
        return;
    };
    for endpoint in [start, end] {
        for side_index in 0..SIDES {
            let side_u16 = u16::try_from(side_index).map_or(0, |value| value);
            let angle = std::f32::consts::TAU * f32::from(side_u16) / 8.0;
            let radial = style.normal * angle.cos() + side * angle.sin();
            vertices.push(vertex(
                endpoint + radial * style.radius,
                radial,
                style.entity_id,
                style.color,
            ));
        }
    }
    for side_index in 0..SIDES {
        let next = (side_index + 1) % SIDES;
        let a = base.saturating_add(u32_index(side_index));
        let b = base.saturating_add(u32_index(next));
        let c = base.saturating_add(u32_index(SIDES + next));
        let d = base.saturating_add(u32_index(SIDES + side_index));
        indices.extend([a, d, c, a, c, b]);
    }
}

fn vertex(position: Vec3, normal: Vec3, entity_id: u32, color: Rgba8) -> RibbonVertex {
    RibbonVertex {
        position: position.to_array(),
        entity_id,
        normal: normal.to_array(),
        color,
    }
}

fn u32_index(value: usize) -> u32 {
    u32::try_from(value).map_or(u32::MAX, |value| value)
}

fn emit_base(
    ring: &[Vec3],
    sugar: Vec3,
    glycosidic: Option<Vec3>,
    half_thickness: f32,
    entity_id: u32,
    vertices: &mut Vec<RibbonVertex>,
    indices: &mut Vec<u32>,
) {
    let (sum, count) = ring
        .iter()
        .fold((Vec3::ZERO, 0.0f32), |(sum, count), point| {
            (sum + *point, count + 1.0)
        });
    let centroid = sum / count.max(1.0);
    let Some(normal) = plane_normal(ring, centroid) else {
        return;
    };
    // Orient the slab's long axis away from the sugar so the rectangle follows
    // the base rather than an arbitrary world axis.
    // The glycosidic nitrogen anchors the base; the sugar carbon stands in
    // when that atom is absent from the model.
    let reference = match glycosidic {
        Some(atom) => atom,
        None => sugar,
    };
    let along = (centroid - reference).reject_from_normalized(normal);
    let Some(axis_u) = along.try_normalize() else {
        return;
    };
    let axis_v = normal.cross(axis_u);
    let mut half_u = 0.0f32;
    let mut half_v = 0.0f32;
    for point in ring {
        let offset = *point - centroid;
        half_u = half_u.max(offset.dot(axis_u).abs());
        half_v = half_v.max(offset.dot(axis_v).abs());
    }
    push_box(
        centroid,
        axis_u * (half_u + FOOTPRINT_MARGIN),
        axis_v * (half_v + FOOTPRINT_MARGIN),
        normal * half_thickness,
        entity_id,
        vertices,
        indices,
    );

    // The connector runs from the sugar to the nearest slab edge.
    let attachment = reference;
    let span = centroid - attachment;
    let Some(direction) = span.try_normalize() else {
        return;
    };
    let side = normal.cross(direction);
    push_box(
        attachment + span * 0.5,
        direction * (span.length() * 0.5),
        side * CONNECTOR_HALF_WIDTH,
        normal * half_thickness.min(CONNECTOR_HALF_WIDTH),
        entity_id,
        vertices,
        indices,
    );
}

/// Plane normal from the two most independent ring directions. Any three
/// non-collinear atoms of a planar ring define it, so this avoids an
/// order-dependent polygon walk over fused rings.
fn plane_normal(ring: &[Vec3], centroid: Vec3) -> Option<Vec3> {
    let primary = ring
        .iter()
        .map(|point| *point - centroid)
        .max_by(|left, right| left.length_squared().total_cmp(&right.length_squared()))?;
    let secondary = ring
        .iter()
        .map(|point| *point - centroid)
        .max_by(|left, right| {
            primary
                .cross(*left)
                .length_squared()
                .total_cmp(&primary.cross(*right).length_squared())
        })?;
    primary.cross(secondary).try_normalize()
}

/// Emits a flat-shaded box from three half-extent vectors: 24 vertices so each
/// face carries its own normal, and 36 indices.
fn push_box(
    center: Vec3,
    half_u: Vec3,
    half_v: Vec3,
    half_n: Vec3,
    entity_id: u32,
    vertices: &mut Vec<RibbonVertex>,
    indices: &mut Vec<u32>,
) {
    const FACES: [(usize, bool); 6] = [
        (2, true),
        (2, false),
        (0, true),
        (0, false),
        (1, true),
        (1, false),
    ];
    let axes = [half_u, half_v, half_n];
    for (axis, positive) in FACES {
        let sign = if positive { 1.0 } else { -1.0 };
        let face_normal = axes[axis] * sign;
        let Some(unit_normal) = face_normal.try_normalize() else {
            continue;
        };
        let first = axes[(axis + 1) % 3];
        let second = axes[(axis + 2) % 3] * sign;
        let Ok(base) = u32::try_from(vertices.len()) else {
            return;
        };
        for (u, v) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            let position = center + face_normal + first * u + second * v;
            vertices.push(RibbonVertex {
                position: position.to_array(),
                entity_id,
                normal: unit_normal.to_array(),
                color: Rgba8::opaque(200, 200, 200),
            });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}
