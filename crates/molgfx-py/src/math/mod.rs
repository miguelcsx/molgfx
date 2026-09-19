//! Python value adapters for the public math and camera surface.
//!
//! The identities a scene is built from — vectors, a quaternion, matrices, a
//! bounding volume, the camera — are read and written field by field, while the
//! sampling functions shape a whole trace and hand back finished points. They
//! are two different kinds of call, so they live apart.

mod identity;
mod sampling;

pub(crate) use identity::{
    PyAabb, PyBoundingSphere, PyCamera, PyMat4, PyProjection, PyQuat, PyRgba8, PyVec3,
};
pub(crate) use sampling::{
    PyCurveSample, PyMat3, PyTransportFrame, PyVec2, PyVec4, parallel_transport, round_u8,
    sample_catmull_rom, sample_catmull_rom_demanding, sample_catmull_rom_fixed,
    sample_cubic_bezier, truncate_u16, unit_to_grid, unorm8,
};
