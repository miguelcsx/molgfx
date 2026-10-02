use super::{GraphTopology, TransparentDepth, realtime_nodes, realtime_resources};
use crate::graph::{PassKind, SizeClass, plan_aliases, schedule};
use crate::passes::{SEGMENT_LABEL_RESOURCE, SEGMENT_VOLUME_RESOURCE};
use crate::testing::MockDevice;
use molgfx_gpu::{TextureFormat, TextureUsage};

#[test]
fn categorical_graph_resources_are_full_resolution_integer_copy_sources() {
    let resources = realtime_resources();
    let source = resources
        .get(SEGMENT_VOLUME_RESOURCE.0 as usize)
        .expect("categorical source resource is declared");
    let label = resources
        .get(SEGMENT_LABEL_RESOURCE.0 as usize)
        .expect("categorical label resource is declared");
    for resource in [source, label] {
        assert_eq!(resource.format, TextureFormat::R32Uint);
        assert_eq!(resource.size, crate::graph::SizeClass::Full);
        assert!(resource.usage.contains(TextureUsage::RENDER_ATTACHMENT));
        assert!(resource.usage.contains(TextureUsage::COPY_SRC));
    }
}

#[test]
fn categorical_graph_clears_ids_before_the_four_target_oit_writer() {
    let nodes = realtime_nodes::<MockDevice>(GraphTopology::default());
    let clear = nodes
        .iter()
        .position(|node| node.name == "clear categorical segment ids")
        .expect("categorical clear node is declared");
    let draw = nodes
        .iter()
        .position(|node| node.name == "categorical segment volumes")
        .expect("categorical OIT node is declared");
    assert!(clear < draw);
    assert_eq!(nodes[clear].kind, PassKind::Graphics);
    assert!(nodes[clear].writes.contains(&SEGMENT_VOLUME_RESOURCE));
    assert!(nodes[clear].writes.contains(&SEGMENT_LABEL_RESOURCE));
    assert!(nodes[draw].writes.contains(&SEGMENT_VOLUME_RESOURCE));
    assert!(nodes[draw].writes.contains(&SEGMENT_LABEL_RESOURCE));
}

#[test]
fn screen_overlays_are_composed_after_tonemapping() {
    let nodes = realtime_nodes::<MockDevice>(GraphTopology::default());
    let tonemap = nodes
        .iter()
        .position(|node| node.name == "HDR tonemap")
        .expect("tonemap");
    let overlay = nodes
        .iter()
        .position(|node| node.name == "screen overlays")
        .expect("overlay");
    assert!(tonemap < overlay);
    assert_eq!(
        nodes[overlay].writes.as_slice(),
        &[crate::graph::ResourceId::SWAPCHAIN]
    );
}

#[test]
fn cinematic_full_resolution_hdr_targets_need_only_five_physical_textures() {
    let resources = realtime_resources();
    let nodes = realtime_nodes::<MockDevice>(GraphTopology {
        depth_of_field: true,
        bloom: true,
        motion_blur: true,
        transparent_depth: TransparentDepth::Live,
    });
    let order = match schedule(&nodes) {
        Ok(value) => value,
        Err(error) => panic!("cinematic graph schedules: {error}"),
    };
    let plan = plan_aliases(&resources, &nodes, &order);
    let hdr_targets = resources
        .iter()
        .enumerate()
        .filter(|(_, resource)| {
            resource.format == TextureFormat::Rgba16Float
                && resource.size == SizeClass::Full
                && resource.usage.contains(TextureUsage::COPY_SRC)
        })
        .collect::<Vec<_>>();
    assert_eq!(hdr_targets.len(), 7);
    let mut slots = hdr_targets
        .iter()
        .map(|(row, _)| plan.slot[*row])
        .collect::<Vec<_>>();
    slots.sort_unstable();
    slots.dedup();

    assert_eq!(slots.len(), 5);
}

#[test]
fn a_device_that_cannot_sample_the_live_depth_gives_transparent_passes_a_copy() {
    use crate::passes::{DEPTH_RESOURCE, OPAQUE_DEPTH_RESOURCE};
    let direct = realtime_nodes::<MockDevice>(GraphTopology::default());
    assert!(
        direct
            .iter()
            .all(|node| node.name != "opaque depth snapshot"),
        "a device that can bind the live depth copies nothing"
    );
    let copied = realtime_nodes::<MockDevice>(GraphTopology {
        transparent_depth: TransparentDepth::Copy,
        ..GraphTopology::default()
    });
    let snapshot = copied
        .iter()
        .position(|node| node.name == "opaque depth snapshot")
        .expect("the snapshot node is declared");
    assert_eq!(copied[snapshot].reads.as_slice(), &[DEPTH_RESOURCE]);
    assert_eq!(copied[snapshot].writes.as_slice(), &[OPAQUE_DEPTH_RESOURCE]);
    for (index, node) in copied.iter().enumerate() {
        let samples_opaque_scene = node.name.starts_with("transparent ")
            || node.name == "direct density volumes"
            || node.name == "categorical segment volumes";
        if samples_opaque_scene && node.name != "transparent sphere impostors" {
            assert!(index > snapshot, "{} follows the snapshot", node.name);
        }
        if samples_opaque_scene {
            assert!(
                node.reads.contains(&OPAQUE_DEPTH_RESOURCE),
                "{} reads the copy",
                node.name
            );
        }
    }
}
