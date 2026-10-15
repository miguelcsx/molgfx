use super::*;
use crate::testing::MockDevice;
use molgfx_gpu::BindGroupLayoutDesc;

fn arena() -> ExposureArena<MockDevice> {
    let device = MockDevice::default();
    let layout = device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "exposure test",
        entries: &[],
    });
    ExposureArena::new(&device, &layout).unwrap()
}

#[test]
fn an_in_flight_bank_is_reusable_only_after_its_own_final_fence() {
    let mut arena = arena();
    let mut group = 999;
    arena.submit(0, FenceValue(4), &mut group);
    arena.submit(1, FenceValue(7), &mut group);
    arena.submit(2, FenceValue(9), &mut group);
    assert_eq!(arena.available(FenceValue(3)), None);
    assert_eq!(arena.available(FenceValue(4)), Some(0));
    arena.submit(0, FenceValue(12), &mut group);
    assert_eq!(arena.available(FenceValue(6)), None);
    assert_eq!(arena.available(FenceValue(7)), Some(1));
    assert_eq!(arena.available(FenceValue(8)), Some(1));
    arena.submit(1, FenceValue(13), &mut group);
    assert_eq!(arena.available(FenceValue(9)), Some(2));
}

#[test]
fn every_sample_retains_its_distinct_uniform_payload_until_upload() {
    let mut arena = arena();
    let mut group = 999;
    for sample in 0..EXPOSURE_SAMPLES {
        let mut uniforms: FrameUniforms = bytemuck::Zeroable::zeroed();
        uniforms.temporal[3] = f32::from(u8::try_from(sample).unwrap());
        arena.stage(1, sample, &uniforms, &mut group).unwrap();
    }
    for sample in 0..EXPOSURE_SAMPLES {
        let offset = sample * arena.stride;
        let uniforms: FrameUniforms = bytemuck::pod_read_unaligned(
            &arena.staging[offset..offset + std::mem::size_of::<FrameUniforms>()],
        );
        assert_eq!(
            uniforms.temporal[3].to_bits(),
            f32::from(u8::try_from(sample).unwrap()).to_bits()
        );
    }
    arena.restore(&mut group);
    assert_eq!(group, 999);
    let uniforms = bytemuck::Zeroable::zeroed();
    assert!(
        arena
            .stage(0, EXPOSURE_SAMPLES, &uniforms, &mut group)
            .is_err()
    );
    assert!(arena.stage(BANKS, 0, &uniforms, &mut group).is_err());
    assert_eq!(group, 999);
}
