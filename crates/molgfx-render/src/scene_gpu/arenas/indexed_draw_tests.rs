use super::*;
use crate::testing::MockDevice;

#[test]
fn indexed_arguments_use_the_hardware_five_word_layout_and_reuse_the_allocation() {
    let device = MockDevice::default();
    let queue = device.queue();
    let mut buffer = None;
    write_arguments(&device, &queue, "indexed test", 9, &mut buffer).expect("arguments upload");
    write_arguments(&device, &queue, "indexed test", 0, &mut buffer)
        .expect("empty arguments upload");
    let buffers = device.log.buffers.lock().expect("buffers recorded");
    assert_eq!(buffers.len(), 1);
    assert_eq!(buffers[0].2, 20);
    let payloads = device.log.write_payloads.lock().expect("uploads recorded");
    assert_eq!(
        bytemuck::cast_slice::<u8, u32>(&payloads[0]),
        &[9, 1, 0, 0, 0]
    );
    assert_eq!(
        bytemuck::cast_slice::<u8, u32>(&payloads[1]),
        &[0, 0, 0, 0, 0]
    );
}
