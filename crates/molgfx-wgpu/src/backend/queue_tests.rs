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
            molgfx_gpu::block_on_polling(&device, detached.resolve_into(0, 32_768, &mut output))
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
        let result = molgfx_gpu::block_on_polling(
            &device,
            queue.read_buffer_into_async(&device, &buffer, offset, size, &mut output[..capacity]),
        );
        assert!(matches!(result, Err(GpuError::Runtime { .. })));
        assert!(output.iter().all(|byte| *byte == 0xcc));
    }
    molgfx_gpu::block_on_polling(
        &device,
        queue.read_buffer_into_async(&device, &buffer, 8, 16, &mut output),
    )
    .expect("async scratch reads subrange after errors");
    assert_eq!(&output[..16], &input[8..24]);
    assert_eq!(&output[16..], &vec![0xcc; output.len() - 16]);
    assert_eq!(
        queue
            .read_buffer_blocking(&device, &buffer, 8, 16)
            .expect("owned reads"),
        input[8..24]
    );
    let error = molgfx_gpu::block_on_polling(&device, detached.resolve(u64::MAX - 7, 16));
    assert!(matches!(error, Err(GpuError::Runtime { .. })));
    let error = molgfx_gpu::block_on_polling(
        &device,
        queue.read_buffer_into_async(&device, &source, 0, 4, &mut output),
    );
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

fn readback_fixture() -> (WgpuDevice, WgpuQueue, WgpuBuffer) {
    let opened = WgpuDevice::open_blocking(&DeviceDesc::default(), None).expect("device opens");
    let buffer = opened
        .device
        .create_buffer(&BufferDesc {
            label: "honest readback",
            size: 16,
            usage: BufferUsage::COPY_DST.union(BufferUsage::MAP_READ),
        })
        .expect("readback allocates");
    (opened.device, opened.queue, buffer)
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn a_mapped_readback_stays_pending_until_the_device_is_polled() {
    use std::task::{Context, Poll, Waker};
    let (device, queue, buffer) = readback_fixture();
    let mut read = std::pin::pin!(queue.read_buffer_async(&device, &buffer, 0, 16));
    let mut context = Context::from_waker(Waker::noop());
    assert!(read.as_mut().poll(&mut context).is_pending());
    assert!(read.as_mut().poll(&mut context).is_pending());
    let mut polls = 0_u32;
    let bytes = loop {
        device.poll().expect("device polls");
        if let Poll::Ready(result) = read.as_mut().poll(&mut context) {
            break result.expect("readback resolves");
        }
        polls += 1;
        assert!(polls < 100_000, "readback never resolved");
    };
    assert_eq!(bytes, [0; 16]);
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn dropping_a_pending_readback_unmaps_so_the_buffer_reads_again() {
    use std::task::{Context, Waker};
    let (device, queue, buffer) = readback_fixture();
    {
        let mut read = Box::pin(queue.read_buffer_async(&device, &buffer, 0, 16));
        let mut context = Context::from_waker(Waker::noop());
        assert!(read.as_mut().poll(&mut context).is_pending());
    }
    device.poll().expect("device polls");
    let bytes = queue
        .read_buffer_blocking(&device, &buffer, 0, 16)
        .expect("buffer maps after the cancelled read");
    assert_eq!(bytes, [0; 16]);
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn a_requested_map_finishes_only_after_polling_and_the_handle_is_reusable() {
    use molgfx_gpu::Readback as _;
    let (device, queue, buffer) = readback_fixture();
    let handle = queue.readback(&device, &buffer);
    let mut output = [0xcc; 16];
    assert!(matches!(
        handle.try_finish_into(&mut output),
        Err(GpuError::Runtime { .. })
    ));
    for _ in 0..3 {
        handle.begin_map(0, 16).expect("map requests");
        assert!(matches!(
            handle.begin_map(0, 16),
            Err(GpuError::Runtime { .. })
        ));
        let mut polls = 0_u32;
        while !handle.try_finish_into(&mut output).expect("map finishes") {
            device.poll().expect("device polls");
            polls += 1;
            assert!(polls < 100_000, "map never completed");
        }
        assert_eq!(output, [0; 16]);
        output.fill(0xcc);
    }
}
