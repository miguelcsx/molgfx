//! Appearance rules resolved against the physical scene.
//!
//! Each structure with rules owns one class column, a scalar atom property of
//! the core scene, and every representation of that structure reads it through
//! the same colour overlay. Resolution is split in two so a patch can do every
//! fallible, expensive step before it mutates anything: [`prepare`] evaluates
//! rules into class values without touching the scene, and [`install`] writes
//! the prepared columns and returns the overlays to attach.

use crate::appearance::{AppearanceClasses, resolve_classes};
use crate::error::Error;
use crate::id::{AppearanceRuleId, StructureId};
use crate::scene::selection_rows::SelectionRows;
use crate::{AppearanceRuleSpec, SceneSpec};
use molgfx_core::{AtomPropertyHandle, ColorOverlay, MolecularSource, StructureHandle};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// Each structure's class column, for structures that have rules.
pub(crate) type AppearanceColumns = BTreeMap<StructureId, AtomPropertyHandle>;

/// One structure's classes, evaluated and validated, waiting to be installed.
pub(crate) struct PreparedClasses {
    structure: StructureId,
    column: Option<(molgfx_core::AtomProperty, Vec<molgfx_core::ColorScheme>)>,
}

/// Evaluates the rules of `touched` structures from `rules`.
///
/// # Errors
///
/// Returns an error when a structure is unbound, a rule's query fails, or a
/// structure's rules use more distinct colourings than one overlay holds.
pub(crate) fn prepare(
    rules: &BTreeMap<AppearanceRuleId, AppearanceRuleSpec>,
    touched: &BTreeSet<StructureId>,
    structures: &BTreeMap<StructureId, MolecularSource>,
    handles: &BTreeMap<StructureId, StructureHandle>,
    rows: &SelectionRows,
) -> Result<Vec<PreparedClasses>, Error> {
    let mut prepared = Vec::with_capacity(touched.len());
    for structure in touched {
        let (Some(source), Some(core)) = (structures.get(structure), handles.get(structure)) else {
            return Err(Error::InvalidSpec(
                "appearance rule targets an unbound structure".to_owned(),
            ));
        };
        let atom_count = u32::try_from(source.coordinates().len())
            .map_err(|_| Error::InvalidSpec("structure is too large to colour".to_owned()))?;
        let classes = resolve_classes(
            rules.values().filter(|rule| rule.structure == *structure),
            atom_count,
            |rule| {
                rows.rows(*structure, source, &rule.target)
                    .map(|(rows, _)| rows)
            },
        )?;
        let column = classes
            .map(|AppearanceClasses { values, schemes }| {
                molgfx_core::AtomProperty::new(
                    *core,
                    Arc::from(format!("molgfx.appearance.{}", structure.get())),
                    values,
                    molgfx_core::AtomPropertyMeaning::Generic,
                    molgfx_core::ScalarFieldSemantics::UncalibratedRank,
                )
                .map(|property| (property, schemes))
            })
            .transpose()?;
        prepared.push(PreparedClasses {
            structure: *structure,
            column,
        });
    }
    Ok(prepared)
}

/// Every structure that has at least one rule.
pub(crate) fn ruled_structures(spec: &SceneSpec) -> BTreeSet<StructureId> {
    spec.appearance
        .values()
        .map(|rule| rule.structure)
        .collect()
}

/// Overlays to attach per structure (`None` detaches), and class columns that
/// nothing reads any more and can be released once representations stop
/// naming them.
pub(crate) struct Installed {
    pub(crate) overlays: BTreeMap<StructureId, Option<ColorOverlay>>,
    pub(crate) retired: Vec<AtomPropertyHandle>,
}

/// Writes prepared class columns into `scene`, reusing each structure's
/// existing column handle so representations keep their binding.
///
/// Every column was validated when it was prepared, so on a scene the plan was
/// prepared against, installation does not fail.
///
/// # Errors
///
/// Returns an error when a column no longer matches its structure.
pub(crate) fn install(
    scene: &mut molgfx_core::Scene,
    columns: &mut AppearanceColumns,
    prepared: Vec<PreparedClasses>,
) -> Result<Installed, Error> {
    let mut overlays = BTreeMap::new();
    let mut retired = Vec::new();
    for PreparedClasses { structure, column } in prepared {
        let Some((property, schemes)) = column else {
            if let Some(handle) = columns.remove(&structure) {
                retired.push(handle);
            }
            let _ = overlays.insert(structure, None);
            continue;
        };
        let handle = if let Some(handle) = columns.get(&structure).copied() {
            scene.replace_atom_property(handle, property)?;
            handle
        } else {
            let handle = scene.add_atom_property(property)?;
            let _ = columns.insert(structure, handle);
            handle
        };
        let overlay = ColorOverlay::new(handle, &schemes)?;
        let _ = overlays.insert(structure, Some(overlay));
    }
    Ok(Installed { overlays, retired })
}
