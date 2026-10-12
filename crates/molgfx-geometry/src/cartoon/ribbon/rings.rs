//! O(ring atoms) bipyramids, with O(vertices × ring size) GPU deformation.

use super::{RibbonDeformation, RibbonMesh, RibbonVertex};
use crate::PackingError;
use molgfx_core::{AtomSelection, EntityId, EntityKind};
use molgfx_math::Vec3;

#[cfg(test)]
#[path = "rings_tests.rs"]
mod tests;

impl RibbonMesh {
    /// Generates complete ring bipyramids from `MolFrame` connectivity. Every
    /// recipe gathers the current ring rows, including puckering motion.
    ///
    /// # Errors
    /// Rejects unavailable connectivity, invalid dimensions and GPU limits.
    pub fn generate_ring_bipyramids(
        &mut self,
        structure: &molframe::Structure,
        selection: &AtomSelection,
        height: f32,
        maximum: u8,
        opacity: u8,
    ) -> Result<(), PackingError> {
        if !height.is_finite() || height <= 0.0 || !(3..=12).contains(&maximum) {
            return Err(PackingError::Topology {
                operation: "finite positive ring height and ring size three through twelve",
            });
        }
        let rings = super::super::paper_chain::selected_rings(structure, selection, maximum)?;
        self.clear();
        super::super::paper_chain::append_palette(&mut self.traces.properties, opacity);
        for rows in &rings {
            let mut points = [Vec3::ZERO; 12];
            for (index, row) in rows.iter().enumerate() {
                let Some(point) = structure
                    .engine()
                    .atom(molframe::AtomIndex::new(*row))
                    .and_then(molframe::AtomRef::position)
                else {
                    return Err(PackingError::Topology {
                        operation: "ring coordinates",
                    });
                };
                points[index] = Vec3::from(point);
            }
            self.append_ring(rows, &points[..rows.len()], height, opacity)?;
        }
        Ok(())
    }

    fn append_ring(
        &mut self,
        rows: &[u32],
        points: &[Vec3],
        height: f32,
        opacity: u8,
    ) -> Result<(), PackingError> {
        let count = u8::try_from(rows.len()).map_err(|_| PackingError::Topology {
            operation: "ring size",
        })?;
        let center = points.iter().copied().sum::<Vec3>() / f32::from(count);
        let mut cross = Vec3::ZERO;
        for (edge, point) in points.iter().enumerate() {
            cross += (*point - center).cross(points[(edge + 1) % points.len()] - center);
        }
        let normal = match cross.try_normalize() {
            Some(normal) => normal,
            None => Vec3::Z,
        };
        let amplitude = super::super::paper_chain::pucker_amplitude(points);
        let color = super::super::paper_chain::pucker_color(amplitude, opacity);
        let offset = super::super::draw_limits::vertex_index(self.traces.properties.len())?;
        self.traces.properties.push(height * 0.5);
        self.traces
            .properties
            .extend(rows.iter().map(|row| f32::from_bits(*row)));
        let entity_id = EntityId::pack(EntityKind::Atom, u64::from(rows[0]))?.0;
        for edge in 0..rows.len() {
            let next = (edge + 1) % rows.len();
            for (apex, a, b, corners) in [
                (
                    center + normal * height * 0.5,
                    points[edge],
                    points[next],
                    [0u32, 1, 2],
                ),
                (
                    center - normal * height * 0.5,
                    points[next],
                    points[edge],
                    [7u32, 6, 5],
                ),
            ] {
                let face_normal = match (a - apex).cross(b - apex).try_normalize() {
                    Some(normal) => normal,
                    None => Vec3::Z,
                };
                let base = super::super::draw_limits::vertex_index(self.vertices.len())?;
                for (position, corner) in [apex, a, b].into_iter().zip(corners) {
                    self.vertices.push(RibbonVertex {
                        position: position.to_array(),
                        normal: face_normal.to_array(),
                        entity_id,
                        color,
                    });
                    self.deformations.push(RibbonDeformation {
                        controls: [rows[edge], rows[next], rows[0], rows[0]],
                        parameter: [
                            f32::from_bits(offset),
                            f32::from(count),
                            f32::from_bits(corner),
                            f32::from_bits(8),
                        ],
                    });
                }
                self.indices.extend([base, base + 1, base + 2]);
            }
        }
        Ok(())
    }
}
