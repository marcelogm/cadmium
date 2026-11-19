pub mod support;

use cadmium_engine::gpu::GpuContext;
use support::{line, session};

#[test]
fn timed_dab_stages_report_every_dab_of_the_frame_once() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    if !s.engine.time_dab_stages(&gpu) {
        eprintln!("skipped: the adapter has no timestamp queries");
        return;
    }
    let samples = line(128.0, 60.0, 300.0, 0.8, 20.0);
    s.begin_stroke(samples[0]);
    s.add_samples(&samples[1..]);
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    s.encode_frame(&gpu, 0.0, &mut encoder);
    s.engine.resolve_dab_stage_times(&mut encoder);
    gpu.queue.submit([encoder.finish()]);

    let times = s.engine.take_dab_stage_times(&gpu).unwrap();
    let stage = |name: &str| {
        times
            .iter()
            .find(|time| time.stage == name)
            .unwrap_or_else(|| panic!("no {name} stage in {times:?}"))
    };
    assert!(stage("Bristles").dabs > 0, "the frame painted no dab");
    assert_eq!(stage("Bristles").dabs, stage("Exchange").dabs);
    assert!(stage("Bristles").gpu_ms > 0.0);
    assert!(
        s.engine.take_dab_stage_times(&gpu).unwrap().is_empty(),
        "a second read reports the same frame again"
    );
}
