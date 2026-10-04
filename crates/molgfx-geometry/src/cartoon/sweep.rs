//! Sweeps source-backed ribbon cross-sections along transport frames.

use super::cross_section::cross_section;
use super::ends::append_end_caps;
use super::profiles::{profile_color, profile_extents_for_guide, sample_profile};
use super::ribbon::{
    PROFILE_SIDES, RibbonBuild, RibbonDeformation, RibbonVertex, SplineProfile, control_rows,
};
use super::traces::GuideKind;
use molgfx_core::{CartoonProfile, SecondaryStructure};
use molgfx_math::Vec3;

#[derive(Clone, Copy)]
pub(super) struct RibbonTrace<'a> {
    pub(super) points: &'a [Vec3],
    pub(super) entities: &'a [u32],
    pub(super) styles: &'a [SecondaryStructure],
    pub(super) property_base: Option<u32>,
    pub(super) property_count: usize,
    pub(super) normals: &'a [Vec3],
    pub(super) guides: &'a [GuideKind],
}

pub(super) fn append_ribbon(
    input: RibbonTrace<'_>,
    build: &mut RibbonBuild<'_>,
) -> Result<(), crate::PackingError> {
    let RibbonTrace {
        points: trace,
        entities,
        styles,
        normals,
        guides,
        ..
    } = input;
    // The adaptive sampler sees the shape of the curve and nothing else, so a
    // twisting ribbon has to tell it how far it turns; without that a glycan
    // earns one sample per sugar and folds. The demand is per interval, so a
    // run of coplanar rings still costs what a straight ribbon costs.
    if build.params.profile == SplineProfile::Twister {
        super::twist::twist_demand(trace, normals, build.demand);
    } else {
        build.demand.clear();
    }
    molgfx_math::sample_catmull_rom_demanding(
        trace,
        build.params.tolerance,
        build.params.max_steps,
        build.demand,
        build.samples,
    );
    if matches!(
        build.params.profile,
        SplineProfile::Cartoon | SplineProfile::Rocket
    ) && build.params.arrow_factor > 0.0
    {
        super::profiles::arrow_shoulders(styles, build.samples);
    }
    molgfx_math::parallel_transport(build.samples, build.frames);
    if build.samples.len() < 2 || build.samples.len() != build.frames.len() {
        return Ok(());
    }
    if build.params.profile == SplineProfile::Twister {
        super::twist::orient_to_rings(build.samples, normals, build.frames);
    }
    let shell_indices: u64 = build
        .samples
        .windows(2)
        .map(|samples| {
            if flat_interval(samples, styles, guides, build.params) {
                24
            } else {
                96
            }
        })
        .sum();
    super::draw_limits::check_sweep(
        build.vertices.len(),
        build.indices.len(),
        build.samples.len(),
        shell_indices,
        build.params.profile == SplineProfile::Twister,
    )?;
    let base_vertex = super::draw_limits::vertex_index(build.vertices.len())?;
    let half_width = build.params.width.abs() * 0.5;
    let half_thickness = build.params.thickness.abs() * 0.5;
    build.vertices.reserve(build.samples.len() * PROFILE_SIDES);
    build
        .deformations
        .reserve(build.samples.len() * PROFILE_SIDES);
    append_vertices(input, build);
    let shell_capacity =
        usize::try_from(shell_indices).map_err(|_| crate::PackingError::IndexOverflow {
            resource: "ribbon draw indices",
            index: shell_indices,
        })?;
    build.indices.reserve(shell_capacity);
    for ring in 0..build.samples.len() - 1 {
        let flat = flat_interval(
            &build.samples[ring..=ring + 1],
            styles,
            guides,
            build.params,
        );
        append_ring(ring, base_vertex, flat, build.indices)?;
    }
    append_end_caps(entities, styles, guides, half_width, half_thickness, build)
}

