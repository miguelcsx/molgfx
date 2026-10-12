//! Shared extraction-to-sweep lowering over independent polymer traces.

use super::{
    GuideProperties, RibbonBuild, RibbonMesh, RibbonParams, SplineProfile, plane_rows_slice,
    plane_slice,
};
use crate::cartoon::sweep::{RibbonTrace, append_ribbon};

impl RibbonMesh {
    pub(super) fn generate_traces(
        &mut self,
        params: RibbonParams,
        properties: GuideProperties,
    ) -> Result<(), crate::PackingError> {
        self.fit_jobs.clear();
        self.classic = params.profile == SplineProfile::Rocket;
        if params.profile == SplineProfile::Rocket {
            self.fitted
                .resize(self.traces.points.len(), molgfx_math::Vec3::ZERO);
            let ranges = if self.traces.parent_ranges.is_empty() {
                &self.traces.ranges
            } else {
                &self.traces.parent_ranges
            };
            for range in ranges {
                super::super::rocket::project_trace_into(
                    &self.traces.points[range.points.clone()],
                    &self.traces.styles[range.points.clone()],
                    &mut self.fitted[range.points.clone()],
                );
            }
        }
        let mut build = RibbonBuild {
            params,
            samples: &mut self.samples,
            demand: &mut self.demand,
            frames: &mut self.frames,
            vertices: &mut self.vertices,
            indices: &mut self.indices,
            deformations: &mut self.deformations,
        };
        for (index, range) in self.traces.ranges.iter().enumerate() {
            let points = &self.traces.points[range.points.clone()];
            let styles = &self.traces.styles[range.points.clone()];
            let points = if params.profile == SplineProfile::Rocket {
                &self.fitted[range.points.clone()]
            } else if params.profile == SplineProfile::Cartoon {
                super::super::rocket::filter_strands(points, styles, &mut self.fitted);
                &self.fitted[..]
            } else {
                points
            };
            let property_base = match properties {
                GuideProperties::RadiusSources => {
                    super::super::draw_limits::vertex_index(range.points.start).map(Some)
                }
                GuideProperties::Unmapped => Ok(None),
            };
            let first_recipe = build.deformations.len();
            let result = property_base.and_then(|property_base| {
                append_ribbon(
                    RibbonTrace {
                        points,
                        entities: &self.traces.entities[range.points.clone()],
                        styles: &self.traces.styles[range.points.clone()],
                        property_base,
                        property_count: range.points.len(),
                        normals: plane_slice(&self.traces.normals, range.points.clone()),
                        guides: &self.traces.guides[range.points.clone()],
                        plane_rows: plane_rows_slice(&self.traces.plane_rows, range.points.clone()),
                        parameter_range: self.traces.visible_parameters.get(index).copied(),
                    },
                    &mut build,
                )
            });
            let result = result.and_then(|()| {
                if params.profile == SplineProfile::Rocket
                    || (params.profile == SplineProfile::Cartoon
                        && styles.iter().any(|style| style.is_strand()))
                {
                    super::schematic::prepare(
                        &mut build.deformations[first_recipe..],
                        &self.traces.entities[range.points.clone()],
                        styles,
                        plane_rows_slice(&self.traces.plane_rows, range.points.clone()),
                        &mut self.traces.properties,
                        params.profile == SplineProfile::Cartoon,
                    )?;
                }
                Ok(())
            });
            if let Err(error) = result {
                build.vertices.clear();
                build.indices.clear();
                build.deformations.clear();
                return Err(error);
            }
        }
        Ok(())
    }
}
