//! Pixel picking and its detached readback.
//!
//! Picking resolves one canvas pixel through the renderer's packed integer map
//! and the live semantic scene. The browser path records a pick, detaches the
//! readback, and resolves it once the bytes arrive, so a frame can render while
//! the readback is in flight — on the browser's single JavaScript thread that
//! is what keeps rendering and picking from aliasing.

use super::Renderer;
use crate::{Error, PickKind, PickResult, ResolvedPick, Scene};

/// A detached pick readback over one renderer frame.
///
/// It borrows neither the renderer nor its device, so a caller can keep
/// rendering — and, on the browser's single JavaScript thread, resolving
/// other work — while the readback is awaited. Resolve it, then hand the
/// bytes to [`Renderer::finish_pick`].
#[derive(Debug)]
pub struct PickReadback(molgfx_wgpu::WgpuReadback);

impl PickReadback {
    /// Awaits the mapped identity bytes.
    ///
    /// # Errors
    ///
    /// Returns a typed readback or device error.
    pub async fn resolve(&self) -> Result<Vec<u8>, Error> {
        molgfx_gpu::Readback::resolve(&self.0, 0, molgfx_render::PICK_READBACK_BYTES)
            .await
            .map_err(|error| Error::Render(error.into()))
    }
}

impl Renderer {
    /// Asynchronously resolves the entity under one target pixel.
    ///
    /// # Errors
    ///
    /// Returns a typed readback or device error.
    pub async fn pick_async(&mut self, x: u32, y: u32) -> Result<Option<PickResult>, Error> {
        self.inner
            .pick_async(x, y)
            .await
            .map(|pick| pick.as_ref().map(semantic_pick))
            .map_err(Error::from)
    }

    /// Records one pick and detaches its readback.
    ///
    /// The returned handle borrows nothing, so a caller may keep rendering
    /// while the readback is awaited; this is the portable browser path.
    /// `None` means the pixel is outside the target or nothing is drawable.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the pick copies cannot be recorded.
    pub fn begin_pick(&mut self, x: u32, y: u32) -> Result<Option<PickReadback>, Error> {
        let readback = self.inner.begin_pick(x, y).map_err(Error::from)?;
        Ok(readback.map(PickReadback))
    }

    /// Resolves a [`Self::begin_pick`] readback against the current scene.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the packed identity cannot be resolved.
    pub fn finish_pick(&self, scene: &Scene, packed: &[u8]) -> Result<Option<ResolvedPick>, Error> {
        let Some(raw) = self.inner.finish_pick(packed).map_err(Error::from)? else {
            return Ok(None);
        };
        scene.resolve_pick(&semantic_pick(&raw)).map(Some)
    }

    /// Resolves the entity under one target pixel on native platforms.
    ///
    /// # Errors
    ///
    /// Returns a typed readback or device error.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn pick(&mut self, x: u32, y: u32) -> Result<Option<PickResult>, Error> {
        self.inner
            .pick(x, y)
            .map(|pick| pick.as_ref().map(semantic_pick))
            .map_err(Error::from)
    }
}

pub(super) fn semantic_pick(pick: &molgfx_render::Pick) -> PickResult {
    match pick.entity {
        molgfx_render::PickEntity::Structure(identity) => PickResult {
            kind: pick_kind(identity.kind()),
            dataset: Some(identity.dataset().get()),
            chunk: Some(identity.chunk().get()),
            row: Some(identity.row().get()),
            volume_label: None,
        },
        molgfx_render::PickEntity::VolumeSegment(segment) => PickResult {
            kind: PickKind::VolumeSegment,
            dataset: None,
            chunk: None,
            row: None,
            volume_label: Some(segment.label),
        },
    }
}

const fn pick_kind(kind: molgfx_core::EntityKind) -> PickKind {
    match kind {
        molgfx_core::EntityKind::Atom => PickKind::Atom,
        molgfx_core::EntityKind::Bond => PickKind::Bond,
        molgfx_core::EntityKind::Edge => PickKind::Interaction,
        molgfx_core::EntityKind::Label => PickKind::Label,
        molgfx_core::EntityKind::Measurement => PickKind::Measurement,
        molgfx_core::EntityKind::Primitive => PickKind::Primitive,
        molgfx_core::EntityKind::Mesh => PickKind::Mesh,
        molgfx_core::EntityKind::LigandPoseBatch => PickKind::LigandPoseBatch,
        molgfx_core::EntityKind::Guide => PickKind::Guide,
        molgfx_core::EntityKind::DynamicBond => PickKind::DynamicBond,
        molgfx_core::EntityKind::Point => PickKind::Point,
        molgfx_core::EntityKind::Instance => PickKind::Instance,
        molgfx_core::EntityKind::TemplatePart => PickKind::TemplatePart,
        molgfx_core::EntityKind::Relation => PickKind::Relation,
    }
}
