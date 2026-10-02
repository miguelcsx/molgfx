//! The two ways a patch is prepared: re-resolve, or apply in place.

use super::{LocalPatchPlan, PatchInputs, routing};
use crate::error::Error;
use crate::scene::Resolution;
use crate::scene::apply::candidate_spec;
use crate::spec::{ScenePatch, SceneSpec};
use routing::is_structural;

pub(crate) enum PatchPlan {
    Structural {
        spec: Box<SceneSpec>,
        resolution: Box<Resolution>,
    },
    Local(Box<LocalPatchPlan>),
}

impl PatchPlan {
    pub(crate) fn prepare(inputs: PatchInputs<'_>, patch: &ScenePatch) -> Result<Self, Error> {
        if patch.operations.iter().any(is_structural) {
            let candidate = candidate_spec(inputs.spec, patch)?;
            // A structural patch in this planner only ever adds, removes or
            // replaces representations and overlay items; the molecules are
            // untouched, so their atom tables are reused rather than rebuilt,
            // and unchanged queries come from the evaluated-rows cache.
            let resolution = crate::scene::runtime::resolve_reusing(
                &candidate,
                inputs.structures,
                inputs.property_bindings,
                inputs.overlay_bindings,
                inputs.rows,
                Some(inputs.structure_assets),
            )
            .map_err(|error| Error::InvalidSpec(format!("patch could not be resolved: {error}")))?;
            return Ok(Self::Structural {
                spec: Box::new(candidate),
                resolution: Box::new(resolution),
            });
        }
        Ok(Self::Local(Box::new(LocalPatchPlan::prepare(
            inputs, patch,
        )?)))
    }
}
