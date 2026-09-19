//! Browser timeline value types and generational handles.

use crate::generic_batches::core_error;
use wasm_bindgen::prelude::*;

/// Playback behavior outside a timeline's local range.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug)]
pub enum WebPlaybackMode {
    /// Clamp to the first or last sample.
    Clamp,
    /// Wrap continuously to the beginning.
    Loop,
    /// Alternate forward and backward playback.
    PingPong,
}

impl From<WebPlaybackMode> for molgfx::core::PlaybackMode {
    fn from(value: WebPlaybackMode) -> Self {
        match value {
            WebPlaybackMode::Clamp => Self::Clamp,
            WebPlaybackMode::Loop => Self::Loop,
            WebPlaybackMode::PingPong => Self::PingPong,
        }
    }
}

/// Deterministic mapping from presentation time to local sample time.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug)]
pub struct WebTimeWarp {
    pub(super) inner: molgfx::core::TimeWarp,
}

#[wasm_bindgen]
impl WebTimeWarp {
    /// Creates a validated time mapping.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error for a malformed range, rate or origin.
    #[wasm_bindgen(constructor)]
    pub fn new(
        global_origin: f64,
        local_origin: f64,
        rate: f64,
        range_start: f64,
        range_end: f64,
        playback: WebPlaybackMode,
    ) -> Result<WebTimeWarp, JsValue> {
        molgfx::core::TimeWarp::new(
            global_origin,
            local_origin,
            rate,
            [range_start, range_end],
            playback.into(),
        )
        .map(|inner| Self { inner })
        .map_err(core_error)
    }

    /// Samples local seconds without mutating scene state.
    #[must_use]
    pub fn sample(&self, global_seconds: f64) -> Option<f64> {
        self.inner.sample(global_seconds)
    }
}

/// Generational handle for one browser timeline track.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug)]
pub struct WebTimelineTrackHandle {
    pub(super) inner: molgfx::core::TimelineTrackHandle,
}

#[wasm_bindgen]
impl WebTimelineTrackHandle {
    /// Stable slot row within the current scene generation.
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn row(&self) -> u32 {
        self.inner.row()
    }

    /// Generation that prevents stale-handle reuse.
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn generation(&self) -> u32 {
        self.inner.generation()
    }
}
