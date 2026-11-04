use std::sync::mpsc;

use cadmium_engine::{canvas::*, gpu::GpuContext, pigments::PigmentDb};

use crate::spec;

fn read_slot_table(gpu: &GpuContext, canvas: &Canvas) -> Vec<u8> {
    let table = canvas.slot_table();
    let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("slot_table_readback"),
        size: table.size(),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_buffer_to_buffer(table, 0, &staging, 0, table.size());
    gpu.queue.submit([encoder.finish()]);
    let slice = staging.slice(..);
    let (sender, receiver) = mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
    receiver.recv().unwrap().unwrap();
    slice.get_mapped_range().unwrap().to_vec()
}

#[test]
fn adding_a_slot_grows_the_palette_and_rewrites_the_slot_table() {
    let gpu = GpuContext::new().unwrap();
    let db = PigmentDb::builtin();
    let mut c = Canvas::new(&gpu, &db, spec(Primer::WhiteGesso, Weave::Smooth)).unwrap();
    assert_eq!(c.add_slot(&gpu, &db, "cadmium_red"), Ok(2));
    assert_eq!(
        c.spec.pigment_slots,
        ["titanium_white", "ultramarine_blue", "cadmium_red"]
    );
    let expected = PigmentSlotsUniform::new(&db, &c.spec.pigment_slots);
    assert_eq!(read_slot_table(&gpu, &c), bytemuck::bytes_of(&expected));
    assert_eq!(expected.count, 3);
}

#[test]
fn adding_an_unknown_duplicate_or_ninth_slot_is_refused() {
    let gpu = GpuContext::new().unwrap();
    let db = PigmentDb::builtin();
    let mut c = Canvas::new(&gpu, &db, spec(Primer::WhiteGesso, Weave::Smooth)).unwrap();
    assert!(c.add_slot(&gpu, &db, "unobtainium").is_err());
    assert!(c.add_slot(&gpu, &db, "ultramarine_blue").is_err());
    for id in [
        "cadmium_yellow",
        "cadmium_red",
        "ivory_black",
        "burnt_umber",
        "phthalo_blue",
        "yellow_ochre",
    ] {
        assert!(c.add_slot(&gpu, &db, id).is_ok(), "{id}");
    }
    assert!(c.add_slot(&gpu, &db, "burnt_sienna").is_err());
    assert_eq!(c.spec.pigment_slots.len(), 8);
}
