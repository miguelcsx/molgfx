use super::*;
use crate::{RenderProfile, ToneMapping};
use molgfx_core::Scene;
use molgfx_math::{Camera, Projection, Vec3};

fn camera() -> Camera {
    Camera {
        eye: Vec3::new(0.0, 0.0, 8.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        projection: Projection::Perspective {
            fov_y: 0.8,
            aspect: 1.0,
            near: 0.1,
            far: 100.0,
        },
    }
}

#[test]
fn session_json_round_trips_camera_and_ordered_profile() {
    let mut profile = RenderProfile::cinematic();
    profile = profile.with_effect(crate::PresentationEffect::Display(
        crate::DisplayTransform {
            tone_mapping: ToneMapping::Reinhard,
            ..crate::DisplayTransform::default()
        },
    ));
    let session = RenderSession::new(&Scene::new(), camera(), profile);
    let json = match session.to_json() {
        Ok(json) => json,
        Err(error) => panic!("session serializes: {error}"),
    };
    let decoded = match RenderSession::from_json(&json) {
        Ok(session) => session,
        Err(error) => panic!("session deserializes: {error}"),
    };
    assert_eq!(decoded, session);
}

#[test]
fn session_rejects_a_different_schema() {
    let mut session = RenderSession::new(&Scene::new(), camera(), RenderProfile::inspection());
    session.schema += 1;
    let json = match serde_json::to_string(&session) {
        Ok(json) => json,
        Err(error) => panic!("test session serializes: {error}"),
    };
    assert!(RenderSession::from_json(&json).is_err());
}
