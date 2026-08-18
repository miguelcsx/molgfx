use super::{realtime_nodes, realtime_resources};
use crate::graph::PassKind;
use crate::passes::{SEGMENT_LABEL_RESOURCE, SEGMENT_VOLUME_RESOURCE};
use crate::testing::MockDevice;
use pdviewx_gpu::{TextureFormat, TextureUsage};

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
    let nodes = realtime_nodes::<MockDevice>(false, false, false);
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
    let nodes = realtime_nodes::<MockDevice>(false, false, false);
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
