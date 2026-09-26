//! Lowering one structure's appearance rules to a class column and a table.
//!
//! Rules are applied in ascending identity, so where two rules cover an atom
//! the later one wins. Rules with the same colouring share one class, which is
//! why the table bound limits distinct colourings rather than rules.
//!
//! Cost is `O(atoms + Σ selected rows)` per structure: one pass to initialise
//! the column and one write per atom each rule selects. Rule queries are not
//! evaluated here; the caller supplies each rule's rows, which it caches by
//! query fingerprint, so an unchanged rule is never evaluated again.

use super::rule::{AppearanceRuleSpec, MAX_APPEARANCE_CLASSES};
use crate::error::Error;
use molgfx_core::{AtomSelection, ColorScheme};
use std::collections::BTreeMap;
use std::sync::Arc;

/// One structure's resolved rules: a class per atom and the scheme per class.
#[derive(Clone, Debug)]
pub(crate) struct AppearanceClasses {
    /// Class per atom row; zero keeps the representation's own colour.
    pub(crate) values: Arc<[f32]>,
    /// Scheme of class `k` at index `k - 1`.
    pub(crate) schemes: Vec<ColorScheme>,
}

/// Resolves `rules`, already in ascending identity order, over `atom_count`
/// rows.
///
/// Returns `None` when there are no rules, so a structure without rules keeps
/// no column at all.
///
/// # Errors
///
/// Returns an invalid-specification error when the rules name more distinct
/// colourings than one table holds, or when a rule's rows cannot be obtained.
pub(crate) fn resolve_classes<'a>(
    rules: impl IntoIterator<Item = &'a AppearanceRuleSpec>,
    atom_count: u32,
    mut rows: impl FnMut(&AppearanceRuleSpec) -> Result<Arc<AtomSelection>, Error>,
) -> Result<Option<AppearanceClasses>, Error> {
    let mut rules = rules.into_iter().peekable();
    if rules.peek().is_none() {
        return Ok(None);
    }
    let length = usize::try_from(atom_count)
        .map_err(|_| Error::InvalidSpec("structure is too large to colour".to_owned()))?;
    let mut values = vec![0.0_f32; length];
    let mut schemes: Vec<ColorScheme> = Vec::new();
    let mut interned: BTreeMap<SchemeKey, u8> = BTreeMap::new();
    for rule in rules {
        let scheme = native_scheme(rule)?;
        let key = SchemeKey::of(scheme);
        let class = if let Some(class) = interned.get(&key) {
            *class
        } else {
            if schemes.len() >= MAX_APPEARANCE_CLASSES {
                return Err(Error::InvalidSpec(format!(
                    "a structure's appearance rules use more than {MAX_APPEARANCE_CLASSES} \
                     distinct colours; remove or merge some colour rules"
                )));
            }
            schemes.push(scheme);
            let Ok(class) = u8::try_from(schemes.len()) else {
                return Err(Error::InvalidSpec(
                    "appearance class table overflowed".to_owned(),
                ));
            };
            let _ = interned.insert(key, class);
            class
        };
        let selected = rows(rule)?;
        let value = f32::from(class);
        selected.for_each(atom_count, |row| {
            if let Some(slot) = usize::try_from(row)
                .ok()
                .and_then(|row| values.get_mut(row))
            {
                *slot = value;
            }
        });
    }
    Ok(Some(AppearanceClasses {
        values: values.into(),
        schemes,
    }))
}

/// The physical scheme one rule's colour lowers to.
fn native_scheme(rule: &AppearanceRuleSpec) -> Result<ColorScheme, Error> {
    rule.validate()?;
    // Property colours are refused by validation, so the property table is
    // never consulted and may be empty.
    rule.color.native(&BTreeMap::new(), rule.structure)
}

/// A total, injective key for interning schemes that have no total order.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SchemeKey {
    Element,
    Chain,
    Residue,
    Secondary,
    Uniform([u8; 4]),
    Other,
}

impl SchemeKey {
    const fn of(scheme: ColorScheme) -> Self {
        match scheme {
            ColorScheme::ByElement => Self::Element,
            ColorScheme::ByChain => Self::Chain,
            ColorScheme::ByResidue => Self::Residue,
            ColorScheme::BySecondaryStructure => Self::Secondary,
            ColorScheme::Uniform(color) => Self::Uniform([color.r, color.g, color.b, color.a]),
            _ => Self::Other,
        }
    }
}
