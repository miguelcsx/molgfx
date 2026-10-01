use super::*;
use molgfx_gpu::{
    BufferDesc, BufferUsage, CommandEncoder as _, Device as _, DeviceDesc, Queue as _,
};

#[test]
#[ignore = "requires a native GPU adapter"]
fn reused_readback_scratch_preserves_tail_and_recovers_after_invalid_requests() {
    let opened = WgpuDevice::open_blocking(&DeviceDesc::default(), None).expect("device opens");
    let device = opened.device;
    let queue = opened.queue;
    let source = device
        .create_buffer(&BufferDesc {
            label: "readback source",
            size: 32_768,
            usage: BufferUsage::COPY_SRC.union(BufferUsage::COPY_DST),
        })
        .expect("source allocates");
    let buffer = device
        .create_buffer(&BufferDesc {
            label: "reused readback",
            size: 32_768,
            usage: BufferUsage::COPY_DST.union(BufferUsage::MAP_READ),
        })
        .expect("readback allocates");
    let input: Vec<_> = (0..32_768)
        .map(|i| u8::try_from(i % 251).expect("byte"))
        .collect();
    queue.write_buffer(&source, 0, &input);
    let mut encoder = device.create_command_encoder();
    encoder.copy_buffer_to_buffer(&source, 0, &buffer, 0, 32_768);
    queue.submit(encoder);
    let detached = queue.readback(&device, &buffer);
    let mut output = vec![0xcc; 32_776];
    let pointer = output.as_ptr();
    let capacity = output.capacity();
    for iteration in 0..12 {
        output.fill(0xcc);
        if iteration % 2 == 0 {
            queue
                .read_buffer_into_blocking(&device, &buffer, 0, 32_768, &mut output)
                .expect("blocking scratch reads");
        } else {
            pollster::block_on(detached.resolve_into(0, 32_768, &mut output))
                .expect("detached scratch reads");
        }
        assert_eq!(&output[..32_768], &input);
        assert_eq!(&output[32_768..], &[0xcc; 8]);
        assert_eq!((output.as_ptr(), output.capacity()), (pointer, capacity));
    }
    for (offset, size, capacity) in [
        (u64::MAX - 7, 16, 16),
        (32_768, 4, 16),
        (0, 0, 16),
        (4, 4, 16),
        (0, 6, 16),
        (0, 16, 15),
    ] {
        output.fill(0xcc);
        let result = pollster::block_on(queue.read_buffer_into_async(
            &device,
            &buffer,
            offset,
            size,
            &mut output[..capacity],
        ));
        assert!(matches!(result, Err(GpuError::Runtime { .. })));
        assert!(output.iter().all(|byte| *byte == 0xcc));
    }
    pollster::block_on(queue.read_buffer_into_async(&device, &buffer, 8, 16, &mut output))
        .expect("async scratch reads subrange after errors");
    assert_eq!(&output[..16], &input[8..24]);
    assert_eq!(&output[16..], &vec![0xcc; output.len() - 16]);
    assert_eq!(
        queue
            .read_buffer_blocking(&device, &buffer, 8, 16)
            .expect("owned reads"),
        input[8..24]
    );
    let error = pollster::block_on(detached.resolve(u64::MAX - 7, 16));
    assert!(matches!(error, Err(GpuError::Runtime { .. })));
    let error =
        pollster::block_on(queue.read_buffer_into_async(&device, &source, 0, 4, &mut output));
    assert!(matches!(error, Err(GpuError::Runtime { .. })));
    device
        .check_errors()
        .expect("invalid requests did not reach backend validation");
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn mapping_failure_unmaps_the_buffer_before_the_next_read() {
    let opened = WgpuDevice::open_blocking(&DeviceDesc::default(), None).expect("device opens");
    let device = opened.device;
    let queue = opened.queue;
    let buffer = device
        .create_buffer(&BufferDesc {
            label: "mapping lifecycle",
            size: 16,
            usage: BufferUsage::COPY_DST.union(BufferUsage::MAP_READ),
        })
        .expect("readback allocates");
    buffer
        .raw
        .slice(..)
        .map_async(wgpu::MapMode::Read, |result| {
            result.expect("initial map completes");
        });
    device
        .device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("map polled");
    let mut output = [0xcc; 16];
    let error = queue.read_buffer_into_blocking(&device, &buffer, 0, 16, &mut output);
    assert!(matches!(error, Err(GpuError::Runtime { .. })));
    assert_eq!(output, [0xcc; 16]);
    queue
        .read_buffer_into_blocking(&device, &buffer, 0, 16, &mut output)
        .expect("read after failed map completes");
    assert_eq!(output, [0; 16]);
    device.check_errors().expect("mapping lifecycle validates");
}
