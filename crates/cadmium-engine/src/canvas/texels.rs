use super::StateTexture;

const BYTES_PER_HALF: usize = 2;
const BYTES_PER_FLOAT: usize = 4;

pub(super) fn decode_texels(which: StateTexture, bytes: &[u8]) -> Vec<f32> {
    match which.format() {
        wgpu::TextureFormat::R32Float => bytes
            .as_chunks::<BYTES_PER_FLOAT>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect(),
        _ => bytes
            .as_chunks::<BYTES_PER_HALF>()
            .0
            .iter()
            .map(|b| half_to_f32(u16::from_le_bytes(*b)))
            .collect(),
    }
}

pub(super) fn encode_texels(which: StateTexture, values: &[f32]) -> Vec<u8> {
    match which.format() {
        wgpu::TextureFormat::R32Float => values.iter().flat_map(|v| v.to_le_bytes()).collect(),
        _ => values
            .iter()
            .flat_map(|v| f32_to_half(*v).to_le_bytes())
            .collect(),
    }
}

const HALF_SIGN_SHIFT: u32 = 16;
const HALF_SIGN_MASK: u16 = 0x8000;
const HALF_EXPONENT_ALL_ONES: i32 = 0x1f;
const HALF_EXPONENT_MASK: u16 = 0x7c00;
const HALF_MANTISSA_MASK: u16 = 0x03ff;
const HALF_MANTISSA_BITS: u32 = 10;
const HALF_EXPONENT_BIAS: i32 = 15;
const HALF_INFINITY: u16 = 0x7c00;
const HALF_QUIET_NAN: u16 = 0x7e00;
const HALF_MIN_SUBNORMAL_EXPONENT: i32 = -24;
const FLOAT_EXPONENT_BIAS: i32 = 127;
const FLOAT_MANTISSA_BITS: u32 = 23;
const FLOAT_EXPONENT_MASK: u32 = 0xff;
const FLOAT_MANTISSA_MASK: u32 = 0x007f_ffff;
const FLOAT_IMPLICIT_ONE: u32 = 0x0080_0000;
const MANTISSA_SHIFT: u32 = FLOAT_MANTISSA_BITS - HALF_MANTISSA_BITS;

pub fn half_to_f32(bits: u16) -> f32 {
    let sign = if bits & HALF_SIGN_MASK != 0 {
        -1.0
    } else {
        1.0
    };
    let exponent = i32::from((bits & HALF_EXPONENT_MASK) >> HALF_MANTISSA_BITS);
    let mantissa = f32::from(bits & HALF_MANTISSA_MASK);
    let mantissa_scale = (1u32 << HALF_MANTISSA_BITS) as f32;
    match exponent {
        0 => sign * mantissa * 2f32.powi(HALF_MIN_SUBNORMAL_EXPONENT),
        HALF_EXPONENT_ALL_ONES if mantissa == 0.0 => sign * f32::INFINITY,
        HALF_EXPONENT_ALL_ONES => f32::NAN,
        _ => sign * (1.0 + mantissa / mantissa_scale) * 2f32.powi(exponent - HALF_EXPONENT_BIAS),
    }
}

pub fn f32_to_half(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = (bits >> HALF_SIGN_SHIFT) as u16 & HALF_SIGN_MASK;
    let float_exponent = (bits >> FLOAT_MANTISSA_BITS) & FLOAT_EXPONENT_MASK;
    let mantissa = bits & FLOAT_MANTISSA_MASK;
    if float_exponent == FLOAT_EXPONENT_MASK {
        return sign
            | if mantissa == 0 {
                HALF_INFINITY
            } else {
                HALF_QUIET_NAN
            };
    }
    let exponent = float_exponent as i32 - FLOAT_EXPONENT_BIAS + HALF_EXPONENT_BIAS;
    if exponent >= HALF_EXPONENT_ALL_ONES {
        return sign | HALF_INFINITY;
    }
    if exponent <= 0 {
        let shift = (1 - exponent) as u32 + MANTISSA_SHIFT;
        if shift > FLOAT_MANTISSA_BITS + 1 {
            return sign;
        }
        let full = mantissa | FLOAT_IMPLICIT_ONE;
        return sign | round_shift_to_nearest_even(full, shift) as u16;
    }
    let combined = ((exponent as u32) << FLOAT_MANTISSA_BITS) | mantissa;
    sign | round_shift_to_nearest_even(combined, MANTISSA_SHIFT) as u16
}

fn round_shift_to_nearest_even(value: u32, shift: u32) -> u32 {
    let kept = value >> shift;
    let remainder = value & ((1 << shift) - 1);
    let halfway = 1 << (shift - 1);
    if remainder > halfway || (remainder == halfway && kept & 1 == 1) {
        kept + 1
    } else {
        kept
    }
}
