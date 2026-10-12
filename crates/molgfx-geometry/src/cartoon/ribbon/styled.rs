//! Direct borrowed-guide authoring through the shared sweep and discrete solids.
use super::{RibbonBuild, RibbonMesh, RibbonParams, SplineProfile, schematic};
use crate::cartoon::{
    directions::DirectionBuild,
    error::CartoonError,
    sweep::{RibbonTrace, append_ribbon},
};
use molgfx_core::SecondaryStructure;
use molgfx_math::Vec3;
use std::collections::HashSet;

impl RibbonMesh {
    /// Regenerates a ribbon with one reversible secondary-structure style per
    /// control point. Missing styles resolve to coil without changing topology.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CartoonError`] for GPU index limits or a guide without a direction.
    pub fn generate_styled(
        &mut self,
        trace: &[Vec3],
        entities: &[u32],
        styles: &[SecondaryStructure],
        params: RibbonParams,
    ) -> Result<(), CartoonError> {
        params.validate_classic_radius()?;
        self.vertices.clear();
        self.indices.clear();
        self.deformations.clear();
        self.traces.clear();
        self.fit_jobs.clear();
        self.strand_fits.clear();
        self.classic = params.profile == SplineProfile::Rocket;
        let source_trace = trace;
        let trace = if params.profile == SplineProfile::Rocket {
            super::super::rocket::project_trace(trace, styles, &mut self.fitted);
            &self.fitted[..]
        } else if params.profile == SplineProfile::Cartoon {
            super::super::rocket::filter_strands(trace, styles, &mut self.fitted);
            &self.fitted[..]
        } else {
            trace
        };
        let mut build = RibbonBuild {
            params,
            samples: &mut self.samples,
            demand: &mut self.demand,
            frames: &mut self.frames,
            vertices: &mut self.vertices,
            indices: &mut self.indices,
            deformations: &mut self.deformations,
        };
        let input = RibbonTrace {
            points: trace,
            entities,
            styles,
            property_base: None,
            property_count: 0,
            normals: &[],
            guides: &[],
            plane_rows: &[],
            parameter_range: None,
        };
        append_ribbon(input, &mut build)?;
        if params.direction_wedges {
            let result = super::super::directions::append_trace(
                input,
                &self.samples,
                &self.frames,
                None,
                &mut DirectionBuild {
                    params,
                    vertices: &mut self.vertices,
                    indices: &mut self.indices,
                    deformations: &mut self.deformations,
                },
            );
            if let Err(error) = result {
                self.clear();
                return Err(error);
            }
        }
        if params.profile == SplineProfile::Rocket
            || (params.profile == SplineProfile::Cartoon
                && styles.iter().any(|style| style.is_strand()))
        {
            schematic::prepare(
                &mut self.deformations,
                entities,
                styles,
                &[],
                &mut self.traces.properties,
                params.profile == SplineProfile::Cartoon,
            )?;
        }
        if params.profile == SplineProfile::Rocket {
            let selected: HashSet<_> = entities.iter().copied().collect();
            self.append_classic_strands(
                source_trace,
                entities,
                styles,
                std::slice::from_ref(&(0..source_trace.len())),
                params,
                &selected,
            )?;
            self.append_classic_helices(
                source_trace,
                entities,
                styles,
                std::slice::from_ref(&(0..source_trace.len())),
                params,
                &selected,
            )?;
        }
        Ok(())
    }
}
