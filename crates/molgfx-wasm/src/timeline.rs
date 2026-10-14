//! The multi-track playback clock in the browser.
//!
//! One `Timeline` maps a single playback clock, in seconds, onto each track's
//! own physical interval. The page drives it with `apply` and reads each
//! track's typed time back with `readingsInto` (reused buffer, no allocation)
//! or `readings` (convenience objects). Physical times cross as `f64` seconds;
//! the unit is a display hint from a fixed set of symbols.

use crate::contract::{WebScene, javascript_error};
use crate::timeline_wire::{Mode, READING_STRIDE, WireError, pack_rows, parse_mode, parse_range};
use molgfx::time::{SimTime, TimeUnit};
use molgfx::timeline::{PlaybackMode, TimeWarp, Timeline, TrackReading};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = TrackReading)]
#[derive(Clone, Copy, Debug)]
/// One track's typed reading at the most recently applied playback time.
pub struct WebTrackReading {
    track: u32,
    inner: TrackReading,
}

#[wasm_bindgen(js_class = TrackReading)]
impl WebTrackReading {
    /// Stable identity of the track, as returned when it was added.
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn track(&self) -> u32 {
        self.track
    }

    /// Physical time in canonical seconds, after the track's boundary policy.
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn seconds(&self) -> f64 {
        self.inner.time.seconds()
    }

    /// Display unit symbol: one of `as`, `fs`, `ps`, `ns`, `us`, `ms`, `s`.
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn unit(&self) -> String {
        self.inner.unit.symbol().to_owned()
    }

    /// Physical time expressed in the display unit.
    #[must_use]
    #[wasm_bindgen(getter, js_name = displayValue)]
    pub fn display_value(&self) -> f64 {
        self.inner.display_value()
    }

    /// Fraction of the track's interval already played, in `[0, 1]`.
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn phase(&self) -> f64 {
        self.inner.phase
    }
}

#[wasm_bindgen(js_name = Timeline)]
#[derive(Debug, Default)]
/// Independent trajectory tracks driven by one playback clock.
pub struct WebTimeline {
    inner: Timeline,
}

fn sim(seconds: f64) -> Result<SimTime, JsError> {
    SimTime::new(seconds).map_err(javascript_error)
}

fn playback_mode(name: &str) -> Result<PlaybackMode, JsError> {
    Ok(match parse_mode(name).map_err(javascript_error)? {
        Mode::Clamp => PlaybackMode::Clamp,
        Mode::Loop => PlaybackMode::Loop,
        Mode::PingPong => PlaybackMode::PingPong,
    })
}

/// Position of a unit in `TimeUnit::ALL`, the code a packed reading carries.
const fn unit_code(unit: TimeUnit) -> f64 {
    match unit {
        TimeUnit::Attosecond => 0.0,
        TimeUnit::Femtosecond => 1.0,
        TimeUnit::Picosecond => 2.0,
        TimeUnit::Nanosecond => 3.0,
        TimeUnit::Microsecond => 4.0,
        TimeUnit::Millisecond => 5.0,
        TimeUnit::Second => 6.0,
    }
}

fn sim_range(values: &[f64]) -> Result<[SimTime; 2], JsError> {
    let [start, end] = parse_range(values).map_err(javascript_error)?;
    Ok([sim(start)?, sim(end)?])
}

fn with_unit(warp: TimeWarp, unit: Option<String>) -> Result<TimeWarp, JsError> {
    match unit {
        Some(symbol) => Ok(warp.with_unit(
            TimeUnit::from_symbol(&symbol)
                .ok_or_else(|| javascript_error(WireError::Unit(symbol.clone())))?,
        )),
        None => Ok(warp),
    }
}

#[wasm_bindgen(js_class = Timeline)]
impl WebTimeline {
    /// Creates an empty timeline at zero playback seconds.
    #[must_use]
    #[wasm_bindgen(constructor)]
    pub fn new() -> WebTimeline {
        Self::default()
    }

    /// Adds a track that advances `simSecondsPerPlaybackSecond` of physical time
    /// each playback second over `rangeSeconds` (`[start, end]`).
    ///
    /// `mode` is `clamp`, `loop` or `pingpong`; `unit` optionally overrides the
    /// display unit (`as` through `s`). Returns the track identity.
    ///
    /// # Errors
    /// Returns an error for an unbound structure, an interval outside the
    /// structure's resident frame pair, a second track on the same structure, or
    /// a malformed mode, unit or time.
    #[wasm_bindgen(js_name = addDilatedTrack)]
    pub fn add_dilated_track(
        &mut self,
        scene: &WebScene,
        structure: u64,
        range_seconds: &[f64],
        sim_seconds_per_playback_second: f64,
        mode: &str,
        unit: Option<String>,
    ) -> Result<u32, JsError> {
        let warp = TimeWarp::dilation(
            sim(sim_seconds_per_playback_second)?,
            sim_range(range_seconds)?,
            playback_mode(mode)?,
        )
        .map_err(javascript_error)?;
        self.bind(scene, structure, with_unit(warp, unit)?)
    }

