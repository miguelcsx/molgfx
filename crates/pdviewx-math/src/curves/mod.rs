//! Curve sampling and twist-free frame transport.

pub(crate) mod bezier;
pub(crate) mod spline;
pub(crate) mod transport;

pub use bezier::sample_cubic_bezier;
pub use spline::{
    CurveSample, sample_catmull_rom, sample_catmull_rom_demanding, sample_catmull_rom_fixed,
};
pub use transport::{TransportFrame, parallel_transport};
