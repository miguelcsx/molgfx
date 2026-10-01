use super::*;
use crate::testing::MockDevice;
use molgfx_gpu::{BufferDesc, Device as _, Queue as _, Readback as _};

#[test]
fn caller_scratch_reads_the_requested_pick_subrange_and_preserves_its_tail() {
    let opened = MockDevice::opened();
    let device = opened.device;
    let queue = opened.queue;
    *device.log.pick_local_row.lock().expect("row") = 41;
    *device.log.pick_resident_page.lock().expect("page") = 27;
    let buffer = device
        .create_buffer(&BufferDesc {
            label: "packed pick readback",
            size: 1024,
            usage: BufferUsage::MAP_READ.union(BufferUsage::COPY_DST),
        })
        .expect("buffer");
    let handle = queue.readback(&device, &buffer);
    let mut output = [0xcc; 8];
    queue
        .read_buffer_into_blocking(&device, &buffer, 256, 4, &mut output)
        .expect("scratch read");
    assert_eq!(&output[..4], &27_u32.to_le_bytes());
    assert_eq!(&output[4..], &[0xcc; 4]);
    pollster::block_on(handle.resolve_into(0, 4, &mut output)).expect("detached read");
    assert_eq!(&output[..4], &41_u32.to_le_bytes());
    assert_eq!(&output[4..], &[0xcc; 4]);
    assert_eq!(
        queue
            .read_buffer_blocking(&device, &buffer, 256, 4)
            .expect("owned read"),
        27_u32.to_le_bytes()
    );
}

#[test]
fn invalid_mock_read_ranges_leave_reserved_scratch_unchanged() {
    let opened = MockDevice::opened();
    let buffer = opened
        .device
        .create_buffer(&BufferDesc {
            label: "local row pick readback",
            size: 16,
            usage: BufferUsage::MAP_READ,
        })
        .expect("buffer");
    for (offset, size, capacity) in [
        (u64::MAX - 7, 16, 8),
        (16, 4, 8),
        (0, 0, 8),
        (4, 4, 8),
        (0, 6, 8),
        (0, 8, 7),
    ] {
        let mut output = [0xcc; 8];
        let result = pollster::block_on(opened.queue.read_buffer_into_async(
            &opened.device,
            &buffer,
            offset,
            size,
            &mut output[..capacity],
        ));
        assert!(matches!(result, Err(GpuError::Runtime { .. })));
        assert_eq!(output, [0xcc; 8]);
    }
    let unreadable = opened
        .device
        .create_buffer(&BufferDesc {
            label: "unreadable",
            size: 16,
            usage: BufferUsage::COPY_DST,
        })
        .expect("buffer");
    let mut output = [0xcc; 8];
    assert!(matches!(
        opened
            .queue
            .read_buffer_into_blocking(&opened.device, &unreadable, 0, 4, &mut output),
        Err(GpuError::Runtime { .. })
    ));
    assert_eq!(output, [0xcc; 8]);
}
