//! Stable table operations for screen overlays.

use crate::{OverlayHandle, Scene, ScreenOverlay};

impl Scene {
    /// Adds one depth-independent screen overlay.
    pub fn add_overlay(&mut self, overlay: ScreenOverlay) -> OverlayHandle {
        self.overlay_revision = self.overlay_revision.wrapping_add(1);
        OverlayHandle(self.overlays.insert(overlay))
    }

    /// Resolves an overlay handle.
    #[must_use]
    pub fn overlay(&self, handle: OverlayHandle) -> Option<&ScreenOverlay> {
        self.overlays.get(handle.0)
    }

    /// Mutably resolves an overlay and invalidates resident overlay geometry.
    pub fn overlay_mut(&mut self, handle: OverlayHandle) -> Option<&mut ScreenOverlay> {
        let overlay = self.overlays.get_mut(handle.0)?;
        self.overlay_revision = self.overlay_revision.wrapping_add(1);
        Some(overlay)
    }

    /// Removes one overlay.
    pub fn remove_overlay(&mut self, handle: OverlayHandle) -> Option<ScreenOverlay> {
        let removed = self.overlays.remove(handle.0);
        if removed.is_some() {
            self.overlay_revision = self.overlay_revision.wrapping_add(1);
        }
        removed
    }

    /// Iterates overlays in stable slot order.
    pub fn overlays(&self) -> impl Iterator<Item = (OverlayHandle, &ScreenOverlay)> + '_ {
        self.overlays
            .iter()
            .map(|(raw, value)| (OverlayHandle(raw), value))
    }

    /// Revision used by persistent GPU overlay storage.
    #[must_use]
    pub const fn overlay_revision(&self) -> u64 {
        self.overlay_revision
    }
}
