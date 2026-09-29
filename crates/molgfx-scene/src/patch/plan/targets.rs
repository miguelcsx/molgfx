//! Representations whose molecular target a patch changes.
//!
//! A retarget changes which atoms a representation draws, so its geometry is
//! rebuilt — but only its own. The new query is evaluated once, against the
//! representation's own structure, and shares rows with any other
//! representation that already draws the same query.

use super::PatchInputs;
use crate::error::Error;
use crate::id::{RepresentationId, StructureId};
use crate::representation::form::RepresentationSpec;
use molgfx_core::{AtomSelection, StructureHandle};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// One retarget, evaluated and waiting to be installed.
pub(super) struct PreparedTarget {
    pub(super) structure: StructureHandle,
    pub(super) rows: Arc<AtomSelection>,
    pub(super) fingerprint: u64,
    /// The key the scene shares identical selections under.
    pub(super) key: String,
}

#[derive(Default)]
pub(super) struct TargetUpdates {
    ids: BTreeSet<RepresentationId>,
    pub(super) prepared: BTreeMap<RepresentationId, PreparedTarget>,
}

impl TargetUpdates {
    pub(super) fn mark(&mut self, id: RepresentationId) {
        let _ = self.ids.insert(id);
    }

    /// Evaluates the final target of every retargeted representation.
    pub(super) fn prepare(
        &mut self,
        representations: &BTreeMap<RepresentationId, RepresentationSpec>,
        inputs: PatchInputs<'_>,
        handles: &BTreeMap<StructureId, StructureHandle>,
    ) -> Result<(), Error> {
        for id in &self.ids {
            let Some(representation) = representations.get(id) else {
                return Err(Error::MissingId);
            };
            let Some(structure) = representation.common.structure else {
                return Err(Error::InvalidSpec(
                    "representation has no structure target".to_owned(),
                ));
            };
            let (Some(source), Some(core)) =
                (inputs.structures.get(&structure), handles.get(&structure))
            else {
                return Err(Error::InvalidSpec(
                    "representation structure is not bound".to_owned(),
                ));
            };
            let target = &representation.common.target;
            let (rows, fingerprint) = inputs.rows.rows(structure, source, target)?;
            let key = format!("{}:{}", structure.get(), target.stable_hash()?);
            let _ = self.prepared.insert(
                *id,
                PreparedTarget {
                    structure: *core,
                    rows,
                    fingerprint,
                    key,
                },
            );
        }
        Ok(())
    }
}
