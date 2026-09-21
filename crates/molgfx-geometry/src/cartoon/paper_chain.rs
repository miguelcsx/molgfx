//! Ring-pucker plates for the `PaperChain` representation.
//!
//! Each known biological ring is drawn as a thin two-sided sheet through its
//! deposited atoms, so the sheet bends exactly as the ring puckers. Its colour
//! reports total out-of-plane puckering on a continuous ramp, so a planar
//! aromatic base and a chair-form sugar are visually distinct without two
//! near-identical rings landing in different colour bands. Geometry generation
//! is `O(ring atoms)` and runs only when topology changes.

use crate::RibbonVertex;
use molgfx_core::{AtomSelection, EntityId, EntityKind};
use molgfx_math::{Rgba8, Vec3};

#[cfg(test)]
#[path = "paper_chain_tests.rs"]
mod tests;

const PYRANOSE: [&str; 6] = ["O5", "C1", "C2", "C3", "C4", "C5"];
const FURANOSE: [&str; 5] = ["O4", "C1", "C2", "C3", "C4"];
const RIBOSE: [&str; 5] = ["O4'", "C1'", "C2'", "C3'", "C4'"];
const PYRIMIDINE: [&str; 6] = ["N1", "C2", "N3", "C4", "C5", "C6"];
const PURINE_SIX: [&str; 6] = ["N1", "C2", "N3", "C4", "C5", "C6"];
const PURINE_FIVE: [&str; 5] = ["C4", "C5", "N7", "C8", "N9"];

/// Appends pucker-coloured bipyramids for selected carbohydrate and nucleotide
/// rings. A partial ring is omitted rather than visually closed across absent
/// source atoms.
///
/// # Errors
///
/// Returns [`crate::PackingError`] when a ring atom row cannot be encoded.
pub fn append_paper_chain(
    structure: &molframe::Structure,
    selection: &AtomSelection,
    height: f32,
    opacity: u8,
    vertices: &mut Vec<RibbonVertex>,
    indices: &mut Vec<u32>,
) -> Result<(), crate::PackingError> {
    for residue in structure.data().residues() {
        if residue.atom("O5").is_some() {
            append_named_ring(
                residue, &PYRANOSE, selection, height, opacity, vertices, indices,
            )?;
        } else if residue.atom("O4").is_some() && residue.atom("C1").is_some() {
            append_named_ring(
                residue, &FURANOSE, selection, height, opacity, vertices, indices,
            )?;
        }
        append_named_ring(
            residue, &RIBOSE, selection, height, opacity, vertices, indices,
        )?;
        if residue.atom("N9").is_some() {
            append_named_ring(
                residue,
                &PURINE_SIX,
                selection,
                height,
                opacity,
                vertices,
                indices,
            )?;
            append_named_ring(
                residue,
                &PURINE_FIVE,
                selection,
                height,
                opacity,
                vertices,
                indices,
            )?;
        } else if residue.atom("N1").is_some() {
            append_named_ring(
                residue,
                &PYRIMIDINE,
                selection,
                height,
                opacity,
                vertices,
                indices,
            )?;
        }
    }
    Ok(())
}

fn append_named_ring(
    residue: molframe::ResidueRef<'_>,
    names: &[&str],
    selection: &AtomSelection,
    height: f32,
    opacity: u8,
    vertices: &mut Vec<RibbonVertex>,
    indices: &mut Vec<u32>,
) -> Result<(), crate::PackingError> {
    let mut ring = Vec::with_capacity(names.len());
    let mut entity = None;
    for name in names {
        let Some(atom) = residue.atom(name) else {
            return Ok(());
        };
        if !selection.contains(atom.index().get()) {
            return Ok(());
        }
        let Some(position) = atom.position() else {
            return Ok(());
        };
        ring.push(Vec3::from(position));
        let entity_id = EntityId::pack(EntityKind::Atom, u64::from(atom.index().get()))?;
        entity.get_or_insert(entity_id.0);
    }
    let Some(entity) = entity else {
        return Ok(());
    };
    emit_plate(&ring, height, opacity, entity, vertices, indices);
    Ok(())
}

