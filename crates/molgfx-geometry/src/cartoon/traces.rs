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

/// Extracts C-alpha or C4-prime guide atoms in topology order. Missing guide
/// atoms and caller-defined spatial gaps split traces instead of drawing a
/// scientifically false bridge. All output vectors are reused.
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
        for residue in chain.residues() {
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
