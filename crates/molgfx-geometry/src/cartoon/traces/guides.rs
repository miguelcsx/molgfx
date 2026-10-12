//! Guide and plane atoms follow the authored selection and compatible conformers.

use super::GuideKind;
use molframe::{AtomRef, Element, ResidueRef};
use molgfx_core::AtomSelection;

pub(in crate::cartoon) fn polymer_guide<'a>(
    residue: ResidueRef<'a>,
    selection: &AtomSelection,
) -> Option<(AtomRef<'a>, GuideKind)> {
    selected_guide(residue, selection, false, None)
}

pub(super) fn selected_guide<'a>(
    residue: ResidueRef<'a>,
    selection: &AtomSelection,
    phosphate: bool,
    preferred: Option<molframe::AltId>,
) -> Option<(AtomRef<'a>, GuideKind)> {
    guide_atom(residue, "CA", Element::CARBON, selection, preferred)
        .map(|atom| (atom, GuideKind::AlphaCarbon))
        .or_else(|| {
            if phosphate {
                guide_atom(residue, "P", Element::PHOSPHORUS, selection, preferred)
                    .map(|atom| (atom, GuideKind::Phosphate))
            } else {
                guide_atom(residue, "C4'", Element::CARBON, selection, preferred)
                    .map(|atom| (atom, GuideKind::SugarCarbon))
            }
        })
}

fn guide_atom<'a>(
    residue: ResidueRef<'a>,
    name: &str,
    element: Element,
    selection: &AtomSelection,
    preferred: Option<molframe::AltId>,
) -> Option<AtomRef<'a>> {
    let mut first = None;
    let mut conformer = None;
    for atom in residue.atoms() {
        if atom.name() != Some(name) || atom.element() != Some(element) {
            continue;
        }
        if selection.contains(atom.index().get()) {
            return Some(atom);
        }
        if preferred.is_some() && atom.alt_id() == preferred {
            conformer = Some(atom);
        }
        if first.is_none() {
            first = Some(atom);
        }
    }
    conformer.or(first)
}

pub(super) fn plane_cue<'a>(
    residue: ResidueRef<'a>,
    guide: AtomRef<'a>,
    kind: GuideKind,
) -> Option<AtomRef<'a>> {
    let name = match kind {
        GuideKind::AlphaCarbon => "O",
        GuideKind::SugarCarbon => "O4'",
        GuideKind::Phosphate | GuideKind::RingCenter => return None,
    };
    let mut compatible = None;
    for atom in residue.atoms() {
        if atom.name() != Some(name) || atom.element() != Some(Element::OXYGEN) {
            continue;
        }
        if atom.alt_id() == guide.alt_id() {
            return Some(atom);
        }
        let left = match atom.alt_id() {
            Some(value) => value,
            None => molframe::AltId::BLANK,
        };
        let right = match guide.alt_id() {
            Some(value) => value,
            None => molframe::AltId::BLANK,
        };
        if compatible.is_none() && molframe::altloc_compatible(left, right) {
            compatible = Some(atom);
        }
    }
    compatible
}
