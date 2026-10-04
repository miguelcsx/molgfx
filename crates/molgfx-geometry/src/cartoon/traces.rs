//! Guide traces: the ordered points a cartoon ribbon is swept along.
//!
//! Extraction is separate from extrusion because the two answer different
//! questions — which residues form one continuous run, and what solid that run
//! sweeps — and because a polymer backbone is not the only thing that yields a
//! trace (see the glycan walker).

use molgfx_core::SecondaryStructure;
use molgfx_math::Vec3;
use std::ops::Range;

/// Reusable topology-ordered guide traces extracted from one structure.
#[derive(Clone, Debug, Default)]
pub struct PolymerTraces {
    pub(super) points: Vec<Vec3>,
    pub(super) entities: Vec<u32>,
    pub(super) styles: Vec<SecondaryStructure>,
    pub(super) properties: Vec<f32>,
    /// Per-point orientation the ribbon's flat face is held in, where the
    /// source residue has one. A polymer guide atom carries no such plane, so
    /// this stays empty for backbone traces and the ribbon keeps its twist-free
    /// transport frames.
    pub(super) normals: Vec<Vec3>,
    pub(super) ranges: Vec<TraceRange>,
}

impl PolymerTraces {
    /// The extracted traces.
    #[must_use]
    pub fn ranges(&self) -> &[TraceRange] {
        &self.ranges
    }

    /// Replaces the contents with caller-built traces.
    ///
    /// Used by extractors that do not walk a linear polymer backbone — a
    /// branched glycan, for instance — and therefore cannot fill the arrays in
    /// one sequential sweep.
    pub(super) fn rebuild_from(
        &mut self,
        points: Vec<Vec3>,
        entities: Vec<u32>,
        normals: Vec<Vec3>,
        ranges: Vec<TraceRange>,
    ) {
        self.styles.clear();
        self.styles
            .resize(points.len(), SecondaryStructure::Unknown);
        self.properties.clear();
        self.properties.resize(points.len(), f32::NAN);
        self.points = points;
        self.entities = entities;
        self.normals = normals;
        self.ranges = ranges;
    }
}

/// One contiguous polymer guide trace in the shared output arrays.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TraceRange {
    /// Stable source chain index.
    pub chain: u32,
    /// Range into the extracted points and entities.
    pub points: Range<usize>,
}

/// Cartesian distance beyond which a cartoon trace is broken rather than
/// bridged.
///
/// A missing guide atom that is genuinely absent from a deposited model and a
/// deliberately large spatial step are the same fact geometrically, so this is
/// expressed as a length rather than a residue count: the two residues either
/// are far enough apart that the ribbon between them would be a fabricated
/// bridge, or they are not. The value is just above the longest credible
/// consecutive C-alpha separation, so an ordered backbone stays one trace and a
/// chain break does not.
pub const CARTOON_GAP_CUTOFF: f32 = 8.0;

/// Extracts C-alpha or C4-prime guide atoms in topology order. Missing guide
/// atoms and caller-defined spatial gaps split traces instead of drawing a
/// physically false bridge. Canonical sequence gaps split even when the
/// surviving guides are spatially close; author numbering is not a sequence
/// position and therefore does not establish a gap. All output vectors are reused.
///
/// # Errors
///
/// Returns [`crate::PackingError`] when a guide atom row cannot be encoded.
pub fn extract_polymer_traces(
    structure: &molframe::Structure,
    selection: &molgfx_core::AtomSelection,
    secondary: &[SecondaryStructure],
    max_gap: f32,
    output: &mut PolymerTraces,
) -> Result<(), crate::PackingError> {
    output.points.clear();
    output.entities.clear();
    output.styles.clear();
    output.properties.clear();
    output.normals.clear();
    output.ranges.clear();
    let max_gap_sq = max_gap.max(0.0).powi(2);
    for chain in structure.chains() {
        let chain_id = chain.index().get();
        let mut trace_start = output.points.len();
        let mut previous_sequence = None;
        for residue in chain.residues() {
            let sequence = residue.label_seq_id();
            let missing_interval = match (previous_sequence, sequence) {
                (Some(previous), Some(current)) => i64::from(current) > i64::from(previous) + 1,
                _ => false,
            };
            previous_sequence = sequence;
            if missing_interval && output.points.len() > trace_start {
                finish_trace(
                    chain_id,
                    trace_start,
                    output.points.len(),
                    &mut output.ranges,
                );
                trace_start = output.points.len();
            }
            let guide = residue.atom("CA").or_else(|| residue.atom("C4'"));
            let Some(atom) = guide else {
                finish_trace(
                    chain_id,
                    trace_start,
                    output.points.len(),
                    &mut output.ranges,
                );
                trace_start = output.points.len();
                continue;
            };
            if !selection.contains(atom.index().get()) {
                finish_trace(
                    chain_id,
                    trace_start,
                    output.points.len(),
                    &mut output.ranges,
                );
                trace_start = output.points.len();
                continue;
            }
            let position = atom.position().map(Vec3::from);
            let Some(position) = position else {
                finish_trace(
                    chain_id,
                    trace_start,
                    output.points.len(),
                    &mut output.ranges,
                );
                trace_start = output.points.len();
                continue;
            };
            if output
                .points
                .last()
                .is_some_and(|previous| previous.distance_squared(position) > max_gap_sq)
            {
                finish_trace(
                    chain_id,
                    trace_start,
                    output.points.len(),
                    &mut output.ranges,
                );
                trace_start = output.points.len();
            }
            let entity = molgfx_core::EntityId::pack(
                molgfx_core::EntityKind::Atom,
                u64::from(atom.index().get()),
            )?;
            output.points.push(position);
            output.entities.push(entity.0);
            let residue_index = residue.index().as_usize();
            output.styles.push(match secondary.get(residue_index) {
                Some(&style) => style,
                None => SecondaryStructure::Unknown,
            });
            output
                .properties
                .push(atom.b_factor().into_iter().fold(f32::NAN, |_, value| value));
        }
        finish_trace(
            chain_id,
            trace_start,
            output.points.len(),
            &mut output.ranges,
        );
    }
    Ok(())
}

fn finish_trace(chain: u32, start: usize, end: usize, ranges: &mut Vec<TraceRange>) {
    if end.saturating_sub(start) >= 2 {
        ranges.push(TraceRange {
            chain,
            points: start..end,
        });
    }
}

#[cfg(test)]
#[path = "traces_tests.rs"]
mod tests;
