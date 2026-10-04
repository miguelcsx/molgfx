//! Indexed boundaries from immutable scalar grids and exact categorical membership.

mod extract;
mod grid;
mod polygonise;
mod types;

pub use extract::{extract_isosurface, extract_label_surfaces};
pub use types::{BoundaryMesh, BoundaryVertex, FieldError};

#[cfg(test)]
mod tests;
