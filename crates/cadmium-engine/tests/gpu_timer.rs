pub mod support;

use cadmium_engine::gpu::GpuContext;
use cadmium_engine::gpu_timer::GpuFrameTimer;
use support::{line, session};

const WORK: usize = 7;

#[test]
fn a_timed_frame_comes_back_with_its_work_and_the_gpu_time_it_took() {
    let gpu = GpuContext::new().unwrap();
    let Some(mut timer) = GpuFrameTimer::new(&gpu) else {
        return;
    };
    let mut s = session(&gpu);
    let samples = line(128.0, 40.0, 470.0, 0.8, 20.0);
    s.begin_stroke(samples[0]);
    s.add_samples(&samples[1..]);
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    let slot = timer
        .begin(&mut encoder)
        .expect("a fresh timer has free slots");
    s.encode_frame(&gpu, 0.0, &mut encoder);
    timer.end(&mut encoder, slot, WORK);
    gpu.queue.submit([encoder.finish()]);
    timer.after_submit();
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    let frames = timer.finished(&gpu);
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].work, WORK);
    assert!(frames[0].gpu_ms > 0.0, "{frames:?}");
    assert!(timer.finished(&gpu).is_empty(), "a frame comes back once");
}
