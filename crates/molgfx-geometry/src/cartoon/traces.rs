//! Guide traces: the ordered points a cartoon ribbon is swept along.
//!
//! Extraction is separate from extrusion because the two answer different
//! questions — which residues form one continuous run, and what solid that run
//! sweeps — and because a polymer backbone is not the only thing that yields a
//! trace (see the glycan walker).

use molgfx_core::SecondaryStructure;
use molgfx_math::Vec3;
use std::ops::Range;
mod context;
mod guides;
pub(super) use context::extract_contextual_traces;
pub(super) use guides::polymer_guide;

/// Reusable topology-ordered guide traces extracted from one structure.
#[derive(Clone, Debug, Default)]
pub struct PolymerTraces {
    pub(super) points: Vec<Vec3>,
    pub(super) entities: Vec<u32>,
    pub(super) styles: Vec<SecondaryStructure>,
    pub(super) guides: Vec<GuideKind>,
    pub(super) properties: Vec<f32>,
    /// Source oxygen rows used to orient protein ribbons as coordinates move.
    pub(super) plane_rows: Vec<u32>,
    /// Per-point thickness directions derived from residue geometry.
    pub(super) normals: Vec<Vec3>,
    pub(super) ranges: Vec<TraceRange>,
    /// Complete original fragments retained when visible ranges clip a selection.
    pub(super) parent_ranges: Vec<TraceRange>,
    pub(super) visible_parameters: Vec<[f32; 2]>,
    context_ranges: Vec<TraceRange>,
}

impl PolymerTraces {
    pub(super) fn clear(&mut self) {
        self.points.clear();
        self.entities.clear();
        self.styles.clear();
        self.guides.clear();
        self.properties.clear();
        self.normals.clear();
        self.plane_rows.clear();
        self.ranges.clear();
        self.parent_ranges.clear();
        self.visible_parameters.clear();
        self.context_ranges.clear();
    }

    fn finish_span(&mut self, chain: u32, start: &mut usize, minimum_points: usize) {
        finish_trace(
            chain,
            *start,
            self.points.len(),
            &mut self.ranges,
            minimum_points,
        );
        *start = self.points.len();
    }

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
        self.guides.clear();
        self.guides.resize(points.len(), GuideKind::RingCenter);
        self.styles.clear();
        self.styles
            .resize(points.len(), SecondaryStructure::Unknown);
        self.properties.clear();
        self.properties.resize(points.len(), f32::NAN);
        self.points = points;
        self.entities = entities;
        self.normals = normals;
        self.plane_rows.clear();
        self.ranges = ranges;
        self.parent_ranges.clear();
        self.visible_parameters.clear();
        self.context_ranges.clear();
    }
}

/// Source atom used to place a backbone cross-section.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum GuideKind {
    AlphaCarbon,
    SugarCarbon,
    Phosphate,
    RingCenter,
}

#[derive(Clone, Copy)]
pub(super) enum GuidePolicy {
    Sugar,
    Phosphate,
    Protein,
}

#[derive(Clone, Copy)]
struct TracePolicy {
    minimum_points: usize,
    guide: GuidePolicy,
    context: bool,
}

impl GuidePolicy {
    fn select<'a>(
        self,
        residue: molframe::ResidueRef<'a>,
        selection: &molgfx_core::AtomSelection,
        preferred: Option<molframe::AltId>,
    ) -> Option<(molframe::AtomRef<'a>, GuideKind)> {
        guides::selected_guide(
            residue,
            selection,
            matches!(self, Self::Phosphate),
            preferred,
        )
        .filter(|(_, kind)| !matches!(self, Self::Protein) || *kind == GuideKind::AlphaCarbon)
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
    extract_backbone_traces(structure, selection, secondary, max_gap, false, output)
}