/// Emits one ring as a plate: two capped faces separated by a lit rim.
///
/// The cap vertices are the deposited ring atoms offset along the plane normal,
/// not a fitted polygon, so the pucker stays in the picture as the bend in the
/// sheet. A bipyramid built to a centroid apex instead reads as a solid gem and
/// buries the very displacement the colour is reporting.
///
/// The rim exists so the plate has a silhouette. A zero-thickness polygon has a
/// razor edge that no amount of sampling resolves, and at 4K that edge is the
/// first thing that looks wrong.
fn emit_plate(
    ring: &[Vec3],
    height: f32,
    opacity: u8,
    entity: u32,
    vertices: &mut Vec<RibbonVertex>,
    indices: &mut Vec<u32>,
) {
    if ring.len() < 3 {
        return;
    }
    let count = u16::try_from(ring.len()).map_or(f32::from(u16::MAX), f32::from);
    let centroid = ring.iter().copied().sum::<Vec3>() / count.max(1.0);
    let Some(normal) = ring_normal(ring) else {
        return;
    };
    let offset = normal * height.abs().max(0.02);
    let color = pucker_color(pucker_amplitude(ring, centroid, normal), opacity);
    for edge in 0..ring.len() {
        let next = (edge + 1) % ring.len();
        let (top, top_next) = (ring[edge] + offset, ring[next] + offset);
        let (bottom, bottom_next) = (ring[edge] - offset, ring[next] - offset);
        // Both caps take the ring's own plane normal rather than the normal of
        // each fan triangle. The positions still carry the pucker, so the sheet
        // bends in silhouette; giving every wedge its own normal instead lights
        // the face as a six-bladed pinwheel, which reads as crumpled foil and
        // hides the very displacement the colour is reporting.
        push_face(
            [centroid + offset, top, top_next],
            normal,
            entity,
            color,
            vertices,
            indices,
        );
        push_face(
            [centroid - offset, bottom_next, bottom],
            -normal,
            entity,
            color,
            vertices,
            indices,
        );
        // The rim is a real edge and keeps its geometric normal, which is what
        // gives the plate a lit border instead of a razor silhouette.
        push_triangle(top, bottom, bottom_next, entity, color, vertices, indices);
        push_triangle(top, bottom_next, top_next, entity, color, vertices, indices);
    }
}

fn ring_normal(ring: &[Vec3]) -> Option<Vec3> {
    let mut normal = Vec3::ZERO;
    for (index, current) in ring.iter().enumerate() {
        normal += current.cross(ring[(index + 1) % ring.len()]);
    }
    normal.try_normalize()
}

fn pucker_amplitude(ring: &[Vec3], centroid: Vec3, normal: Vec3) -> f32 {
    ring.iter()
        .map(|point| ((*point - centroid).dot(normal)).powi(2))
        .sum::<f32>()
        .sqrt()
}

/// Colour ramp stops, each paired with the pucker amplitude it sits at in
/// hundredths of an Angstrom.
///
/// Keeping the stops as data and interpolating between them replaces the step
/// function these thresholds used to drive. Two rings whose pucker differs by a
/// hundredth of an Angstrom now differ by a hundredth of a hue instead of
/// jumping a whole band, which is what a reader expects from a quantity that is
/// itself continuous.
const PUCKER_STOPS: [(u16, [u8; 3]); 6] = [
    (0, [220, 62, 55]),
    (19, [240, 178, 48]),
    (48, [74, 176, 96]),
    (78, [54, 190, 194]),
    (102, [65, 105, 225]),
    (130, [184, 76, 190]),
];

fn pucker_color(amplitude: f32, opacity: u8) -> Rgba8 {
    let scaled = centi_angstrom(amplitude);
    let mut color = PUCKER_STOPS[PUCKER_STOPS.len() - 1].1;
    for window in PUCKER_STOPS.windows(2) {
        let [(low, start), (high, end)] = [window[0], window[1]];
        if scaled >= high {
            continue;
        }
        color = [
            mix_channel(start[0], end[0], scaled - low, high - low),
            mix_channel(start[1], end[1], scaled - low, high - low),
            mix_channel(start[2], end[2], scaled - low, high - low),
        ];
        break;
    }
    Rgba8::new(color[0], color[1], color[2], opacity)
}

/// Blends one channel between two stops without leaving integer arithmetic.
fn mix_channel(start: u8, end: u8, amount: u16, span: u16) -> u8 {
    let span = u32::from(span.max(1));
    let amount = u32::from(amount).min(span);
    let low = u32::from(start.min(end));
    let step = (u32::from(start.max(end)) - low) * amount / span;
    let value = if start <= end {
        low + step
    } else {
        u32::from(start) - step
    };
    u8::try_from(value)
        .into_iter()
        .fold(u8::MAX, |_, byte| byte)
}

/// Amplitude in hundredths of an Angstrom, saturating rather than wrapping.
///
/// Truncation rather than rounding: this is a magnitude, and rounding up would
/// report an amplitude the source never reached.
fn centi_angstrom(amplitude: f32) -> u16 {
    molgfx_math::truncate_u16(amplitude * 100.0)
}

fn push_triangle(
    a: Vec3,
    b: Vec3,
    c: Vec3,
    entity_id: u32,
    color: Rgba8,
    vertices: &mut Vec<RibbonVertex>,
    indices: &mut Vec<u32>,
) {
    let Some(normal) = (b - a).cross(c - a).try_normalize() else {
        return;
    };
    push_face([a, b, c], normal, entity_id, color, vertices, indices);
}

/// Appends one triangle carrying a normal the caller chose.
fn push_face(
    corners: [Vec3; 3],
    normal: Vec3,
    entity_id: u32,
    color: Rgba8,
    vertices: &mut Vec<RibbonVertex>,
    indices: &mut Vec<u32>,
) {
    let Ok(base) = u32::try_from(vertices.len()) else {
        return;
    };
    for position in corners {
        vertices.push(RibbonVertex {
            position: position.to_array(),
            entity_id,
            normal: normal.to_array(),
            color,
        });
    }
    indices.extend([base, base.saturating_add(1), base.saturating_add(2)]);
}
