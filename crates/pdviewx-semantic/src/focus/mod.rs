//! Focus-and-context policies and their surface extent helpers.

#[path = "focus.rs"]
pub(crate) mod policies;
pub(crate) mod surface_zone;

pub use policies::{
    DistanceBands, FocusBand, FocusContext, FocusError, FocusScene, FocusStyle, FocusSurfaceExtent,
    FocusView,
};
pub use surface_zone::{SurfaceZone, SurfaceZoneScene, SurfaceZoneStyle};
