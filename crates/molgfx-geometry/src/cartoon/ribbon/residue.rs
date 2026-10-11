//! Local affine base-ring deformation over borrowed source atom rows.
use super::{RibbonDeformation, RibbonMesh};
use molgfx_core::{EntityId, EntityKind};
use molgfx_math::{Mat3, Vec3};

const ANCHORS: [&str; 10] = ["C1'", "N1", "C2", "N3", "C4", "C5", "C6", "N7", "C8", "N9"];

impl RibbonMesh {
    /// Anchors each nucleotide vertex to nearby source atoms. Ring deformation
    /// follows internal motion as well as translation, without copying coordinates.
    pub fn anchor_nucleotide_vertices(&mut self, structure: &molframe::Structure, start: usize) {
        self.pad_static_deformations();
        for (vertex, recipe) in self.vertices[start..]
            .iter()
            .zip(&mut self.deformations[start..])
        {
            let Some((EntityKind::Atom, row)) = EntityId(vertex.entity_id).unpack() else {
                continue;
            };
            let Some(residue) = structure
                .engine()
                .atom(molframe::AtomIndex::new(row))
                .and_then(molframe::AtomRef::residue)
            else {
                continue;
            };
            let position = Vec3::from(vertex.position);
            let mut anchors = [(u32::MAX, Vec3::ZERO, f32::INFINITY); 10];
            let mut count = 0;
            for name in ANCHORS {
                let Some(atom) = residue.atom(name) else {
                    continue;
                };
                let Some(point) = atom.position().map(Vec3::from) else {
                    continue;
                };
                anchors[count] = (atom.index().get(), point, point.distance_squared(position));
                count += 1;
            }
            anchors[..count]
                .sort_by(|left, right| left.2.total_cmp(&right.2).then(left.0.cmp(&right.0)));
            if count < 3 {
                continue;
            }
            let first = anchors[0];
            let second = anchors[1];
            let edge = second.1 - first.1;
            let Some(third) = anchors[2..count]
                .iter()
                .find(|third| edge.cross(third.1 - first.1).length_squared() > 1e-12)
            else {
                continue;
            };
            let normal = edge.cross(third.1 - first.1).normalize();
            let reference = Mat3::from_cols(edge, third.1 - first.1, normal);
            let local = reference.inverse() * (position - first.1);
            *recipe = RibbonDeformation {
                controls: [first.0, second.0, third.0, third.0],
                parameter: [local.x, local.y, local.z, f32::from_bits(5)],
            };
        }
    }
}
