use super::tests::{camera, engine};
use molgfx_core::Scene;
use molgfx_gpu::{DeviceLoss, DeviceLossReason};

#[test]
fn a_healthy_engine_reports_no_loss() {
    assert_eq!(engine().device_loss(), None);
}

#[test]
fn a_lost_device_surfaces_its_reason_and_recoverability() {
    let engine = engine();
    engine.device.lose(DeviceLoss {
        reason: DeviceLossReason::Unknown,
        message: "adapter reset".to_owned(),
    });
    let Some(loss) = engine.device_loss() else {
        panic!("the loss is reported")
    };
    assert_eq!(loss.reason, DeviceLossReason::Unknown);
    assert!(loss.recoverable());
}

#[test]
fn a_replacement_engine_renders_the_scene_the_lost_one_held() {
    let scene = Scene::new();
    let lost = engine();
    lost.device.lose(DeviceLoss {
        reason: DeviceLossReason::Unknown,
        message: String::new(),
    });
    assert!(lost.device_loss().is_some());
    let mut replacement = engine();
    assert_eq!(replacement.device_loss(), None);
    assert!(replacement.render(&scene, &camera()).is_ok());
}
