//! How overlay specifications are added to a scene.

use super::{
    EllipsoidSpec, InteractionSpec, Label, MeasurementSpec, PlaneSpec, TrajectorySpec, Volume,
};
use crate::id::{
    AnnotationId, EllipsoidId, InteractionId, MeasurementId, PlaneId, TrajectoryId, VolumeId,
};
use crate::representation::private::Sealed;
use crate::{Error, Scene, SceneItem};

macro_rules! tuple_scene_item {
    ($item:ty, $id:ty, $method:ident) => {
        impl Sealed for $item {}

        impl SceneItem for $item {
            type Id = $id;

            fn add_to(self, scene: &mut Scene) -> Result<Self::Id, Error> {
                scene.$method(self.0)
            }
        }
    };
}

tuple_scene_item!(Volume, VolumeId, insert_volume);
tuple_scene_item!(Label, AnnotationId, insert_annotation);

impl Sealed for MeasurementSpec {}

impl SceneItem for MeasurementSpec {
    type Id = MeasurementId;

    fn add_to(self, scene: &mut Scene) -> Result<Self::Id, Error> {
        scene.insert_measurement(self)
    }
}

impl Sealed for InteractionSpec {}

impl SceneItem for InteractionSpec {
    type Id = InteractionId;

    fn add_to(self, scene: &mut Scene) -> Result<Self::Id, Error> {
        scene.insert_interaction(self)
    }
}

impl Sealed for EllipsoidSpec {}

impl SceneItem for EllipsoidSpec {
    type Id = EllipsoidId;

    fn add_to(self, scene: &mut Scene) -> Result<Self::Id, Error> {
        scene.insert_ellipsoids(self)
    }
}

impl Sealed for PlaneSpec {}

impl SceneItem for PlaneSpec {
    type Id = PlaneId;

    fn add_to(self, scene: &mut Scene) -> Result<Self::Id, Error> {
        scene.insert_plane(self)
    }
}

impl Sealed for TrajectorySpec {}

impl SceneItem for TrajectorySpec {
    type Id = TrajectoryId;

    fn add_to(self, scene: &mut Scene) -> Result<Self::Id, Error> {
        scene.insert_trajectory(self)
    }
}
