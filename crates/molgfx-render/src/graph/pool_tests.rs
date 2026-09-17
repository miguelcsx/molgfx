use super::*;
use crate::graph::node::{PassKind, ResourceId};
use crate::testing::MockDevice;
use molgfx_gpu::{TextureFormat, TextureUsage};

fn noop(_: &mut crate::graph::PassContext<'_, MockDevice>) {}

fn desc(label: &'static str) -> ResourceDesc {
    ResourceDesc {
        label,
        format: TextureFormat::Rgba16Float,
        size: SizeClass::Full,
        usage: TextureUsage::RENDER_ATTACHMENT.union(TextureUsage::TEXTURE_BINDING),
        persistent: false,
    }
}

#[test]
fn persistent_history_does_not_alias_frame_writers() {
    let early = desc("early transient");
    let mut history = desc("temporal history");
    history.persistent = true;
    let late = desc("late transient");
    let resources = vec![early, history, late];
    let passes = vec![
        pass("write transient", &[], &[0]),
        pass("read transient", &[0], &[]),
        pass("read history", &[1], &[]),
        pass("write history", &[], &[1]),
        pass("write late transient", &[], &[2]),
        pass("read late transient", &[2], &[]),
    ];

    let plan = plan_aliases(&resources, &passes, &[0, 1, 2, 3, 4, 5]);

    assert_ne!(plan.slot[0], plan.slot[1]);
    assert_ne!(plan.slot[1], plan.slot[2]);
}

fn pass(name: &'static str, reads: &[u32], writes: &[u32]) -> PassNode<MockDevice> {
    PassNode {
        name,
        reads: reads.iter().map(|&r| ResourceId(r)).collect(),
        writes: writes.iter().map(|&w| ResourceId(w)).collect(),
        kind: PassKind::Graphics,
        record: noop,
    }
}

#[test]
fn non_overlapping_lifetimes_share_one_physical_texture() {
    // r0 lives in steps 0-1, r1 lives in steps 2-3: same shape, no overlap.
    let resources = vec![desc("early"), desc("late")];
    let passes = vec![
        pass("write_early", &[], &[0]),
        pass("read_early", &[0], &[]),
        pass("write_late", &[], &[1]),
        pass("read_late", &[1], &[]),
    ];
    let order = vec![0, 1, 2, 3];
    let plan = plan_aliases(&resources, &passes, &order);
    assert_eq!(plan.slots, 1, "the two transients alias one texture");
    assert_eq!(plan.slot[0], plan.slot[1]);
}

#[test]
fn overlapping_lifetimes_get_distinct_textures() {
    let resources = vec![desc("a"), desc("b")];
    let passes = vec![
        pass("write_both", &[], &[0, 1]),
        pass("read_both", &[0, 1], &[]),
    ];
    let order = vec![0, 1];
    let plan = plan_aliases(&resources, &passes, &order);
    assert_eq!(plan.slots, 2);
    assert_ne!(plan.slot[0], plan.slot[1]);
}

#[test]
fn different_shapes_never_alias_even_without_overlap() {
    let mut depth = desc("depth");
    depth.format = TextureFormat::Depth32Float;
    let resources = vec![desc("full"), depth];
    let passes = vec![
        pass("write_full", &[], &[0]),
        pass("read_full", &[0], &[]),
        pass("write_half", &[], &[1]),
        pass("read_half", &[1], &[]),
    ];
    let plan = plan_aliases(&resources, &passes, &[0, 1, 2, 3]);
    assert_eq!(plan.slots, 2);
}

#[test]
fn the_physical_pool_is_stable_across_frames_at_one_size() {
    let resources = vec![desc("hdr")];
    let passes = vec![pass("write", &[], &[0]), pass("read", &[0], &[])];
    let order = vec![0, 1];
    let plan = plan_aliases(&resources, &passes, &order);
    let opened = match <MockDevice as molgfx_gpu::Device>::open_blocking(
        &molgfx_gpu::DeviceDesc::default(),
        None,
    ) {
        Ok(opened) => opened,
        Err(e) => panic!("mock opens: {e}"),
    };
    let pool = match TransientPool::build(&opened.device, &resources, plan, 640, 480) {
        Ok(pool) => pool,
        Err(e) => panic!("pool builds: {e}"),
    };
    // One texture was created; a second frame at the same size reuses the
    // pool without touching the device.
    let Ok(created_log) = opened.device.log.textures.lock() else {
        panic!("log lock")
    };
    let created = created_log.len();
    assert_eq!(created, 1);
    assert!(pool.matches(640, 480));
    assert!(!pool.matches(800, 600));
    assert!(pool.view(ResourceId(0)).is_some());
    assert!(pool.view(ResourceId(7)).is_none());
}

#[test]
fn tile_classification_targets_round_up_partial_frame_tiles() {
    let mut tiles = desc("tiles");
    tiles.size = SizeClass::Tiles16;
    let resources = vec![tiles];
    let passes = vec![pass("write", &[], &[0])];
    let plan = plan_aliases(&resources, &passes, &[0]);
    let opened = match <MockDevice as molgfx_gpu::Device>::open_blocking(
        &molgfx_gpu::DeviceDesc::default(),
        None,
    ) {
        Ok(opened) => opened,
        Err(error) => panic!("mock opens: {error}"),
    };
    if let Err(error) = TransientPool::build(&opened.device, &resources, plan, 641, 481) {
        panic!("tile pool builds: {error}")
    }
    let Ok(extents) = opened.device.log.texture_extents.lock() else {
        panic!("extent log locks")
    };
    assert_eq!(extents.as_slice(), &[("tiles", [41, 31, 1])]);
}

#[test]
fn inactive_resources_have_no_allocation_even_if_marked_persistent() {
    let mut dormant = desc("inactive history");
    dormant.persistent = true;
    let resources = [desc("active"), desc("inactive effect"), dormant];
    let passes = [
        pass("write active", &[], &[0]),
        pass("disabled", &[], &[1, 2]),
    ];
    let plan = plan_aliases(&resources, &passes, &[0]);
    assert_eq!(plan.slots, 1);
    let opened = MockDevice::open_blocking(&molgfx_gpu::DeviceDesc::default(), None)
        .expect("mock device opens");
    let pool = TransientPool::build(&opened.device, &resources, plan, 1920, 1080)
        .expect("active pool builds");
    assert!(pool.view(ResourceId(0)).is_some());
    assert!(pool.view(ResourceId(1)).is_none());
    assert!(pool.texture(ResourceId(2)).is_none());
    assert_eq!(
        opened
            .device
            .log
            .textures
            .lock()
            .expect("log locks")
            .as_slice(),
        &["active"]
    );
}
