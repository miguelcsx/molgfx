//! Copies the opaque depth for passes that sample it while testing against it.
//!
//! Where a depth texture may be a read-only attachment and a bound texture at
//! once, transparent passes bind the live depth. Where it may not, the copy
//! stands in for the binding, and the live depth stays the attachment.

use crate::graph::PassContext;
use crate::passes::{DEPTH_RESOURCE, OPAQUE_DEPTH_RESOURCE};
use molgfx_gpu::{CommandEncoder as _, Device};

/// Load-time state of the snapshot pass.
#[derive(Debug)]
pub(crate) struct OpaqueDepthPass;

impl OpaqueDepthPass {
    /// Records the whole-texture copy.
    pub(crate) fn record<D: Device>(ctx: &mut PassContext<'_, D>) {
        let (Some(live), Some(copy)) = (
            ctx.resources.texture(DEPTH_RESOURCE),
            ctx.resources.texture(OPAQUE_DEPTH_RESOURCE),
        ) else {
            return;
        };
        ctx.encoder
            .copy_texture_to_texture(live, copy, ctx.resources.extent());
    }
}
