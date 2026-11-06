use std::mem::offset_of;

use super::{DAB_RECORD_SIZE, DAB_RECORD_STRIDE, DabRecord};

const UNIFORM_ALIGNMENT_BYTES: u64 = 16;
const SCALAR_BYTES: u64 = 4;
const VECTOR2_BYTES: u64 = 2 * SCALAR_BYTES;

struct WgslField {
    name: String,
    offset_bytes: u64,
}

fn wgsl_field_bytes(kind: &str) -> u64 {
    match kind {
        "vec2<f32>" | "vec2<i32>" | "vec2<u32>" => VECTOR2_BYTES,
        _ => SCALAR_BYTES,
    }
}

fn wgsl_dab_fields() -> (Vec<WgslField>, u64) {
    let source = include_str!("../../shaders/brush_tip.wgsl");
    let body = source
        .split("struct Dab {")
        .nth(1)
        .and_then(|rest| rest.split("};").next())
        .expect("brush_tip.wgsl declares struct Dab");
    let mut fields = Vec::new();
    let mut offset_bytes = 0_u64;
    for (name, kind) in body.lines().filter_map(|line| line.trim().split_once(':')) {
        let size_bytes = wgsl_field_bytes(kind.trim().trim_end_matches(','));
        offset_bytes = offset_bytes.next_multiple_of(size_bytes);
        fields.push(WgslField {
            name: name.trim().to_string(),
            offset_bytes,
        });
        offset_bytes += size_bytes;
    }
    (fields, offset_bytes)
}

const SCALAR_OFFSET: usize = SCALAR_BYTES as usize;

macro_rules! field {
    ($name:ident) => {
        (stringify!($name), offset_of!(DabRecord, $name))
    };
    ($label:literal, $name:ident, $lane:literal) => {
        ($label, offset_of!(DabRecord, $name) + $lane * SCALAR_OFFSET)
    };
}

const RUST_DAB_OFFSETS: [(&str, usize); 55] = [
    field!(center),
    field!(radius),
    field!(pressure),
    field!(altitude),
    field!(first_rest),
    field!(direction),
    field!(speed),
    field!(orientation),
    field!(elongation),
    field!(tool_mode),
    field!(canvas_origin),
    field!(canvas_size),
    field!(tip_origin),
    field!(tip_size),
    field!(transfer_fraction),
    field!(velocity_cutoff),
    field!(residual_um),
    field!(penetration_um),
    field!(bristle_contact),
    field!(full_reserve_volume),
    field!(refill_rate),
    field!(loaded_thickness_um),
    field!(advection),
    field!(squeeze),
    field!(diffusion),
    field!(blade_gap_um),
    field!(spread_origin),
    field!(spread_size),
    field!(rag_strength),
    field!(rest_follow),
    field!(blade_capacity_um),
    field!(blade_spill_share),
    field!(step_ratio),
    field!("sweep_x", sweep, 0),
    field!("sweep_y", sweep, 1),
    field!(capillary_rate),
    field!(wicking_rate),
    field!(capillary_scale),
    field!(handle_height_px),
    field!("handle_axis_x", handle_axis, 0),
    field!("handle_axis_y", handle_axis, 1),
    field!("handle_axis_z", handle_axis, 2),
    field!(handle_rest_height_px),
    field!(ferrule_radius_px),
    field!(dt_seconds),
    field!(substeps),
    field!(bristle_total),
    field!(bristle_stiffness),
    field!(bristle_damping),
    field!(bristle_reset),
    field!(bristle_orientation),
    field!(ferrule_press_share),
    field!(slump_rate),
    field!(slump_yield_um2_per_px),
    field!(hair_load_limit),
];

#[test]
fn the_dab_record_fits_its_uniform_stride() {
    assert_eq!(DAB_RECORD_SIZE, 256);
    const { assert!(DAB_RECORD_SIZE <= DAB_RECORD_STRIDE) };
    assert_eq!(DAB_RECORD_STRIDE, 256);
    assert_eq!(DAB_RECORD_SIZE % UNIFORM_ALIGNMENT_BYTES, 0);
}

#[test]
fn the_shader_dab_struct_mirrors_the_dab_record() {
    let (fields, wgsl_size_bytes) = wgsl_dab_fields();
    let wgsl_offsets: Vec<(String, u64)> = fields
        .into_iter()
        .map(|field| (field.name, field.offset_bytes))
        .collect();
    let rust_offsets: Vec<(String, u64)> = RUST_DAB_OFFSETS
        .iter()
        .map(|&(name, offset)| (name.to_string(), offset as u64))
        .collect();
    assert_eq!(wgsl_offsets, rust_offsets);
    assert_eq!(wgsl_size_bytes, DAB_RECORD_SIZE);
}