/// Extracts selected alpha-carbon and sugar-carbon spans with their spline and
/// strand-filter neighbours. Unselected guides shape the curve without owning
/// visible geometry. Missing guides, conformers and sequence gaps split spans.
///
/// # Errors
/// Returns a geometry error for malformed source rows or unaddressable spans.
pub fn extract_contextual_polymer_traces(
    structure: &molframe::Structure,
    selection: &molgfx_core::AtomSelection,
    secondary: &[SecondaryStructure],
    max_gap: f32,
    output: &mut PolymerTraces,
) -> Result<(), crate::PackingError> {
    extract_contextual_traces(
        structure,
        selection,
        secondary,
        max_gap,
        GuidePolicy::Sugar,
        output,
    )
}

/// Extracts protein alpha carbons and nucleic backbone phosphates, retaining
/// spline neighbours around selected spans and half intervals at their boundaries.
///
/// # Errors
/// Returns a geometry error for malformed source rows.
pub fn extract_phosphate_traces(
    structure: &molframe::Structure,
    selection: &molgfx_core::AtomSelection,
    secondary: &[SecondaryStructure],
    max_gap: f32,
    output: &mut PolymerTraces,
) -> Result<(), crate::PackingError> {
    extract_contextual_traces(
        structure,
        selection,
        secondary,
        max_gap,
        GuidePolicy::Phosphate,
        output,
    )
}

/// Extracts alpha-carbon traces for discrete protein secondary solids.
/// Nucleic sugar and phosphate guides belong to ribbon or backbone methods.
///
/// # Errors
/// Returns a geometry error when a source guide cannot be encoded.
pub fn extract_protein_traces(
    structure: &molframe::Structure,
    selection: &molgfx_core::AtomSelection,
    secondary: &[SecondaryStructure],
    max_gap: f32,
    output: &mut PolymerTraces,
) -> Result<(), crate::PackingError> {
    extract_contextual_traces(
        structure,
        selection,
        secondary,
        max_gap,
        GuidePolicy::Protein,
        output,
    )
}

pub(super) fn extract_backbone_traces(
    structure: &molframe::Structure,
    selection: &molgfx_core::AtomSelection,
    secondary: &[SecondaryStructure],
    max_gap: f32,
    phosphate: bool,
    output: &mut PolymerTraces,
) -> Result<(), crate::PackingError> {
    extract_traces(
        structure,
        selection,
        secondary,
        max_gap,
        TracePolicy {
            minimum_points: 2,
            guide: if phosphate {
                GuidePolicy::Phosphate
            } else {
                GuidePolicy::Sugar
            },
            context: false,
        },
        output,
    )
}

pub(super) fn extract_direction_traces(
    structure: &molframe::Structure,
    secondary: &[SecondaryStructure],
    max_gap: f32,
    output: &mut PolymerTraces,
) -> Result<(), crate::PackingError> {
    extract_traces(
        structure,
        &molgfx_core::AtomSelection::All,
        secondary,
        max_gap,
        TracePolicy {
            minimum_points: 1,
            guide: GuidePolicy::Sugar,
            context: false,
        },
        output,
    )
}

