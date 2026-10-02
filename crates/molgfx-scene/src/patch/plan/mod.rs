//! Prepared atomic updates for the live semantic and physical scene.
//!
//! A patch is prepared in full — every query evaluated, every value validated,
//! every physical record computed — against the unchanged scene, and only then
//! committed. Operations that change retained scene domains or which
//! representations/overlay items exist re-resolve the scene; everything
//! else, including recolouring, retargeting a representation and editing
//! appearance rules, is applied in place to the representations and columns it
//! touches.

mod appearance;
mod commit;
mod interactions;
mod lower;
mod overlay;
mod routing;
mod targets;

pub(crate) use commit::CommitTarget;

mod change;
mod inputs;
mod local_plan;
mod overlay_domains;
mod patch_plan;

use change::{Change, assign};
pub(crate) use inputs::PatchInputs;
pub(crate) use local_plan::LocalPatchPlan;
use overlay_domains::OverlayDomains;
pub(crate) use patch_plan::PatchPlan;