fn flat_interval(
    samples: &[molgfx_math::CurveSample],
    styles: &[SecondaryStructure],
    guides: &[GuideKind],
    params: super::ribbon::RibbonParams,
) -> bool {
    samples.iter().all(|sample| {
        let segment = sample.segment as usize;
        let style = match styles.get(segment) {
            Some(&style) => style,
            None => SecondaryStructure::Unknown,
        };
        sample_profile(params, style, guides.get(segment).copied()) == CartoonProfile::Square
    })
}

fn append_vertices(input: RibbonTrace<'_>, build: &mut RibbonBuild<'_>) {
    let RibbonTrace {
        entities,
        styles,
        property_base,
        property_count,
        guides,
        ..
    } = input;
    for (sample, frame) in build.samples.iter().zip(build.frames.iter()) {
        let segment = usize::try_from(sample.segment)
            .into_iter()
            .fold(usize::MAX, |_, value| value);
        let entity_id = match entities.get(segment) {
            Some(&value) => value,
            None => u32::MAX,
        };
        let style = match styles.get(segment) {
            Some(&value) => value,
            None => SecondaryStructure::Unknown,
        };
        let (width, thickness) = profile_extents_for_guide(
            build.params,
            style,
            sample.parameter,
            styles,
            segment,
            guides.get(segment).copied(),
        );
        let controls = control_rows(entities, segment);
        let radius_controls = radius_control_rows(property_base, property_count, segment);
        for side in 0..PROFILE_SIDES {
            let (x, y, normal) = cross_section(
                sample_profile(build.params, style, guides.get(segment).copied()),
                side,
                width,
                thickness,
            );
            let offset = frame.normal * (x * width) + frame.binormal * (y * thickness);
            // A zero-width arrow tip has a vanishing ellipse gradient. The
            // transported face normal keeps that shared tip finite.
            let normal =
                match (frame.normal * normal[0] + frame.binormal * normal[1]).try_normalize() {
                    Some(normal) => normal,
                    None => frame.normal,
                };
            build.vertices.push(RibbonVertex {
                position: (sample.position + offset).to_array(),
                entity_id,
                normal: normal.to_array(),
                color: profile_color(build.params.profile, y, build.params.color),
            });
            build.deformations.push(RibbonDeformation {
                controls,
                parameter: [
                    sample.parameter,
                    f32::from_bits(radius_controls[0]),
                    f32::from_bits(radius_controls[1]),
                    0.0,
                ],
            });
        }
    }
}

fn radius_control_rows(base: Option<u32>, count: usize, segment: usize) -> [u32; 2] {
    let Some(base) = base else {
        return [u32::MAX; 2];
    };
    let Some(right) = segment.checked_add(1) else {
        return [u32::MAX; 2];
    };
    if right >= count {
        return [u32::MAX; 2];
    }
    let (Ok(left), Ok(right)) = (u32::try_from(segment), u32::try_from(right)) else {
        return [u32::MAX; 2];
    };
    match (base.checked_add(left), base.checked_add(right)) {
        (Some(left), Some(right)) => [left, right],
        _ => [u32::MAX; 2],
    }
}

fn append_ring(
    ring: usize,
    base_vertex: u32,
    flat: bool,
    indices: &mut Vec<u32>,
) -> Result<(), crate::PackingError> {
    let base = super::draw_limits::vertex_index(ring * PROFILE_SIDES)? + base_vertex;
    let stride = super::draw_limits::vertex_index(PROFILE_SIDES)?;
    for side in 0..PROFILE_SIDES {
        if flat && side % 4 != 0 {
            continue;
        }
        let current = super::draw_limits::vertex_index(side)?;
        let next =
            super::draw_limits::vertex_index((side + if flat { 3 } else { 1 }) % PROFILE_SIDES)?;
        indices.extend_from_slice(&[
            base + current,
            base + stride + next,
            base + stride + current,
            base + current,
            base + next,
            base + stride + next,
        ]);
    }
    Ok(())
}
