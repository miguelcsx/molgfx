//! Retains spline controls and strand-filter neighbours around selected spans.
//! Extraction costs O(guides + spans); controls retain original atom addresses.

use super::{GuidePolicy, PolymerTraces, TracePolicy, TraceRange};
use molgfx_core::{AtomSelection, EntityId, EntityKind, SecondaryStructure};
use num_traits::ToPrimitive as _;

pub(in crate::cartoon) fn extract_contextual_traces(
    structure: &molframe::Structure,
    selection: &AtomSelection,
    secondary: &[SecondaryStructure],
    max_gap: f32,
    guide: GuidePolicy,
    output: &mut PolymerTraces,
) -> Result<(), crate::PackingError> {
    super::extract_traces(
        structure,
        selection,
        secondary,
        max_gap,
        TracePolicy {
            minimum_points: 2,
            guide,
            context: true,
        },
        output,
    )?;
    let secondary_bounds = if matches!(guide, GuidePolicy::Protein) {
        secondary_bounds(&output.styles, &output.ranges)
    } else {
        Vec::new()
    };
    for range in &output.ranges {
        let mut start = None;
        for point in range.points.clone() {
            let selected = match EntityId(output.entities[point]).unpack() {
                Some((EntityKind::Atom, row)) => selection.contains(row),
                _ => false,
            };
            if selected {
                if start.is_none() {
                    start = Some(point);
                }
            } else if let Some(first) = start.take() {
                append_span(
                    range,
                    first,
                    point,
                    if matches!(guide, GuidePolicy::Phosphate) {
                        2
                    } else {
                        3
                    },
                    &secondary_bounds,
                    &mut output.context_ranges,
                    &mut output.visible_parameters,
                )?;
            }
        }
        if let Some(first) = start {
            append_span(
                range,
                first,
                range.points.end,
                if matches!(guide, GuidePolicy::Phosphate) {
                    2
                } else {
                    3
                },
                &secondary_bounds,
                &mut output.context_ranges,
                &mut output.visible_parameters,
            )?;
        }
    }
    std::mem::swap(&mut output.ranges, &mut output.context_ranges);
    std::mem::swap(&mut output.parent_ranges, &mut output.context_ranges);
    Ok(())
}

fn append_span(
    trace: &TraceRange,
    first: usize,
    end: usize,
    halo: usize,
    secondary_bounds: &[[usize; 2]],
    ranges: &mut Vec<TraceRange>,
    parameters: &mut Vec<[f32; 2]>,
) -> Result<(), crate::PackingError> {
    let mut context_start = first.saturating_sub(halo).max(trace.points.start);
    let mut context_end = end.saturating_add(halo).min(trace.points.end);
    if let Some(bounds) = secondary_bounds.get(context_start) {
        context_start = bounds[0];
    }
    if let Some(bounds) = secondary_bounds.get(context_end - 1) {
        context_end = bounds[1];
    }
    if context_end - context_start > 1_000_000 {
        return Err(crate::PackingError::GeometryBudget {
            resource: "polymer guide span",
        });
    }
    let minimum = if first == trace.points.start {
        0.0
    } else {
        number(first - context_start)? - 0.5
    };
    let maximum = if end == trace.points.end {
        number(end - context_start - 1)?
    } else {
        number(end - context_start)? - 0.5
    };
    ranges.push(TraceRange {
        chain: trace.chain,
        points: context_start..context_end,
    });
    parameters.push([minimum, maximum]);
    Ok(())
}

fn number(index: usize) -> Result<f32, crate::PackingError> {
    index.to_f32().ok_or(crate::PackingError::GeometryBudget {
        resource: "polymer guide parameter",
    })
}

// Every guide is visited once, so disjoint selections share whole-run context without rescanning it.
fn secondary_bounds(styles: &[SecondaryStructure], ranges: &[TraceRange]) -> Vec<[usize; 2]> {
    let mut bounds = vec![[0; 2]; styles.len()];
    for trace in ranges {
        let mut first = trace.points.start;
        while first < trace.points.end {
            let class = |style: SecondaryStructure| {
                if style.is_helix() {
                    1
                } else if style.is_strand() {
                    2
                } else {
                    0
                }
            };
            let category = class(styles[first]);
            let mut end = first + 1;
            if category != 0 {
                while end < trace.points.end && class(styles[end]) == category {
                    end += 1;
                }
            }
            bounds[first..end].fill([first, end]);
            first = end;
        }
    }
    bounds
}
