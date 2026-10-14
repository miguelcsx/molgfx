//! Capture and restoration: the portable semantic snapshot of a scene and the
//! runtime archive that carries its scientific assets.

mod archive;
mod value;

pub use archive::{RuntimeCapture, SceneArchive};
pub use value::SceneSnapshot;
