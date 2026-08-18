//! Abstract input events.
//!
//! The engine never names a windowing library; embedders translate their
//! toolkit's events into these and feed them to a camera controller.

/// A pointer button.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Button {
    /// Primary button.
    Left,
    /// Secondary button.
    Right,
    /// Middle button or wheel click.
    Middle,
}

/// A key the controllers care about; navigation only, not text input.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    /// Move forward (fly).
    Forward,
    /// Move backward (fly).
    Backward,
    /// Strafe left (fly).
    Left,
    /// Strafe right (fly).
    Right,
    /// Rise (fly).
    Up,
    /// Descend (fly).
    Down,
}

/// One abstract input event, in window coordinates where applicable.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum InputEvent {
    /// The pointer moved to `(x, y)`, in pixels from the top-left.
    PointerMove {
        /// Horizontal position, pixels.
        x: f32,
        /// Vertical position, pixels.
        y: f32,
    },
    /// A pointer button changed state at `(x, y)`.
    PointerButton {
        /// Which button.
        button: Button,
        /// True on press, false on release.
        pressed: bool,
        /// Horizontal position, pixels.
        x: f32,
        /// Vertical position, pixels.
        y: f32,
    },
    /// Scroll wheel or trackpad scroll; positive is toward the scene.
    Scroll {
        /// Scroll amount in notches or normalized trackpad units.
        delta: f32,
    },
    /// A navigation key changed state.
    Key {
        /// Which key.
        key: Key,
        /// True on press, false on release.
        pressed: bool,
    },
    /// Trackpad pinch; scale > 1 zooms in.
    Pinch {
        /// Relative scale change since the last event.
        scale: f32,
    },
}
