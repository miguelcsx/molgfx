//! Compact fit jobs and borrowed guide indices, prepared in O(vertices + guides).

use super::RibbonMesh;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
struct Run {
    rows: Vec<u32>,
    seen: HashSet<u32>,
    destination: u32,
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
        let mut runs: Vec<Run> = Vec::new();
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
            }
            for row in recipe.controls[..2].iter().copied() {
                if run.seen.insert(row) {
                    run.rows.push(row);
                }
            }
            recipe.parameter[1] = f32::from_bits(run.destination);
        }
        if runs.is_empty() {
            return Ok(());
        }
        self.fit_jobs.resize(1 + runs.len() * 3, 0);
        self.fit_jobs[0] = super::super::draw_limits::vertex_index(runs.len())?;
        for (index, run) in runs.iter().enumerate() {
            let header = 1 + index * 3;
            self.fit_jobs[header] = super::super::draw_limits::vertex_index(self.fit_jobs.len())?;
            self.fit_jobs[header + 1] = super::super::draw_limits::vertex_index(run.rows.len())?;
            self.fit_jobs[header + 2] = run.destination;
            self.fit_jobs.extend_from_slice(&run.rows);
        }
        Ok(())
    }

    /// Job headers and source guide rows for GPU helix fitting.
    #[must_use]
    pub fn helix_fit_jobs(&self) -> &[u32] {
        &self.fit_jobs
    }
}