    /// Adds a track that plays `rangeSeconds` (`[start, end]`) once across
    /// `playbackSeconds` of the playback clock.
    ///
    /// # Errors
    /// Returns the same errors as `addDilatedTrack`, plus one for a playback
    /// duration that is not finite and positive.
    #[wasm_bindgen(js_name = addFittedTrack)]
    pub fn add_fitted_track(
        &mut self,
        scene: &WebScene,
        structure: u64,
        range_seconds: &[f64],
        playback_seconds: f64,
        mode: &str,
        unit: Option<String>,
    ) -> Result<u32, JsError> {
        let warp = TimeWarp::fit(
            sim_range(range_seconds)?,
            playback_seconds,
            playback_mode(mode)?,
        )
        .map_err(javascript_error)?;
        self.bind(scene, structure, with_unit(warp, unit)?)
    }

    fn bind(&mut self, scene: &WebScene, structure: u64, warp: TimeWarp) -> Result<u32, JsError> {
        let resolved = scene
            .resolved
            .as_ref()
            .ok_or_else(|| JsError::new("resolve the scene before adding a timeline track"))?;
        resolved
            .bind_timeline_trajectory(&mut self.inner, molgfx::StructureId::new(structure), warp)
            .map(molgfx::timeline::TimelineTrackHandle::row)
            .map_err(javascript_error)
    }

    /// Applies one playback time to every track of `scene`, atomically.
    ///
    /// Allocates nothing. On error the scene is unchanged.
    ///
    /// # Errors
    /// Returns an error for an unresolved scene, a non-finite time, or a track
    /// whose resident frame pair no longer matches.
    pub fn apply(&mut self, scene: &mut WebScene, playback_seconds: f64) -> Result<(), JsError> {
        scene
            .resolved
            .as_mut()
            .ok_or_else(|| JsError::new("resolve the scene before applying a timeline"))?
            .apply_timeline(&mut self.inner, playback_seconds)
            .map_err(javascript_error)
    }

    /// Most recently applied playback time, in seconds.
    #[must_use]
    #[wasm_bindgen(getter, js_name = playbackSeconds)]
    pub fn playback_seconds(&self) -> f64 {
        self.inner.time_seconds()
    }

    /// Number of tracks.
    #[must_use]
    #[wasm_bindgen(getter, js_name = trackCount)]
    pub fn track_count(&self) -> usize {
        self.inner.len()
    }

    /// Every track's reading as objects; allocates, so prefer `readingsInto`
    /// inside a frame loop.
    #[must_use]
    pub fn readings(&self) -> Vec<WebTrackReading> {
        self.inner
            .readings()
            .map(|(handle, inner)| WebTrackReading {
                track: handle.row(),
                inner,
            })
            .collect()
    }

    /// Writes `[track, seconds, phase, unitCode]` per track into `out` and
    /// returns the number of tracks written.
    ///
    /// `unitCode` indexes `Timeline.unitSymbols()`. The buffer is reused by the
    /// caller, so a frame loop allocates nothing here; `out` needs
    /// `trackCount * Timeline.readingStride()` values.
    ///
    /// # Errors
    /// Returns an error, leaving `out` untouched, when it is too small.
    #[wasm_bindgen(js_name = readingsInto)]
    pub fn readings_into(&self, out: &mut [f64]) -> Result<usize, JsError> {
        pack_rows(
            || {
                self.inner.readings().map(|(handle, reading)| {
                    [
                        f64::from(handle.row()),
                        reading.time.seconds(),
                        reading.phase,
                        unit_code(reading.unit),
                    ]
                })
            },
            out,
        )
        .map_err(javascript_error)
    }

    /// `f64` slots one track occupies in `readingsInto`.
    #[must_use]
    #[wasm_bindgen(js_name = readingStride)]
    pub fn reading_stride() -> usize {
        READING_STRIDE
    }

    /// Unit symbols in `unitCode` order, shortest to longest.
    #[must_use]
    #[wasm_bindgen(js_name = unitSymbols)]
    pub fn unit_symbols() -> Vec<String> {
        TimeUnit::ALL
            .iter()
            .map(|unit| unit.symbol().to_owned())
            .collect()
    }
}
