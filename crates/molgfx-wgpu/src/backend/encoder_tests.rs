use super::*;
use molgfx_gpu::{
    BufferDesc, BufferUsage, CommandEncoder as _, Device as _, DeviceDesc, LoadOp, Queue as _,
    TextureDesc, TextureDimension, TextureFormat, TextureUsage, TextureViewDesc,
};

#[test]
#[ignore = "requires a native GPU adapter"]
fn inline_attachment_descriptors_preserve_every_color_target() {
    let opened = WgpuDevice::open_blocking(&DeviceDesc::default(), None).expect("device opens");
    let device = opened.device;
    let queue = opened.queue;
    let readback = device
        .create_buffer(&BufferDesc {
            label: "attachment readback",
            size: 256,
            usage: BufferUsage::COPY_DST.union(BufferUsage::MAP_READ),
        })
        .expect("readback allocates");
    for count in [1, 5, 8] {
        let textures: Vec<_> = (0..count)
            .map(|_| {
                device
                    .create_texture(&TextureDesc {
                        label: "inline color target",
                        width: 1,
                        height: 1,
                        depth: 1,
                        dimension: TextureDimension::D2,
                        // Eight R32Uint targets fit the portable 32-byte sample budget.
                        format: TextureFormat::R32Uint,
                        usage: TextureUsage::RENDER_ATTACHMENT.union(TextureUsage::COPY_SRC),
                    })
                    .expect("color target allocates")
            })
            .collect();
        let views: Vec<_> = textures
            .iter()
            .map(|texture| device.create_texture_view(texture, &TextureViewDesc::default()))
            .collect();
        let colors: Vec<_> = views
            .iter()
            .map(|view| ColorAttachment {
                view,
                load: LoadOp::Clear([1.0, 0.0, 0.0, 1.0]),
            })
            .collect();
        let mut encoder = device.create_command_encoder();
        drop(encoder.begin_render_pass(&RenderPassDesc {
            label: "inline attachment conversion",
            colors: &colors,
            depth: None,
            timestamps: None,
        }));
        queue.submit(encoder);
        for texture in &textures {
            let mut copy = device.create_command_encoder();
            copy.copy_texture_to_buffer(texture, (0, 0), (1, 1), 256, 0, &readback);
            queue.submit(copy);
            let bytes = queue
                .read_buffer_blocking(&device, &readback, 0, 4)
                .expect("color target reads back");
            assert_eq!(bytes, [1, 0, 0, 0]);
        }
    }
}
