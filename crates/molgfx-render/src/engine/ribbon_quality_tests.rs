use super::tests::camera;
use super::{AdaptiveQualityConfig, Engine, EngineConfig, QualityTier};
use crate::testing::MockDevice;
use molgfx_core::{AtomSelection, Representation, Scene};
use molgfx_geometry::{RibbonMesh, RibbonParams, RibbonVertex, SplineProfile};
use std::fmt::Write;

#[test]
fn a_large_fixed_quality_ribbon_keeps_every_requested_sample() {
    let mut pdb = String::new();
    for row in 1..=9_000_u16 {
        let x = f32::from(row) * 0.9;
        let y = [0.0, 2.0, -2.0][usize::from(row % 3)];
        writeln!(
            pdb,
            "ATOM  {row:5}  CA  GLY A{row:4}    {x:8.3}{y:8.3}{:8.3}  1.00 10.00           C  ",
            0.0
        )
        .expect("fixture formats");
    }
    let (source, _) = molframe::read_bytes(
        pdb.into_bytes(),
        Some("large-ribbon.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("fixture parses");
    let mut expected = RibbonMesh::default();
    expected
        .generate_structure(
            &source,
            &AtomSelection::All,
            &[],
            molgfx_geometry::CARTOON_GAP_CUTOFF,
            RibbonParams {
                width: 0.6,
                thickness: 0.6,
                profile: SplineProfile::Tube,
                max_steps: 8,
                ..RibbonParams::default()
            },
        )
        .expect("full-quality geometry generates");
    assert!(
        expected.vertices.len() > 400_000,
        "fixture crosses the former silent reduction threshold"
    );
    let mut scene = Scene::from_structure(&source).expect("scene binds");
    let selection = scene.add_selection(AtomSelection::All);
    scene
        .represent(
            selection,
            Representation::tube()
                .tube_radius(0.3)
                .expect("radius validates"),
        )
        .expect("tube attaches");
    let config = EngineConfig {
        adaptive: AdaptiveQualityConfig::fixed(120, QualityTier::High),
        ..EngineConfig::default()
    };
    let mut renderer = Engine::<MockDevice>::new(&config, None).expect("renderer opens");
    renderer
        .render(&scene, &camera())
        .expect("full-quality ribbon renders");
    let buffers = renderer
        .device
        .log
        .buffers
        .lock()
        .expect("buffers recorded");
    let (id, _, _) = buffers
        .iter()
        .find(|(_, label, _)| *label == "cartoon vertices")
        .expect("ribbon vertices uploaded");
    let writes = renderer.device.log.writes.lock().expect("writes recorded");
    let bytes = writes
        .iter()
        .find(|(buffer, _, _, _)| buffer == id)
        .expect("ribbon upload recorded")
        .2;
    assert_eq!(
        bytes,
        expected.vertices.len() * std::mem::size_of::<RibbonVertex>(),
        "the selected quality controls sampling, regardless of scene size"
    );
    drop(writes);
    drop(buffers);
    drop(renderer);
    drop(expected);
    let limit = 16 << 20;
    let mut limited = Engine::<MockDevice>::from_opened(
        &config,
        MockDevice::opened_with(MockDevice::with_storage_limit(limit)),
    )
    .expect("limited renderer opens");
    assert!(
        matches!(limited.render(&scene, &camera()), Err(crate::RenderError::Gpu(molgfx_gpu::GpuError::LimitExceeded { resource: "cartoon vertices", limit: actual })) if actual == limit),
        "insufficient capacity is an explicit error, never lower fidelity"
    );
}
