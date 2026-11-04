pub(super) mod dip;
pub(super) mod dry;
pub(super) mod pick;

fn dispatch(pass: &mut wgpu::ComputePass<'_>, pipeline: &wgpu::ComputePipeline, [x, y]: [u32; 2]) {
    pass.set_pipeline(pipeline);
    pass.dispatch_workgroups(x, y, 1);
}