fn extract_traces(
    structure: &molframe::Structure,
    selection: &molgfx_core::AtomSelection,
    secondary: &[SecondaryStructure],
    max_gap: f32,
    policy: TracePolicy,
    output: &mut PolymerTraces,
) -> Result<(), crate::PackingError> {
    output.clear();
    let TracePolicy {
        minimum_points,
        guide: guide_policy,
        context,
    } = policy;
    let max_gap_sq = max_gap.max(0.0).powi(2);
    let data = structure.engine().data();
    for chain in structure.chains() {
        // Ligands, waters and glycans are never backbone, even when an atom is
        // named CA (calcium ions, ligand alpha-carbon-like names).
        let entity_kind = chain
            .entity()
            .and_then(|entity| data.topology.entities.kind(entity));
        if matches!(
            entity_kind,
            Some(
                molframe::EntityKind::NonPolymer
                    | molframe::EntityKind::Water
                    | molframe::EntityKind::Branched
            )
        ) {
            continue;
        }
        let chain_id = chain.index().get();
        let mut trace_start = output.points.len();
        let mut previous_sequence = None;
        let mut previous_alt = None;
        let mut preferred = chain.residues().find_map(|residue| {
            let (atom, _) = guide_policy.select(residue, selection, None)?;
            if !selection.contains(atom.index().get()) {
                return None;
            }
            atom.alt_id().filter(|alt| !alt.is_blank())
        });
        for residue in chain.residues() {
            let sequence = residue.label_seq_id();
            let missing_interval = missing_sequence_interval(previous_sequence, sequence);
            previous_sequence = sequence;
            if missing_interval && output.points.len() > trace_start {
                output.finish_span(chain_id, &mut trace_start, minimum_points);
            }
            let guide = guide_policy.select(residue, selection, preferred);
            let Some((atom, guide_kind)) = guide else {
                output.finish_span(chain_id, &mut trace_start, minimum_points);
                previous_alt = None;
                continue;
            };
            if !context && !selection.contains(atom.index().get()) {
                output.finish_span(chain_id, &mut trace_start, minimum_points);
                previous_alt = None;
                continue;
            }
            let position = atom.position().map(Vec3::from);
            let Some(position) = position else {
                output.finish_span(chain_id, &mut trace_start, minimum_points);
                previous_alt = None;
                continue;
            };
            let alt = match atom.alt_id() {
                Some(value) => value,
                None => molframe::AltId::BLANK,
            };
            if selection.contains(atom.index().get()) && !alt.is_blank() {
                preferred = Some(alt);
            }
            let incompatible =
                previous_alt.is_some_and(|previous| !molframe::altloc_compatible(previous, alt));
            if (output.points.len() > trace_start
                && (incompatible || output.guides.last() != Some(&guide_kind)))
                || output
                    .points
                    .last()
                    .is_some_and(|previous| previous.distance_squared(position) > max_gap_sq)
            {
                output.finish_span(chain_id, &mut trace_start, minimum_points);
            }
            append_guide(residue, atom, guide_kind, position, secondary, output)?;
            previous_alt = Some(alt);
        }
        finish_trace(
            chain_id,
            trace_start,
            output.points.len(),
            &mut output.ranges,
            minimum_points,
        );
    }
    Ok(())
}

fn append_guide(
    residue: molframe::ResidueRef<'_>,
    atom: molframe::AtomRef<'_>,
    guide_kind: GuideKind,
    position: Vec3,
    secondary: &[SecondaryStructure],
    output: &mut PolymerTraces,
) -> Result<(), crate::PackingError> {
    let entity =
        molgfx_core::EntityId::pack(molgfx_core::EntityKind::Atom, u64::from(atom.index().get()))?;
    output.points.push(position);
    let cue = guides::plane_cue(residue, atom, guide_kind);
    output.plane_rows.push(match cue {
        Some(atom) => atom.index().get(),
        None => u32::MAX,
    });
    output
        .normals
        .push(match cue.and_then(molframe::AtomRef::position) {
            Some(cue) => Vec3::from(cue) - position,
            None => Vec3::ZERO,
        });
    output.guides.push(guide_kind);
    output.entities.push(entity.0);
    let residue_index = residue.index().as_usize();
    output.styles.push(match secondary.get(residue_index) {
        Some(&style) => style,
        None => SecondaryStructure::Unknown,
    });
    output
        .properties
        .push(atom.b_factor().into_iter().fold(f32::NAN, |_, value| value));
    Ok(())
}

pub(super) fn missing_sequence_interval(previous: Option<i32>, current: Option<i32>) -> bool {
    match (previous, current) {
        (Some(previous), Some(current)) => i64::from(current) > i64::from(previous) + 1,
        _ => false,
    }
}

fn finish_trace(
    chain: u32,
    start: usize,
    end: usize,
    ranges: &mut Vec<TraceRange>,
    minimum_points: usize,
) {
    if end.saturating_sub(start) >= minimum_points {
        ranges.push(TraceRange {
            chain,
            points: start..end,
        });
    }
}

#[cfg(test)]
#[path = "traces_tests.rs"]
mod tests;
