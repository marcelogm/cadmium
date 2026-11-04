pub const BANDS: usize = 8;
pub type BandValues = [f32; BANDS];

pub const SURFACE_REFLECTION_K1: f32 = 0.04;
pub const INTERNAL_REFLECTION_K2: f32 = 0.6;
pub const D65_WHITE_XYZ: [f32; 3] = [0.95047, 1.0, 1.08883];
pub const XYZ_TO_LINEAR_SRGB: [[f32; 3]; 3] = [
    [3.2406, -1.5372, -0.4986],
    [-0.9689, 1.8758, 0.0415],
    [0.0557, -0.2040, 1.0570],
];
pub const LUMINANCE_FROM_LINEAR_SRGB: [f32; 3] = [0.2126, 0.7152, 0.0722];

const SPECTRAL_SAMPLES: usize = 31;
const OPTICAL_DEPTH_LIMIT: f32 = 20.0;
const NEGLIGIBLE_BETA: f32 = 1e-7;
const NEGLIGIBLE_SCATTERING: f32 = 1e-9;
const SRGB_LINEAR_SEGMENT_END: f32 = 0.003_130_8;
const SRGB_LINEAR_SLOPE: f32 = 12.92;
const SRGB_GAMMA: f32 = 2.4;
const SRGB_SCALE: f32 = 1.055;
const SRGB_OFFSET: f32 = 0.055;
const SRGB8_MAX: f32 = 255.0;

const BAND_OF_SAMPLE: [usize; SPECTRAL_SAMPLES] = [
    0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6, 7, 7, 7, 7,
];

const CIE1931_2DEG_CMF: [[f64; 3]; SPECTRAL_SAMPLES] = [
    [0.01431, 0.000396, 0.06785],
    [0.04351, 0.00121, 0.2074],
    [0.13438, 0.004, 0.6456],
    [0.2839, 0.0116, 1.3856],
    [0.34828, 0.023, 1.74706],
    [0.3362, 0.038, 1.77211],
    [0.2908, 0.06, 1.6692],
    [0.19536, 0.09098, 1.28764],
    [0.09564, 0.13902, 0.81295],
    [0.03201, 0.20802, 0.46518],
    [0.0049, 0.323, 0.272],
    [0.0093, 0.503, 0.1582],
    [0.06327, 0.71, 0.07825],
    [0.1655, 0.862, 0.04216],
    [0.2904, 0.954, 0.0203],
    [0.43345, 0.99495, 0.00875],
    [0.5945, 0.995, 0.0039],
    [0.7621, 0.952, 0.0021],
    [0.9163, 0.87, 0.00165],
    [1.0263, 0.757, 0.0011],
    [1.0622, 0.631, 0.0008],
    [1.0026, 0.503, 0.00034],
    [0.85445, 0.381, 0.00019],
    [0.6424, 0.265, 0.00005],
    [0.4479, 0.175, 0.00002],
    [0.2835, 0.107, 0.0],
    [0.1649, 0.061, 0.0],
    [0.0874, 0.032, 0.0],
    [0.04677, 0.017, 0.0],
    [0.0227, 0.00821, 0.0],
    [0.01136, 0.0041, 0.0],
];

