//! Compact fit jobs and borrowed guide indices, prepared in O(vertices + guides).

use super::RibbonMesh;
use molgfx_core::{EntityId, EntityKind};
use num_traits::ToPrimitive;
use std::collections::HashMap;
#[cfg(test)]
#[path = "helix_tests.rs"]
mod tests;

#[derive(Default)]
pub(super) struct Run {
    pub(super) rows: Vec<u32>,
    pub(super) seen: HashMap<u32, usize>,
    pub(super) destination: u32,
}

impl RibbonMesh {
    /// Appends persistent current, previous and reference helix-fit output lanes.
    /// Coordinates remain in the source bindings; jobs carry source indices only.
    ///
    /// # Errors
    /// Returns a packing error when the fit stream exceeds GPU index limits.
    pub fn prepare_helix_fits(&mut self) -> Result<(), crate::PackingError> {
        if !self.fit_jobs.is_empty() {
            return Ok(());
        }
        let mut lookup = HashMap::new();
        let guide_indices: HashMap<_, _> = self
            .traces
            .entities
            .iter()
            .enumerate()
            .filter_map(|(index, &entity)| {
                let (EntityKind::Atom, row) = EntityId(entity).unpack()? else {
                    return None;
                };
                Some((row, index))
            })
            .collect();
        let mut runs: Vec<Run> = Vec::new();
        super::helix_connections::seed(self, &mut lookup, &mut runs, &guide_indices)?;
        self.map_helix_recipes(&mut lookup, &mut runs, &guide_indices)?;
        super::helix_connections::bind(self, &runs)?;
        if runs.is_empty() && self.strand_fits.is_empty() {
            return Ok(());
        }
        let jobs = runs.len() + self.strand_fits.len();
        self.fit_jobs.resize(1 + jobs * 3, 0);
        self.fit_jobs[0] = super::super::draw_limits::vertex_index(jobs)?;
        for (index, run) in runs.iter().enumerate() {
            let header = 1 + index * 3;
            self.fit_jobs[header] = super::super::draw_limits::vertex_index(self.fit_jobs.len())?;
            self.fit_jobs[header + 1] = super::super::draw_limits::vertex_index(run.rows.len())?;
            self.fit_jobs[header + 2] = run.destination;
            self.fit_jobs.extend_from_slice(&run.rows);
        }
        for (index, run) in self.strand_fits.iter().enumerate() {
            let header = 1 + (runs.len() + index) * 3;
            self.fit_jobs[header] = super::super::draw_limits::vertex_index(self.fit_jobs.len())?;
            self.fit_jobs[header + 1] = super::super::draw_limits::vertex_index(run.rows.len())?
                | super::strands::STRAND_JOB;
            self.fit_jobs[header + 2] = run.destination;
            self.fit_jobs.extend_from_slice(&run.rows);
        }
        Ok(())
    }

    fn map_helix_recipes(
        &mut self,
        lookup: &mut HashMap<(u32, u32), usize>,
        runs: &mut Vec<Run>,
        guide_indices: &HashMap<u32, usize>,
    ) -> Result<(), crate::PackingError> {
        for recipe in &mut self.deformations {
            if recipe.parameter[3].to_bits() != 6 {
                continue;
            }
            let key = (recipe.controls[2], recipe.controls[3]);
            let index = *lookup.entry(key).or_insert_with(|| {
                let index = runs.len();
                runs.push(Run::default());
                index
            });
            let run = &mut runs[index];
            if run.rows.is_empty() {
                run.destination =
                    super::super::draw_limits::vertex_index(self.traces.properties.len())?;
                self.traces
                    .properties
                    .resize(self.traces.properties.len() + 24, 0.0);
                run.fill_source_guides(key, guide_indices, &self.traces.entities)?;
            }
            for row in recipe.controls[..2].iter().copied() {
                if let std::collections::hash_map::Entry::Vacant(entry) = run.seen.entry(row) {
                    entry.insert(run.rows.len());
                    run.rows.push(row);
                }
            }
            recipe.parameter[1] = f32::from_bits(run.destination);
        }
        for recipe in &mut self.deformations {
            if recipe.parameter[3].to_bits() != 6 {
                continue;
            }
            let run = &runs[lookup[&(recipe.controls[2], recipe.controls[3])]];
            let midpoint = match run.rows.len().to_f32() {
                Some(count) => (count - 1.0) * 0.5,
                None => {
                    return Err(crate::PackingError::IndexOverflow {
                        resource: "helix guides",
                        index: u64::MAX,
                    });
                }
            };
            if let Some(index) = run
                .seen
                .get(&recipe.controls[0])
                .and_then(ToPrimitive::to_f32)
            {
                recipe.parameter[2] = index - midpoint;
            }
        }
        Ok(())
    }

    /// Job headers and source guide rows for GPU helix fitting.
    #[must_use]
    pub fn helix_fit_jobs(&self) -> &[u32] {
        &self.fit_jobs
    }
}

impl Run {
    pub(super) fn fill_source_guides(
        &mut self,
        key: (u32, u32),
        guide_indices: &HashMap<u32, usize>,
        entities: &[u32],
    ) -> Result<(), crate::PackingError> {
        if let (Some(&first), Some(&last)) = (guide_indices.get(&key.0), guide_indices.get(&key.1))
        {
            let guides = entities
                .get(first..=last)
                .ok_or(crate::PackingError::GeometryBudget {
                    resource: "helix guide order",
                })?;
            for &entity in guides {
                if let Some((EntityKind::Atom, row)) = EntityId(entity).unpack() {
                    self.seen.insert(row, self.rows.len());
                    self.rows.push(row);
                }
            }
        }
        Ok(())
    }
}
