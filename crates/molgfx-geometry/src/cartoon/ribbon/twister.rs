//! Compact ring addresses shared by every cross-section deformation recipe.
use super::RibbonMesh;
use crate::PackingError;
use std::collections::BTreeMap;

impl RibbonMesh {
    pub(super) fn bind_ring_guides(&mut self, rings: &[[u32; 7]]) -> Result<(), PackingError> {
        self.traces.properties.clear();
        let mut offsets = BTreeMap::new();
        for ring in rings {
            let offset = u32::try_from(self.traces.properties.len()).map_err(|_| {
                PackingError::IndexOverflow {
                    resource: "ring guide addresses",
                    index: self.traces.properties.len() as u64,
                }
            })?;
            offsets.insert(ring[1], offset);
            self.traces
                .properties
                .extend(ring.iter().copied().map(f32::from_bits));
        }
        for deformation in &mut self.deformations {
            for control in &mut deformation.controls {
                *control = *offsets.get(control).ok_or(PackingError::Topology {
                    operation: "complete ring guide for each glycosidic control",
                })?;
            }
            deformation.parameter[3] = f32::from_bits(11);
        }
        Ok(())
    }
}
