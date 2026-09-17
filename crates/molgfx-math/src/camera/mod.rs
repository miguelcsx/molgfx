//! Viewpoint and projection construction.

pub(crate) mod projection;
#[path = "camera.rs"]
pub(crate) mod view;

pub use projection::Projection;
pub use view::Camera;