const D65_SPD: [f64; SPECTRAL_SAMPLES] = [
    82.7549, 91.486, 93.4318, 86.6823, 104.865, 117.008, 117.812, 114.861, 115.923, 108.811,
    109.354, 107.802, 104.790, 107.689, 104.405, 104.046, 100.0, 96.3342, 95.788, 88.6856, 90.0062,
    89.5991, 87.6987, 83.2886, 83.6992, 80.0268, 80.2146, 82.2778, 78.2842, 69.7213, 71.6091,
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layer {
    pub reflectance: f32,
    pub transmittance: f32,
}

pub fn km_layer(absorption: f32, scattering: f32, thickness_um: f32) -> Layer {
    let k = absorption.max(0.0);
    let s = scattering.max(0.0);
    let x = thickness_um.max(0.0);
    let beta = (k * (k + 2.0 * s)).sqrt();
    if beta < NEGLIGIBLE_BETA {
        let sx = s * x;
        return Layer {
            reflectance: sx / (1.0 + sx),
            transmittance: 1.0 / (1.0 + sx),
        };
    }
    let optical_depth = beta * x;
    let bx = optical_depth.min(OPTICAL_DEPTH_LIMIT);
    let d = (k + s) * bx.sinh() + beta * bx.cosh();
    Layer {
        reflectance: s * bx.sinh() / d,
        transmittance: if optical_depth >= OPTICAL_DEPTH_LIMIT {
            0.0
        } else {
            beta / d
        },
    }
}

pub fn km_infinite(absorption: f32, scattering: f32) -> f32 {
    if scattering <= NEGLIGIBLE_SCATTERING {
        return 0.0;
    }
    let ratio = (absorption / scattering).max(0.0);
    1.0 / (1.0 + ratio + (ratio * ratio + 2.0 * ratio).sqrt())
}

pub fn composite_over(top: &Layer, below_reflectance: f32) -> f32 {
    top.reflectance
        + top.transmittance * top.transmittance * below_reflectance
            / (1.0 - top.reflectance * below_reflectance)
}

pub fn saunderson_display(reflectance: f32) -> f32 {
    (1.0 - SURFACE_REFLECTION_K1) * (1.0 - INTERNAL_REFLECTION_K2) * reflectance
        / (1.0 - INTERNAL_REFLECTION_K2 * reflectance)
}

pub fn band_xyz_weights() -> [[f32; BANDS]; 3] {
    let mut sums = [[0.0f64; BANDS]; 3];
    for (sample, (cmf, illuminant)) in CIE1931_2DEG_CMF.iter().zip(D65_SPD).enumerate() {
        for (channel, value) in cmf.iter().enumerate() {
            sums[channel][BAND_OF_SAMPLE[sample]] += value * illuminant;
        }
    }
    let mut weights = [[0.0f32; BANDS]; 3];
    for channel in 0..3 {
        let total: f64 = sums[channel].iter().sum();
        let scale = f64::from(D65_WHITE_XYZ[channel]) / total;
        for band in 0..BANDS {
            weights[channel][band] = (sums[channel][band] * scale) as f32;
        }
    }
    weights
}

pub fn bands_to_linear_srgb(reflectance: &BandValues) -> [f32; 3] {
    let weights = band_xyz_weights();
    let xyz = weights.map(|channel| {
        channel
            .iter()
            .zip(reflectance)
            .map(|(weight, value)| weight * value)
            .sum::<f32>()
    });
    XYZ_TO_LINEAR_SRGB.map(|row| row.iter().zip(xyz).map(|(m, v)| m * v).sum())
}

pub fn compress_gamut(rgb: [f32; 3]) -> [f32; 3] {
    let mut out = rgb;
    let lowest = out.iter().copied().fold(f32::INFINITY, f32::min);
    if lowest < 0.0 {
        let gray = out
            .iter()
            .zip(LUMINANCE_FROM_LINEAR_SRGB)
            .map(|(c, w)| c * w)
            .sum::<f32>();
        if gray <= 0.0 {
            return [0.0; 3];
        }
        let toward_gray = lowest / (lowest - gray);
        out = out.map(|c| (c + (gray - c) * toward_gray).max(0.0));
    }
    let highest = out.iter().copied().fold(0.0f32, f32::max);
    if highest > 1.0 {
        out = out.map(|c| c / highest);
    }
    out
}

pub fn linear_to_srgb8(value: f32) -> u8 {
    let v = value.clamp(0.0, 1.0);
    let encoded = if v <= SRGB_LINEAR_SEGMENT_END {
        SRGB_LINEAR_SLOPE * v
    } else {
        SRGB_SCALE * v.powf(1.0 / SRGB_GAMMA) - SRGB_OFFSET
    };
    (encoded * SRGB8_MAX).round().clamp(0.0, SRGB8_MAX) as u8
}
