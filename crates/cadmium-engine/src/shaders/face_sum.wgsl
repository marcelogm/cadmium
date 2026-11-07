// Sums the mixing knife's face, channel by channel, for a cheap readout.

const FACE_SUM_THREADS: u32 = 256u;

var<workgroup> partial: array<f32, FACE_SUM_THREADS>;

@group(0) @binding(0) var<storage, read> face_load: array<f32>;
@group(0) @binding(1) var<storage, read_write> face_sums: array<f32, TIP_CHANNELS>;

@compute @workgroup_size(FACE_SUM_THREADS)
fn main(@builtin(local_invocation_index) local: u32) {
    let cells = arrayLength(&face_load) / TIP_CHANNELS;
    var mine = array<f32, TIP_CHANNELS>();
    for (var cell = local; cell < cells; cell += FACE_SUM_THREADS) {
        for (var channel = 0u; channel < TIP_CHANNELS; channel++) {
            mine[channel] += face_load[cell * TIP_CHANNELS + channel];
        }
    }
    for (var channel = 0u; channel < TIP_CHANNELS; channel++) {
        partial[local] = mine[channel];
        workgroupBarrier();
        for (var stride = FACE_SUM_THREADS / 2u; stride > 0u; stride /= 2u) {
            if (local < stride) {
                partial[local] += partial[local + stride];
            }
            workgroupBarrier();
        }
        if (local == 0u) {
            face_sums[channel] = partial[0];
        }
        workgroupBarrier();
    }
}
