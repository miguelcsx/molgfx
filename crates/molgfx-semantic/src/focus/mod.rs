//! Focus-and-context policies and their surface extent helpers.

mod composition;
pub(crate) mod surface_zone;

pub use composition::{
    DistanceBands, FocusBand, FocusContext, FocusError, FocusScene, FocusStyle, FocusSurfaceExtent,
    FocusView,
};
pub use surface_zone::{SurfaceZone, SurfaceZoneScene, SurfaceZoneStyle};
