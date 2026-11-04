use wgpu::util::DeviceExt;

use crate::brush::{
    BrushSettings, BrushShape, DEFAULT_MAX_DIAMETER_PX, DEFAULT_SPACING, FULL_RESERVOIR_VOLUME_UM,
};
use crate::canvas::{Canvas, PaintTotals, StateTexture, TILE_SIZE, TileCoord};
use crate::gpu::{GpuContext, GpuLost, read_mapped, staging_buffer};
use crate::params::SimParams;
use crate::pigments::{MAX_SLOTS, Mixture};
use crate::stroke::{Dab, StrokeSample, Tool, pen_angles};

pub const TIP_SIZE: u32 = 512;
pub const TIP_MARGIN: u32 = 12;
pub const DELTA_SCALE: f32 = 65536.0;
pub const MAX_TIP_THICKNESS_UM: f32 = FULL_RESERVOIR_VOLUME_UM;

const TIP_HALF: u32 = TIP_SIZE / 2;
const MAX_TIP_RADIUS: f32 = ((TIP_SIZE - TIP_MARGIN) / 2) as f32;
const MIN_DAB_RADIUS: f32 = 0.5;
const REFERENCE_STEP_PX: f32 = DEFAULT_SPACING * DEFAULT_MAX_DIAMETER_PX;
const PIGMENT_CHANNELS: usize = MAX_SLOTS;
const VOLUME_CHANNEL: usize = PIGMENT_CHANNELS;
const TIP_CHANNELS: usize = PIGMENT_CHANNELS + 1;
const TIP_TEXELS: usize = (TIP_SIZE * TIP_SIZE) as usize;
const TIP_VALUES: usize = TIP_TEXELS * TIP_CHANNELS;
const TIP_BYTES: u64 = (TIP_VALUES * std::mem::size_of::<f32>()) as u64;
pub const FULL_DIP: f32 = 1.0;
const TIP_SUM_INVOCATIONS: u32 = 256;
const TIP_SUM_CHANNELS: usize = TIP_CHANNELS + 1;
const TIP_TEXEL_COUNT_SUM: usize = TIP_CHANNELS;
const TIP_SUMS_BYTES: u64 = (TIP_SUM_CHANNELS * std::mem::size_of::<f32>()) as u64;
const MIN_TIP_MEAN_VOLUME_UM: f32 = 1e-3;
const DAB_RECORD_STRIDE: u64 = 256;
const DAB_APRON_PX: i32 = 2;
const MAX_SWEEP_PX: f32 = 32.0;
const FLAT_THICKNESS_SHARE: f32 = 0.25;
const FAN_THICKNESS_SHARE: f32 = 0.12;
const FAN_BRISTLE_CONTRAST: f32 = 0.9;
const FAN_BRISTLE_COUNT_SHARE: f32 = 0.5;
const UPRIGHT_TILT_RAD: f32 = 5.0 * std::f32::consts::PI / 180.0;
const LEANING_TILT_RAD: f32 = 20.0 * std::f32::consts::PI / 180.0;
const UPRIGHT_ORIENTATION: f32 = std::f32::consts::FRAC_PI_2;
const CAPILLARY_SHARE: f32 = 0.2;
const CROSS_CAPILLARY_SHARE: f32 = 0.01;
const CAPILLARY_POOLS: u64 = TIP_SIZE as u64 + 1;
const CAPILLARY_BYTES: u64 = (3 * CAPILLARY_POOLS * TIP_CHANNELS as u64 + 2 * CAPILLARY_POOLS)
    * std::mem::size_of::<u32>() as u64;
const CAPILLARY_POOL_BUDGET: f32 = 4.0e9;
const CAPILLARY_POOL_GROUP_SIZE: u32 = 64;
const CAPILLARY_POOL_GROUPS: u32 = (CAPILLARY_POOLS as u32).div_ceil(CAPILLARY_POOL_GROUP_SIZE);
const SPREAD_APRON_PX: i32 = 1;
const TIP_APRON_TEXELS: f32 = 1.0;
const MIN_BRISTLE_PERIOD_PX: f32 = 3.0;
pub(crate) const WORKGROUP_SIZE: u32 = 8;
const KNIFE_MIN_HALF_WIDTH_PX: f32 = 1.5;
const KNIFE_HALF_WIDTH_RATIO: f32 = 0.12;
const KNIFE_DRAG: f32 = 0.15;
const KNIFE_LAID_ALTITUDE_RAD: f32 = std::f32::consts::FRAC_PI_4;
const KNIFE_UPRIGHT_ALTITUDE_RAD: f32 = 80.0 * std::f32::consts::PI / 180.0;
const KNIFE_DIP_LENGTH_PX: f32 = 380.0;
const KNIFE_LAID_CAPACITY_UM: f32 = MAX_TIP_THICKNESS_UM * KNIFE_DIP_LENGTH_PX;
const KNIFE_UPRIGHT_CAPACITY_UM: f32 = 200.0;
const KNIFE_SPILL_SHARE: f32 = 0.5;
const KNIFE_REST_FOLLOW: f32 = 0.25;
const CONTACT_REST_FOLLOW: f32 = 0.25;
const FIRST_REST_FOLLOW: f32 = 1.0;
const BLADE_SPILL_BAND_PX: f32 = 2.0;
const BLADE_COVERAGE_MARGIN_PX: f32 = 1.0;
const BLADE_DELTA_SCALE: f32 = 4096.0;
const BLADE_ENDS: usize = 2;
const BLADE_CELLS: usize = TIP_SIZE as usize;
const BLADE_DELTA_CHANNELS: usize = TIP_CHANNELS + 1;
const BLADE_SPILL_VALUES: usize = BLADE_ENDS * TIP_CHANNELS;
const BLADE_STATE_VALUES: usize = 2 + BLADE_ENDS + BLADE_SPILL_VALUES;
const BLADE_LOAD_VALUES: usize = BLADE_CELLS * TIP_CHANNELS;
const BLADE_LOAD_BYTES: u64 = (BLADE_LOAD_VALUES * std::mem::size_of::<f32>()) as u64;
const BLADE_DELTA_BYTES: u64 =
    (BLADE_CELLS * BLADE_DELTA_CHANNELS * std::mem::size_of::<i32>()) as u64;
const BLADE_STATE_BYTES: u64 = (BLADE_STATE_VALUES * std::mem::size_of::<f32>()) as u64;
const BLADE_CELL_GROUP_SIZE: u32 = 64;
const BLADE_CELL_GROUPS: u32 = (BLADE_CELLS as u32).div_ceil(BLADE_CELL_GROUP_SIZE);
const MAX_PICK_RADIUS_PX: f32 = 16.0;
const MIN_PICK_RADIUS_PX: f32 = 0.5;
const DRY_WORKGROUP_SIZE: u32 = 8;
const MIN_TOP_LAYER_UM: f32 = 0.01;
const PICK_INVOCATIONS: u32 = 256;
const DRY_WORKGROUPS_PER_TILE_SIDE: u32 = TILE_SIZE / DRY_WORKGROUP_SIZE;
const TIP_GROUPS: u32 = TIP_SIZE.div_ceil(WORKGROUP_SIZE);

const DAB_SNIPPETS: [&str; 3] = [
    include_str!("shaders/brush_tip.wgsl"),
    include_str!("shaders/nearest_half.wgsl"),
    include_str!("shaders/footprint.wgsl"),
];
const EYEDROPPER_SNIPPETS: [&str; 2] = [
    include_str!("shaders/brush_tip.wgsl"),
    include_str!("shaders/slot_lane.wgsl"),
];
const DRY_SNIPPETS: [&str; 4] = [
    include_str!("shaders/pigment_slots.wgsl"),
    include_str!("shaders/slot_lane.wgsl"),
    include_str!("shaders/kubelka_munk.wgsl"),
    include_str!("shaders/nearest_half.wgsl"),
];

const TOOL_MODE_BRUSH: u32 = 0;
const TOOL_MODE_KNIFE: u32 = 1;
const TOOL_MODE_BLENDER: u32 = 2;
const TOOL_MODE_RAG: u32 = 3;
const TOOL_MODE_EYEDROPPER: u32 = 4;

pub trait PaintEngine {
    fn begin_stroke(&mut self);
    fn encode_dabs(
        &mut self,
        gpu: &GpuContext,
        canvas: &mut Canvas,
        brush: &BrushSettings,
        dabs: &[Dab],
        encoder: &mut wgpu::CommandEncoder,
    );
    fn params(&self) -> &SimParams;
    fn params_mut(&mut self) -> &mut SimParams;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct DabRecord {
    center: [f32; 2],
    radius: f32,
    pressure: f32,
    altitude: f32,
    azimuth: f32,
    direction: f32,
    speed: f32,
    orientation: [f32; 2],
    elongation: f32,
    tool_mode: u32,
    canvas_origin: [i32; 2],
    canvas_size: [u32; 2],
    tip_origin: [i32; 2],
    tip_size: [u32; 2],
    transfer_fraction: f32,
    velocity_cutoff: f32,
    residual_um: f32,
    penetration_um: f32,
    bristle_contrast: f32,
    bristle_count: f32,
    refill_rate: f32,
    loaded_thickness_um: f32,
    advection: f32,
    squeeze: f32,
    diffusion: f32,
    blade_gap_um: f32,
    spread_origin: [i32; 2],
    spread_size: [u32; 2],
    rag_strength: f32,
    rest_follow: f32,
    blade_capacity_um: f32,
    blade_spill_share: f32,
    step_ratio: f32,
    sweep: [f32; 2],
    capillary_rate: f32,
    cross_capillary_rate: f32,
    capillary_scale: f32,
    _pad: [f32; 2],
}

const DAB_RECORD_SIZE: u64 = std::mem::size_of::<DabRecord>() as u64;
const _: () = assert!(DAB_RECORD_SIZE == 192 && DAB_RECORD_SIZE <= DAB_RECORD_STRIDE);

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct BodyState {
    pigment: [f32; PIGMENT_CHANNELS],
    volume: f32,
    granted_ratio: f32,
    granted_fractions: [f32; PIGMENT_CHANNELS],
}

const BODY_BYTES: u64 = std::mem::size_of::<BodyState>() as u64;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct DipRecord {
    fractions: [f32; PIGMENT_CHANNELS],
    tip_radius: f32,
    tip_add_um: f32,
    tip_limit_um: f32,
    body_add_um: f32,
    body_limit_um: f32,
    _pad: [f32; 3],
}

impl DipRecord {
    fn measuring(tip_radius: f32) -> DipRecord {
        DipRecord {
            tip_radius,
            ..bytemuck::Zeroable::zeroed()
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct PickRecord {
    center: [f32; 2],
    radius: f32,
    _pad: f32,
}
const WANTED_BYTES: u64 = 2 * std::mem::size_of::<u32>() as u64;
const CONTACT_REFERENCE_BYTES: u64 = std::mem::size_of::<f32>() as u64;
const PICKED_PAINT_FOUND: usize = 0;
const PICKED_FRACTIONS: usize = 1;
const PICKED_BYTES: u64 =
    ((PICKED_FRACTIONS + PIGMENT_CHANNELS) * std::mem::size_of::<f32>()) as u64;

#[derive(Clone, Copy, Debug)]
struct Placement {
    record: DabRecord,
    canvas_rect: [f32; 4],
    canvas_groups: [u32; 2],
    tip_groups: [u32; 2],
    spread_groups: [u32; 2],
    spreads: bool,
    refills: bool,
    wipes: bool,
    blades: bool,
}

struct ScratchSurface {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

struct Scratch {
    size: [u32; 2],
    pig0: ScratchSurface,
    pig1: ScratchSurface,
    vol: ScratchSurface,
    motion: ScratchSurface,
}

impl Scratch {
    fn new(gpu: &GpuContext, size: [u32; 2]) -> Scratch {
        Scratch {
            size,
            pig0: scratch_surface(gpu, "scratch_pig0", size, wgpu::TextureFormat::Rgba16Float),
            pig1: scratch_surface(gpu, "scratch_pig1", size, wgpu::TextureFormat::Rgba16Float),
            vol: scratch_surface(gpu, "scratch_vol", size, wgpu::TextureFormat::R32Float),
            motion: scratch_surface(
                gpu,
                "scratch_motion",
                size,
                wgpu::TextureFormat::Rgba32Float,
            ),
        }
    }
}

fn scratch_surface(
    gpu: &GpuContext,
    label: &str,
    [width, height]: [u32; 2],
    format: wgpu::TextureFormat,
) -> ScratchSurface {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::STORAGE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    ScratchSurface {
        _texture: texture,
        view,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrushOutline {
    pub semi_axes: [f32; 2],
    pub orientation: f32,
}

struct Footprint {
    radius: f32,
    elongation: f32,
    orientation: f32,
    rectangular: bool,
    lateral_reach: f32,
}

struct Spreading {
    advection: f32,
    squeeze: f32,
    diffusion: f32,
}

impl Spreading {
    fn of(tool: Tool, params: &SimParams) -> Spreading {
        let advection = params.get("advection");
        let squeeze = params.get("squeeze");
        match tool {
            Tool::Knife => Spreading {
                advection: KNIFE_DRAG,
                squeeze: 0.0,
                diffusion: 0.0,
            },
            Tool::Blender => Spreading {
                advection,
                squeeze,
                diffusion: params.get("blend_diffusion"),
            },
            _ => Spreading {
                advection,
                squeeze,
                diffusion: 0.0,
            },
        }
    }
}

fn spreads(tool: Tool) -> bool {
    matches!(tool, Tool::Brush | Tool::Knife | Tool::Blender)
}

fn refills(tool: Tool) -> bool {
    matches!(tool, Tool::Brush)
}

fn wipes(tool: Tool) -> bool {
    tool == Tool::Rag
}

fn blades(tool: Tool) -> bool {
    tool == Tool::Knife
}

fn rest_follow(tool: Tool) -> f32 {
    if blades(tool) {
        KNIFE_REST_FOLLOW
    } else {
        CONTACT_REST_FOLLOW
    }
}

fn upright_share(altitude: f32) -> f32 {
    let t = ((altitude - KNIFE_LAID_ALTITUDE_RAD)
        / (KNIFE_UPRIGHT_ALTITUDE_RAD - KNIFE_LAID_ALTITUDE_RAD))
        .clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn blade_gap_um(params: &SimParams, pressure: f32, altitude: f32) -> f32 {
    let residual = params.get("residual_um");
    let lift = (1.0 - pressure.clamp(0.0, 1.0)) * (1.0 - upright_share(altitude));
    residual + (params.get("knife_gap_um") - residual).max(0.0) * lift
}

fn blade_capacity_um(altitude: f32) -> f32 {
    KNIFE_LAID_CAPACITY_UM
        + (KNIFE_UPRIGHT_CAPACITY_UM - KNIFE_LAID_CAPACITY_UM) * upright_share(altitude)
}

fn shape_thickness(shape: BrushShape) -> f32 {
    match shape {
        BrushShape::Round => 1.0,
        BrushShape::Flat => FLAT_THICKNESS_SHARE,
        BrushShape::Fan => FAN_THICKNESS_SHARE,
    }
}

fn blade_orientation(shape: BrushShape, altitude: f32, azimuth: f32) -> f32 {
    if shape == BrushShape::Round {
        return azimuth;
    }
    let tilt = std::f32::consts::FRAC_PI_2 - altitude;
    let lean = smoothstep(UPRIGHT_TILT_RAD, LEANING_TILT_RAD, tilt);
    let doubled = [
        lean * (2.0 * azimuth).cos() + (1.0 - lean) * (2.0 * UPRIGHT_ORIENTATION).cos(),
        lean * (2.0 * azimuth).sin() + (1.0 - lean) * (2.0 * UPRIGHT_ORIENTATION).sin(),
    ];
    if doubled[0].hypot(doubled[1]) <= f32::EPSILON {
        return UPRIGHT_ORIENTATION;
    }
    doubled[1].atan2(doubled[0]) / 2.0
}

fn smoothstep(from: f32, to: f32, value: f32) -> f32 {
    let t = ((value - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn bristle_spread(shape: BrushShape, params: &SimParams) -> (f32, f32) {
    let contrast = params.get("bristle_contrast");
    let count = params.get("bristle_count");
    match shape {
        BrushShape::Fan => (
            contrast.max(FAN_BRISTLE_CONTRAST),
            count * FAN_BRISTLE_COUNT_SHARE,
        ),
        _ => (contrast, count),
    }
}

fn knife_half_width(radius: f32) -> f32 {
    (KNIFE_HALF_WIDTH_RATIO * radius).max(KNIFE_MIN_HALF_WIDTH_PX)
}

fn blade_cell_cover(offset: i32, half_length: f32) -> f32 {
    let centre = offset as f32 + 0.5;
    (half_length - centre.abs() + 0.5).clamp(0.0, 1.0)
}

fn heading_known(dab: &Dab) -> bool {
    dab.speed > 0.0
}

fn is_finite(dab: &Dab) -> bool {
    [
        dab.x,
        dab.y,
        dab.radius,
        dab.pressure,
        dab.altitude,
        dab.azimuth,
        dab.direction,
        dab.speed,
    ]
    .iter()
    .all(|value| value.is_finite())
}

impl Footprint {
    fn of(dab: &Dab, brush: &BrushSettings, params: &SimParams) -> Footprint {
        if brush.tool == Tool::Knife {
            let radius =
                (brush.geometry.max_diameter_px / 2.0).clamp(MIN_DAB_RADIUS, MAX_TIP_RADIUS);
            return Footprint {
                radius,
                elongation: knife_half_width(radius) / radius,
                orientation: dab.direction,
                rectangular: true,
                lateral_reach: BLADE_SPILL_BAND_PX + BLADE_COVERAGE_MARGIN_PX,
            };
        }
        let radius = dab.radius.clamp(MIN_DAB_RADIUS, MAX_TIP_RADIUS);
        let tilt = 1.0 - dab.altitude.sin().clamp(0.0, 1.0);
        let elongation = (shape_thickness(brush.shape)
            * (1.0 + params.get("tilt_elongation") * tilt))
            .min(MAX_TIP_RADIUS / radius);
        let orientation = blade_orientation(brush.shape, dab.altitude, dab.azimuth);
        Footprint {
            radius,
            elongation,
            orientation,
            rectangular: false,
            lateral_reach: 0.0,
        }
    }

    fn semi_axes(&self) -> [f32; 2] {
        [self.radius * self.elongation, self.radius]
    }

    fn canvas_half_extents(&self) -> [f32; 2] {
        let [along, across] = self.semi_axes();
        let (sin, cos) = self.orientation.sin_cos();
        if self.rectangular {
            let along = along + BLADE_COVERAGE_MARGIN_PX;
            let across = across + self.lateral_reach;
            return [
                along * cos.abs() + across * sin.abs(),
                along * sin.abs() + across * cos.abs(),
            ];
        }
        [
            (along * along * cos * cos + across * across * sin * sin).sqrt(),
            (along * along * sin * sin + across * across * cos * cos).sqrt(),
        ]
    }
}

fn tool_mode(tool: Tool) -> u32 {
    match tool {
        Tool::Brush => TOOL_MODE_BRUSH,
        Tool::Knife => TOOL_MODE_KNIFE,
        Tool::Blender => TOOL_MODE_BLENDER,
        Tool::Rag => TOOL_MODE_RAG,
        Tool::Eyedropper => TOOL_MODE_EYEDROPPER,
    }
}

fn rate_over_step(reference_rate: f32, step_ratio: f32) -> f32 {
    1.0 - (1.0 - reference_rate.clamp(0.0, 1.0)).powf(step_ratio)
}

fn capillary_scale(tip_size: [u32; 2]) -> f32 {
    let texels = (tip_size[0] * tip_size[1]).max(1) as f32;
    let fitting = CAPILLARY_POOL_BUDGET / (texels * MAX_TIP_THICKNESS_UM);
    fitting.min(DELTA_SCALE).log2().floor().exp2()
}

fn tip_window(semi_axes: [f32; 2]) -> ([i32; 2], [u32; 2]) {
    let reach = semi_axes.map(|semi| (semi + TIP_APRON_TEXELS).ceil() as u32);
    let origin = reach.map(|r| (TIP_HALF - r) as i32);
    let size = reach.map(|r| 2 * r + 1);
    (origin, size)
}

fn place(
    dab: &Dab,
    step_px: f32,
    sweep: [f32; 2],
    brush: &BrushSettings,
    params: &SimParams,
    loaded_thickness_um: f32,
    canvas: &Canvas,
) -> Option<Placement> {
    if brush.tool == Tool::Eyedropper || !is_finite(dab) {
        return None;
    }
    if brush.tool == Tool::Knife && !heading_known(dab) {
        return None;
    }
    let footprint = Footprint::of(dab, brush, params);
    let (bristle_contrast, bristle_count) = bristle_spread(brush.shape, params);
    let step_ratio = step_px / REFERENCE_STEP_PX;
    let [half_x, half_y] = footprint.canvas_half_extents();
    let swept_from = [dab.x + sweep[0], dab.y + sweep[1]];
    let first = [
        (dab.x.min(swept_from[0]) - half_x).floor() as i32,
        (dab.y.min(swept_from[1]) - half_y).floor() as i32,
    ];
    let last = [
        (dab.x.max(swept_from[0]) + half_x).floor() as i32,
        (dab.y.max(swept_from[1]) + half_y).floor() as i32,
    ];
    let limits = [canvas.spec.width as i32, canvas.spec.height as i32];
    let (canvas_origin, canvas_size) = clamped_region(first, last, DAB_APRON_PX, limits)?;
    let (spread_origin, spread_size) = clamped_region(first, last, SPREAD_APRON_PX, limits)?;
    let [x0, y0] = canvas_origin.map(|origin| origin as f32);
    let [x1, y1] = [0, 1].map(|axis| (canvas_origin[axis] + canvas_size[axis] as i32) as f32);
    let (tip_origin, tip_size) = tip_window(footprint.semi_axes());
    let (sin, cos) = footprint.orientation.sin_cos();
    let spreading = Spreading::of(brush.tool, params);
    let record = DabRecord {
        center: [dab.x, dab.y],
        radius: footprint.radius,
        pressure: dab.pressure,
        altitude: dab.altitude,
        azimuth: dab.azimuth,
        direction: dab.direction,
        speed: dab.speed,
        orientation: [cos, sin],
        elongation: footprint.elongation,
        tool_mode: tool_mode(brush.tool),
        canvas_origin,
        canvas_size,
        tip_origin,
        tip_size,
        transfer_fraction: params.get("transfer_fraction"),
        velocity_cutoff: params.get("velocity_cutoff"),
        residual_um: params.get("residual_um"),
        penetration_um: params.get("penetration_um"),
        bristle_contrast: if wipes(brush.tool) {
            0.0
        } else {
            bristle_contrast
        },
        bristle_count: bristle_count.min(2.0 * footprint.radius / MIN_BRISTLE_PERIOD_PX),
        refill_rate: rate_over_step(params.get("refill_rate"), step_ratio),
        loaded_thickness_um,
        advection: spreading.advection,
        squeeze: spreading.squeeze,
        diffusion: spreading.diffusion,
        blade_gap_um: blade_gap_um(params, dab.pressure, dab.altitude),
        spread_origin,
        spread_size,
        rag_strength: params.get("rag_strength"),
        rest_follow: rest_follow(brush.tool),
        blade_capacity_um: blade_capacity_um(dab.altitude),
        blade_spill_share: KNIFE_SPILL_SHARE,
        step_ratio,
        sweep,
        capillary_rate: rate_over_step(CAPILLARY_SHARE, step_ratio),
        cross_capillary_rate: rate_over_step(CROSS_CAPILLARY_SHARE, step_ratio),
        capillary_scale: capillary_scale(tip_size),
        _pad: [0.0; 2],
    };
    Some(Placement {
        record,
        canvas_rect: [x0, y0, x1 - 1.0, y1 - 1.0],
        canvas_groups: canvas_size.map(|s| s.div_ceil(WORKGROUP_SIZE)),
        tip_groups: tip_size.map(|s| s.div_ceil(WORKGROUP_SIZE)),
        spread_groups: spread_size.map(|s| s.div_ceil(WORKGROUP_SIZE)),
        spreads: spreads(brush.tool),
        refills: refills(brush.tool),
        wipes: wipes(brush.tool),
        blades: blades(brush.tool),
    })
}

fn max_dab_region_side() -> u32 {
    let along = knife_half_width(MAX_TIP_RADIUS) + BLADE_COVERAGE_MARGIN_PX;
    let across = MAX_TIP_RADIUS + BLADE_SPILL_BAND_PX + BLADE_COVERAGE_MARGIN_PX;
    let half_extent = across.hypot(along).ceil() as u32;
    2 * (half_extent + 1 + DAB_APRON_PX as u32) + 2 * MAX_SWEEP_PX as u32
}

pub(crate) fn clamped_region(
    first: [i32; 2],
    last: [i32; 2],
    apron: i32,
    limits: [i32; 2],
) -> Option<([i32; 2], [u32; 2])> {
    let low = [0, 1].map(|axis| first[axis].saturating_sub(apron).max(0));
    let high = [0, 1].map(|axis| last[axis].saturating_add(apron).min(limits[axis] - 1));
    if (0..2).any(|axis| high[axis] < low[axis]) {
        return None;
    }
    Some((low, [0, 1].map(|axis| (high[axis] - low[axis] + 1) as u32)))
}

fn shader_source(snippets: &[&str], body: &str) -> String {
    let mut source = format!(
        "const TIP_SIZE: u32 = {TIP_SIZE}u;\n\
         const TIP_HALF: i32 = {TIP_HALF};\n\
         const TIP_CHANNELS: u32 = {TIP_CHANNELS}u;\n\
         const PIGMENT_CHANNELS: u32 = {PIGMENT_CHANNELS}u;\n\
         const VOLUME_CHANNEL: u32 = {VOLUME_CHANNEL}u;\n\
         const DELTA_SCALE: f32 = {DELTA_SCALE:?};\n\
         const TOOL_MODE_KNIFE: u32 = {TOOL_MODE_KNIFE}u;\n\
         const TILE_SIZE: u32 = {TILE_SIZE}u;\n\
         const DRY_WORKGROUP_SIZE: u32 = {DRY_WORKGROUP_SIZE}u;\n\
         const PICK_INVOCATIONS: u32 = {PICK_INVOCATIONS}u;\n\
         const TIP_SUM_INVOCATIONS: u32 = {TIP_SUM_INVOCATIONS}u;\n\
         const MIN_TOP_LAYER_UM: f32 = {MIN_TOP_LAYER_UM:?};\n\
         const BLADE_DELTA_SCALE: f32 = {BLADE_DELTA_SCALE:?};\n\
         const BLADE_DELTA_CHANNELS: u32 = {BLADE_DELTA_CHANNELS}u;\n\
         const BLADE_SPILL_VALUES: u32 = {BLADE_SPILL_VALUES}u;\n\
         const BLADE_SPILL_BAND_PX: f32 = {BLADE_SPILL_BAND_PX:?};\n\
         const BLADE_CELL_GROUP_SIZE: u32 = {BLADE_CELL_GROUP_SIZE}u;\n"
    );
    for snippet in snippets {
        source.push_str(snippet);
        source.push('\n');
    }
    source.push_str(body);
    source
}

pub(crate) fn compute_pipeline(
    gpu: &GpuContext,
    label: &str,
    module: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    entry_point: &str,
) -> wgpu::ComputePipeline {
    gpu.device
        .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(label),
            layout: Some(layout),
            module,
            entry_point: Some(entry_point),
            compilation_options: Default::default(),
            cache: None,
        })
}

pub(crate) fn shader_module(
    gpu: &GpuContext,
    label: &str,
    snippets: &[&str],
    body: &str,
) -> wgpu::ShaderModule {
    gpu.device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::ShaderSource::Wgsl(shader_source(snippets, body).into()),
        })
}

pub(crate) fn uniform_entry(binding: u32, dynamic_size: Option<u64>) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: dynamic_size.is_some(),
            min_binding_size: dynamic_size.and_then(wgpu::BufferSize::new),
        },
        count: None,
    }
}

pub(crate) fn storage_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

pub(crate) fn texture_entry(
    binding: u32,
    access: wgpu::StorageTextureAccess,
    format: wgpu::TextureFormat,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::StorageTexture {
            access,
            format,
            view_dimension: wgpu::TextureViewDimension::D2,
        },
        count: None,
    }
}

pub(crate) fn layouts(
    gpu: &GpuContext,
    label: &str,
    entries: &[wgpu::BindGroupLayoutEntry],
) -> (wgpu::BindGroupLayout, wgpu::PipelineLayout) {
    let bind_group_layout = gpu
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(label),
            entries,
        });
    let pipeline_layout = gpu
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(label),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
    (bind_group_layout, pipeline_layout)
}

pub(crate) fn storage_buffer(gpu: &GpuContext, label: &str, size: u64) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

pub(crate) fn read_buffer(gpu: &GpuContext, buffer: &wgpu::Buffer) -> Result<Vec<f32>, GpuLost> {
    let staging = staging_buffer(gpu, "brush_readback", buffer.size());
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("brush_readback"),
        });
    encoder.copy_buffer_to_buffer(buffer, 0, &staging, 0, buffer.size());
    gpu.queue.submit([encoder.finish()]);
    read_mapped(gpu, &staging, |bytes| bytemuck::cast_slice(bytes).to_vec())
}

struct BladePipelines {
    rest: wgpu::ComputePipeline,
    want: wgpu::ComputePipeline,
    grant: wgpu::ComputePipeline,
    lay: wgpu::ComputePipeline,
    settle: wgpu::ComputePipeline,
    spill: wgpu::ComputePipeline,
}

impl BladePipelines {
    fn new(gpu: &GpuContext, layout: &wgpu::PipelineLayout) -> BladePipelines {
        let module = shader_module(
            gpu,
            "blade",
            &DAB_SNIPPETS,
            include_str!("shaders/blade.wgsl"),
        );
        let pipeline =
            |entry: &str| compute_pipeline(gpu, &format!("blade_{entry}"), &module, layout, entry);
        BladePipelines {
            rest: pipeline("rest"),
            want: pipeline("want"),
            grant: pipeline("grant"),
            lay: pipeline("lay"),
            settle: pipeline("settle"),
            spill: pipeline("spill"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrushReadout {
    pub tip_mean: Mixture,
    pub reserve: PaintTotals,
    pub knife: PaintTotals,
}

struct CapillaryPipelines {
    gather: wgpu::ComputePipeline,
    settle: wgpu::ComputePipeline,
    distribute: wgpu::ComputePipeline,
}

pub struct WetOilEngine {
    params: SimParams,
    tip_radius: f32,
    loaded_thickness_um: f32,
    tip: wgpu::Buffer,
    tip_delta: wgpu::Buffer,
    body: wgpu::Buffer,
    wanted: wgpu::Buffer,
    capillary_rows: wgpu::Buffer,
    contact_reference: wgpu::Buffer,
    picked: wgpu::Buffer,
    tip_sums: wgpu::Buffer,
    blade_load: wgpu::Buffer,
    blade_delta: wgpu::Buffer,
    blade_grant: wgpu::Buffer,
    blade_state: wgpu::Buffer,
    rest_pending: bool,
    previous_dab: Option<[f32; 2]>,
    blade_layout: wgpu::BindGroupLayout,
    blade_pipelines: BladePipelines,
    exchange_layout: wgpu::BindGroupLayout,
    apply_layout: wgpu::BindGroupLayout,
    refill_layout: wgpu::BindGroupLayout,
    copy_layout: wgpu::BindGroupLayout,
    advect_layout: wgpu::BindGroupLayout,
    rag_layout: wgpu::BindGroupLayout,
    pick_layout: wgpu::BindGroupLayout,
    dry_layout: wgpu::BindGroupLayout,
    dip_layout: wgpu::BindGroupLayout,
    scratch: Option<Scratch>,
    reference_pipeline: wgpu::ComputePipeline,
    exchange_pipeline: wgpu::ComputePipeline,
    apply_pipeline: wgpu::ComputePipeline,
    gather_pipeline: wgpu::ComputePipeline,
    settle_pipeline: wgpu::ComputePipeline,
    distribute_pipeline: wgpu::ComputePipeline,
    capillary_pipelines: CapillaryPipelines,
    copy_pipeline: wgpu::ComputePipeline,
    advect_pipeline: wgpu::ComputePipeline,
    rag_pipeline: wgpu::ComputePipeline,
    pick_pipeline: wgpu::ComputePipeline,
    dry_pipeline: wgpu::ComputePipeline,
    fill_body_pipeline: wgpu::ComputePipeline,
    fill_tip_pipeline: wgpu::ComputePipeline,
    sum_tip_pipeline: wgpu::ComputePipeline,
}

impl WetOilEngine {
    pub fn new(gpu: &GpuContext) -> Self {
        use wgpu::StorageTextureAccess::{ReadOnly, ReadWrite, WriteOnly};
        use wgpu::TextureFormat::{R32Float, Rgba16Float, Rgba32Float};

        let (exchange_layout, exchange_pipeline_layout) = layouts(
            gpu,
            "dab_exchange",
            &[
                uniform_entry(0, Some(DAB_RECORD_SIZE)),
                texture_entry(1, ReadWrite, Rgba16Float),
                texture_entry(2, ReadWrite, Rgba16Float),
                texture_entry(3, ReadWrite, R32Float),
                texture_entry(4, ReadOnly, R32Float),
                storage_entry(5, true),
                storage_entry(6, false),
                storage_entry(7, false),
            ],
        );
        let (apply_layout, apply_pipeline_layout) = layouts(
            gpu,
            "tip_apply",
            &[
                uniform_entry(0, Some(DAB_RECORD_SIZE)),
                storage_entry(1, false),
                storage_entry(2, false),
            ],
        );
        let (refill_layout, refill_pipeline_layout) = layouts(
            gpu,
            "tip_refill",
            &[
                uniform_entry(0, Some(DAB_RECORD_SIZE)),
                storage_entry(1, false),
                storage_entry(2, false),
                storage_entry(3, false),
                storage_entry(4, false),
                storage_entry(5, false),
            ],
        );
        let spreading_entries = |motion_access| {
            [
                uniform_entry(0, Some(DAB_RECORD_SIZE)),
                texture_entry(1, ReadOnly, Rgba16Float),
                texture_entry(2, ReadOnly, Rgba16Float),
                texture_entry(3, ReadOnly, R32Float),
                texture_entry(4, WriteOnly, Rgba16Float),
                texture_entry(5, WriteOnly, Rgba16Float),
                texture_entry(6, WriteOnly, R32Float),
                texture_entry(7, motion_access, Rgba32Float),
            ]
        };
        let (copy_layout, copy_pipeline_layout) =
            layouts(gpu, "region_copy", &spreading_entries(WriteOnly));
        let (advect_layout, advect_pipeline_layout) =
            layouts(gpu, "advect", &spreading_entries(ReadOnly));

        let (rag_layout, rag_pipeline_layout) = layouts(
            gpu,
            "rag",
            &[
                uniform_entry(0, Some(DAB_RECORD_SIZE)),
                texture_entry(1, ReadWrite, R32Float),
            ],
        );
        let (pick_layout, pick_pipeline_layout) = layouts(
            gpu,
            "eyedropper",
            &[
                uniform_entry(0, None),
                texture_entry(1, ReadOnly, Rgba16Float),
                texture_entry(2, ReadOnly, Rgba16Float),
                texture_entry(3, ReadOnly, R32Float),
                texture_entry(4, ReadOnly, Rgba16Float),
                texture_entry(5, ReadOnly, Rgba16Float),
                storage_entry(6, false),
            ],
        );
        let (dry_layout, dry_pipeline_layout) = layouts(
            gpu,
            "dry",
            &[
                uniform_entry(0, None),
                storage_entry(1, true),
                texture_entry(2, ReadWrite, Rgba16Float),
                texture_entry(3, ReadWrite, Rgba16Float),
                texture_entry(4, ReadWrite, R32Float),
                texture_entry(5, ReadWrite, Rgba16Float),
                texture_entry(6, ReadWrite, Rgba16Float),
                texture_entry(7, ReadWrite, R32Float),
                texture_entry(8, WriteOnly, Rgba16Float),
                texture_entry(9, WriteOnly, Rgba16Float),
            ],
        );

        let (blade_layout, blade_pipeline_layout) = layouts(
            gpu,
            "blade",
            &[
                uniform_entry(0, Some(DAB_RECORD_SIZE)),
                texture_entry(1, ReadWrite, Rgba16Float),
                texture_entry(2, ReadWrite, Rgba16Float),
                texture_entry(3, ReadWrite, R32Float),
                texture_entry(4, ReadOnly, R32Float),
                storage_entry(5, false),
                storage_entry(6, false),
                storage_entry(7, false),
                storage_entry(8, false),
            ],
        );

        let (dip_layout, dip_pipeline_layout) = layouts(
            gpu,
            "brush_dip",
            &[
                uniform_entry(0, None),
                storage_entry(1, false),
                storage_entry(2, false),
                storage_entry(3, false),
            ],
        );

        let dab_module = |label, body| shader_module(gpu, label, &DAB_SNIPPETS, body);
        let exchange = dab_module("dab_exchange", include_str!("shaders/dab_exchange.wgsl"));
        let apply = dab_module("tip_apply", include_str!("shaders/tip_apply.wgsl"));
        let refill = dab_module("tip_refill", include_str!("shaders/tip_refill.wgsl"));
        let capillary = dab_module("tip_capillary", include_str!("shaders/tip_capillary.wgsl"));
        let copy = dab_module("region_copy", include_str!("shaders/region_copy.wgsl"));
        let advect = dab_module("advect", include_str!("shaders/advect.wgsl"));
        let rag = dab_module("rag", include_str!("shaders/rag.wgsl"));
        let eyedropper = shader_module(
            gpu,
            "eyedropper",
            &EYEDROPPER_SNIPPETS,
            include_str!("shaders/eyedropper.wgsl"),
        );
        let dry = shader_module(gpu, "dry", &DRY_SNIPPETS, include_str!("shaders/dry.wgsl"));
        let dip = shader_module(
            gpu,
            "brush_dip",
            &EYEDROPPER_SNIPPETS,
            include_str!("shaders/brush_dip.wgsl"),
        );

        WetOilEngine {
            params: SimParams::default(),
            tip_radius: DEFAULT_MAX_DIAMETER_PX / 2.0,
            loaded_thickness_um: 0.0,
            tip: storage_buffer(gpu, "brush_tip", TIP_BYTES),
            tip_delta: storage_buffer(gpu, "brush_tip_delta", TIP_BYTES),
            body: storage_buffer(gpu, "brush_body", BODY_BYTES),
            wanted: storage_buffer(gpu, "brush_refill_wanted", WANTED_BYTES),
            capillary_rows: storage_buffer(gpu, "brush_capillary_rows", CAPILLARY_BYTES),
            contact_reference: storage_buffer(
                gpu,
                "dab_contact_reference",
                CONTACT_REFERENCE_BYTES,
            ),
            picked: storage_buffer(gpu, "eyedropper_picked", PICKED_BYTES),
            tip_sums: storage_buffer(gpu, "brush_tip_sums", TIP_SUMS_BYTES),
            blade_load: storage_buffer(gpu, "knife_blade_load", BLADE_LOAD_BYTES),
            blade_delta: storage_buffer(gpu, "knife_blade_delta", BLADE_DELTA_BYTES),
            blade_grant: storage_buffer(gpu, "knife_blade_grant", BLADE_LOAD_BYTES),
            blade_state: storage_buffer(gpu, "knife_blade_state", BLADE_STATE_BYTES),
            rest_pending: true,
            previous_dab: None,
            blade_pipelines: BladePipelines::new(gpu, &blade_pipeline_layout),
            blade_layout,
            reference_pipeline: compute_pipeline(
                gpu,
                "dab_reference",
                &exchange,
                &exchange_pipeline_layout,
                "reference",
            ),
            exchange_pipeline: compute_pipeline(
                gpu,
                "dab_exchange",
                &exchange,
                &exchange_pipeline_layout,
                "main",
            ),
            apply_pipeline: compute_pipeline(
                gpu,
                "tip_apply",
                &apply,
                &apply_pipeline_layout,
                "main",
            ),
            gather_pipeline: compute_pipeline(
                gpu,
                "tip_refill_gather",
                &refill,
                &refill_pipeline_layout,
                "gather",
            ),
            settle_pipeline: compute_pipeline(
                gpu,
                "tip_refill_settle",
                &refill,
                &refill_pipeline_layout,
                "settle",
            ),
            distribute_pipeline: compute_pipeline(
                gpu,
                "tip_refill_distribute",
                &refill,
                &refill_pipeline_layout,
                "distribute",
            ),
            capillary_pipelines: CapillaryPipelines {
                gather: compute_pipeline(
                    gpu,
                    "tip_capillary_gather",
                    &capillary,
                    &refill_pipeline_layout,
                    "gather",
                ),
                settle: compute_pipeline(
                    gpu,
                    "tip_capillary_settle",
                    &capillary,
                    &refill_pipeline_layout,
                    "settle",
                ),
                distribute: compute_pipeline(
                    gpu,
                    "tip_capillary_distribute",
                    &capillary,
                    &refill_pipeline_layout,
                    "distribute",
                ),
            },
            copy_pipeline: compute_pipeline(
                gpu,
                "region_copy",
                &copy,
                &copy_pipeline_layout,
                "main",
            ),
            advect_pipeline: compute_pipeline(
                gpu,
                "advect",
                &advect,
                &advect_pipeline_layout,
                "main",
            ),
            rag_pipeline: compute_pipeline(gpu, "rag", &rag, &rag_pipeline_layout, "main"),
            pick_pipeline: compute_pipeline(
                gpu,
                "eyedropper",
                &eyedropper,
                &pick_pipeline_layout,
                "sample",
            ),
            dry_pipeline: compute_pipeline(gpu, "dry", &dry, &dry_pipeline_layout, "main"),
            fill_body_pipeline: compute_pipeline(
                gpu,
                "brush_fill_body",
                &dip,
                &dip_pipeline_layout,
                "fill_body",
            ),
            fill_tip_pipeline: compute_pipeline(
                gpu,
                "brush_fill_tip",
                &dip,
                &dip_pipeline_layout,
                "fill_tip",
            ),
            sum_tip_pipeline: compute_pipeline(
                gpu,
                "brush_sum_tip",
                &dip,
                &dip_pipeline_layout,
                "sum_tip",
            ),
            exchange_layout,
            apply_layout,
            refill_layout,
            copy_layout,
            advect_layout,
            rag_layout,
            pick_layout,
            dry_layout,
            dip_layout,
            scratch: None,
        }
    }

    fn step_since_previous_dab(&mut self, dab: &Dab) -> (f32, [f32; 2]) {
        if !is_finite(dab) {
            return (REFERENCE_STEP_PX, [0.0; 2]);
        }
        let back = self.previous_dab.map(|[x, y]| [x - dab.x, y - dab.y]);
        self.previous_dab = Some([dab.x, dab.y]);
        let Some(back) = back else {
            return (REFERENCE_STEP_PX, [0.0; 2]);
        };
        let step = back[0].hypot(back[1]);
        let sweep = if step <= MAX_SWEEP_PX { back } else { [0.0; 2] };
        (step, sweep)
    }

    pub fn resize_tip(&mut self, radius_px: f32) {
        if radius_px.is_finite() {
            self.tip_radius = radius_px.clamp(MIN_DAB_RADIUS, MAX_TIP_RADIUS);
        }
    }

    pub fn load_reservoir(&mut self, gpu: &GpuContext, mixture: &Mixture) {
        self.clean_brush(gpu);
        self.dip(gpu, mixture, FULL_DIP);
    }

    pub fn dip(&mut self, gpu: &GpuContext, mixture: &Mixture, amount: f32) {
        let Some(thickness) = dip_thickness(mixture) else {
            return;
        };
        if !amount.is_finite() || amount <= 0.0 {
            return;
        }
        let record = DipRecord {
            fractions: dip_fractions(mixture),
            tip_radius: self.tip_radius,
            tip_add_um: amount * thickness,
            tip_limit_um: MAX_TIP_THICKNESS_UM,
            body_add_um: amount * self.reserve_volume(thickness),
            body_limit_um: self.reserve_volume(MAX_TIP_THICKNESS_UM),
            _pad: [0.0; 3],
        };
        self.submit_fill(gpu, &record, true);
        self.loaded_thickness_um = thickness;
    }

    pub fn restore_reserve(&mut self, gpu: &GpuContext, mixture: &Mixture) {
        let Some(thickness) = dip_thickness(mixture) else {
            return;
        };
        let full_reserve = self.reserve_volume(thickness);
        let record = DipRecord {
            fractions: dip_fractions(mixture),
            tip_radius: self.tip_radius,
            tip_add_um: 0.0,
            tip_limit_um: 0.0,
            body_add_um: full_reserve,
            body_limit_um: full_reserve,
            _pad: [0.0; 3],
        };
        self.submit_fill(gpu, &record, false);
        self.loaded_thickness_um = thickness;
    }

    pub fn tip_mean_mixture(&self, gpu: &GpuContext) -> Result<Mixture, GpuLost> {
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("brush_sum_tip"),
            });
        self.encode_sum_tip(gpu, &mut encoder);
        gpu.queue.submit([encoder.finish()]);
        Ok(tip_mean_from_sums(&read_buffer(gpu, &self.tip_sums)?))
    }

    pub fn reserve_totals(&self, gpu: &GpuContext) -> Result<PaintTotals, GpuLost> {
        Ok(reserve_from_body(&read_buffer(gpu, &self.body)?))
    }

    pub fn read_brush(&self, gpu: &GpuContext) -> Result<BrushReadout, GpuLost> {
        let sources = [&self.tip_sums, &self.body, &self.blade_load];
        let staging = staging_buffer(
            gpu,
            "brush_readout",
            sources.iter().map(|source| source.size()).sum(),
        );
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("brush_readout"),
            });
        self.encode_sum_tip(gpu, &mut encoder);
        let mut offset = 0;
        for source in sources {
            encoder.copy_buffer_to_buffer(source, 0, &staging, offset, source.size());
            offset += source.size();
        }
        gpu.queue.submit([encoder.finish()]);
        read_mapped(gpu, &staging, |bytes| {
            let values: &[f32] = bytemuck::cast_slice(bytes);
            let (sums, rest) = values.split_at(TIP_SUM_CHANNELS);
            let (body, load) = rest.split_at(BODY_BYTES as usize / std::mem::size_of::<f32>());
            BrushReadout {
                tip_mean: tip_mean_from_sums(sums),
                reserve: reserve_from_body(body),
                knife: knife_from_load(load),
            }
        })
    }

    fn encode_sum_tip(&self, gpu: &GpuContext, encoder: &mut wgpu::CommandEncoder) {
        let group = self.dip_bind_group(gpu, &DipRecord::measuring(self.tip_radius));
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("brush_sum_tip"),
            timestamp_writes: None,
        });
        pass.set_bind_group(0, &group, &[]);
        pass.set_pipeline(&self.sum_tip_pipeline);
        pass.dispatch_workgroups(1, 1, 1);
    }

    pub fn full_reserve_volume(&self) -> f32 {
        self.reserve_volume(FULL_RESERVOIR_VOLUME_UM)
    }

    fn reserve_volume(&self, thickness_um: f32) -> f32 {
        self.params.get("reserve_factor") * thickness_um * disk_texels(self.tip_radius) as f32
    }

    fn submit_fill(&self, gpu: &GpuContext, record: &DipRecord, fills_tip: bool) {
        let group = self.dip_bind_group(gpu, record);
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("brush_dip"),
            });
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("brush_dip"),
            timestamp_writes: None,
        });
        pass.set_bind_group(0, &group, &[]);
        pass.set_pipeline(&self.fill_body_pipeline);
        pass.dispatch_workgroups(1, 1, 1);
        if fills_tip {
            pass.set_pipeline(&self.fill_tip_pipeline);
            pass.dispatch_workgroups(TIP_GROUPS, TIP_GROUPS, 1);
        }
        drop(pass);
        gpu.queue.submit([encoder.finish()]);
    }

    fn dip_bind_group(&self, gpu: &GpuContext, record: &DipRecord) -> wgpu::BindGroup {
        let uniform = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("brush_dip"),
                contents: bytemuck::bytes_of(record),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("brush_dip"),
            layout: &self.dip_layout,
            entries: &[
                buffer_binding(0, &uniform),
                buffer_binding(1, &self.body),
                buffer_binding(2, &self.tip),
                buffer_binding(3, &self.tip_sums),
            ],
        })
    }

    pub fn clean_brush(&mut self, gpu: &GpuContext) {
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("clean_brush"),
            });
        encoder.clear_buffer(&self.tip, 0, None);
        encoder.clear_buffer(&self.tip_delta, 0, None);
        encoder.clear_buffer(&self.body, 0, None);
        encoder.clear_buffer(&self.wanted, 0, None);
        encoder.clear_buffer(&self.capillary_rows, 0, None);
        gpu.queue.submit([encoder.finish()]);
    }

    pub fn dip_knife(
        &mut self,
        gpu: &GpuContext,
        mixture: &Mixture,
        amount: f32,
    ) -> Result<(), GpuLost> {
        let Some(thickness) = dip_thickness(mixture) else {
            return Ok(());
        };
        if !amount.is_finite() || amount <= 0.0 {
            return Ok(());
        }
        let fractions = dip_fractions(mixture);
        let mut load = read_buffer(gpu, &self.blade_load)?;
        let reach = self.tip_radius.ceil() as i32 + 1;
        for offset in -reach..=reach {
            let cover = blade_cell_cover(offset, self.tip_radius);
            let cell = (TIP_HALF as i32 + offset).clamp(0, BLADE_CELLS as i32 - 1) as usize;
            let values = &mut load[cell * TIP_CHANNELS..(cell + 1) * TIP_CHANNELS];
            let room = (KNIFE_LAID_CAPACITY_UM - values[VOLUME_CHANNEL]).max(0.0);
            let added = (cover * amount * thickness * KNIFE_DIP_LENGTH_PX).min(room);
            for (slot, fraction) in fractions.iter().enumerate() {
                values[slot] += added * fraction;
            }
            values[VOLUME_CHANNEL] += added;
        }
        gpu.queue
            .write_buffer(&self.blade_load, 0, bytemuck::cast_slice(&load));
        Ok(())
    }

    pub fn knife_full_dip_volume(&self, thickness_um: f32) -> f32 {
        if !thickness_um.is_finite() || thickness_um <= 0.0 {
            return 0.0;
        }
        let thickness = thickness_um.min(MAX_TIP_THICKNESS_UM);
        let reach = self.tip_radius.ceil() as i32 + 1;
        (-reach..=reach)
            .map(|offset| {
                (blade_cell_cover(offset, self.tip_radius) * thickness * KNIFE_DIP_LENGTH_PX)
                    .min(KNIFE_LAID_CAPACITY_UM)
            })
            .sum()
    }

    pub fn clean_knife(&mut self, gpu: &GpuContext) {
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("clean_knife"),
            });
        encoder.clear_buffer(&self.blade_load, 0, None);
        encoder.clear_buffer(&self.blade_delta, 0, None);
        gpu.queue.submit([encoder.finish()]);
    }

    pub fn knife_totals(&self, gpu: &GpuContext) -> Result<PaintTotals, GpuLost> {
        Ok(knife_from_load(&read_buffer(gpu, &self.blade_load)?))
    }

    fn blade_bind_group(
        &self,
        gpu: &GpuContext,
        canvas: &Canvas,
        dabs: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blade"),
            layout: &self.blade_layout,
            entries: &[
                dab_binding(0, dabs),
                texture_binding(1, canvas, StateTexture::WetPig0),
                texture_binding(2, canvas, StateTexture::WetPig1),
                texture_binding(3, canvas, StateTexture::WetVol),
                texture_binding(4, canvas, StateTexture::DryHeight),
                buffer_binding(5, &self.blade_load),
                buffer_binding(6, &self.blade_delta),
                buffer_binding(7, &self.blade_grant),
                buffer_binding(8, &self.blade_state),
            ],
        })
    }

    pub fn brush_totals(&self, gpu: &GpuContext) -> Result<PaintTotals, GpuLost> {
        let mut totals = PaintTotals::default();
        for texel in read_buffer(gpu, &self.tip)?.as_chunks::<TIP_CHANNELS>().0 {
            accumulate(&mut totals, texel);
        }
        accumulate(&mut totals, &read_buffer(gpu, &self.body)?[..TIP_CHANNELS]);
        Ok(totals)
    }

    pub fn reservoir_mixture(&self, gpu: &GpuContext) -> Result<Mixture, GpuLost> {
        let body = read_buffer(gpu, &self.body)?;
        let volume = body[VOLUME_CHANNEL];
        let fractions = std::array::from_fn(|slot| {
            if volume > 0.0 {
                body[slot] / volume
            } else {
                0.0
            }
        });
        Ok(Mixture {
            volume_um: self.loaded_thickness_um,
            fractions,
        })
    }

    pub fn encode_eyedropper(
        &self,
        gpu: &GpuContext,
        canvas: &Canvas,
        x: f32,
        y: f32,
        radius: f32,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        encoder.clear_buffer(&self.picked, 0, None);
        if !x.is_finite() || !y.is_finite() {
            return;
        }
        let radius = if radius.is_finite() {
            radius.clamp(MIN_PICK_RADIUS_PX, MAX_PICK_RADIUS_PX)
        } else {
            MIN_PICK_RADIUS_PX
        };
        let (width, height) = (canvas.spec.width as f32, canvas.spec.height as f32);
        if x + radius < 0.0 || y + radius < 0.0 || x - radius > width || y - radius > height {
            return;
        }
        let record = PickRecord {
            center: [x, y],
            radius,
            _pad: 0.0,
        };
        let pick = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("eyedropper"),
                contents: bytemuck::bytes_of(&record),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("eyedropper"),
            layout: &self.pick_layout,
            entries: &[
                buffer_binding(0, &pick),
                texture_binding(1, canvas, StateTexture::WetPig0),
                texture_binding(2, canvas, StateTexture::WetPig1),
                texture_binding(3, canvas, StateTexture::WetVol),
                texture_binding(4, canvas, StateTexture::DryTopPig0),
                texture_binding(5, canvas, StateTexture::DryTopPig1),
                buffer_binding(6, &self.picked),
            ],
        });
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("eyedropper"),
            timestamp_writes: None,
        });
        pass.set_bind_group(0, &group, &[]);
        pass.set_pipeline(&self.pick_pipeline);
        pass.dispatch_workgroups(1, 1, 1);
    }

    pub fn picked_mixture(&self, gpu: &GpuContext) -> Result<Option<Mixture>, GpuLost> {
        let picked = read_buffer(gpu, &self.picked)?;
        if picked[PICKED_PAINT_FOUND].to_bits() == 0 {
            return Ok(None);
        }
        Ok(Some(Mixture {
            volume_um: self.loaded_thickness_um,
            fractions: std::array::from_fn(|slot| picked[PICKED_FRACTIONS + slot]),
        }))
    }

    pub fn brush_outline(
        &self,
        brush: &BrushSettings,
        sample: &StrokeSample,
    ) -> Option<BrushOutline> {
        if matches!(brush.tool, Tool::Knife | Tool::Eyedropper) {
            return None;
        }
        let angles = pen_angles(sample.tilt_x_deg, sample.tilt_y_deg);
        let dab = Dab {
            x: sample.x as f32,
            y: sample.y as f32,
            radius: brush.geometry.max_diameter_px / 2.0,
            pressure: 1.0,
            altitude: angles.altitude_rad,
            azimuth: angles.azimuth_rad,
            direction: 0.0,
            speed: 0.0,
        };
        let footprint = Footprint::of(&dab, brush, &self.params);
        Some(BrushOutline {
            semi_axes: footprint.semi_axes(),
            orientation: footprint.orientation,
        })
    }

    pub fn dab_bounds(
        &self,
        canvas: &Canvas,
        brush: &BrushSettings,
        dab: &Dab,
    ) -> Option<[f32; 4]> {
        place(
            dab,
            REFERENCE_STEP_PX,
            [0.0; 2],
            brush,
            &self.params,
            self.loaded_thickness_um,
            canvas,
        )
        .map(|placement| placement.canvas_rect)
    }

    pub fn encode_dry_tiles(
        &mut self,
        gpu: &GpuContext,
        canvas: &mut Canvas,
        tiles: &[TileCoord],
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if tiles.is_empty() {
            return;
        }
        let tile_list: Vec<[u32; 2]> = tiles.iter().map(|tile| [tile.x, tile.y]).collect();
        let tile_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("dry_tiles"),
                contents: bytemuck::cast_slice(&tile_list),
                usage: wgpu::BufferUsages::STORAGE,
            });
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("dry"),
            layout: &self.dry_layout,
            entries: &[
                buffer_binding(0, canvas.slot_table()),
                buffer_binding(1, &tile_buffer),
                texture_binding(2, canvas, StateTexture::WetPig0),
                texture_binding(3, canvas, StateTexture::WetPig1),
                texture_binding(4, canvas, StateTexture::WetVol),
                texture_binding(5, canvas, StateTexture::DryRefl0),
                texture_binding(6, canvas, StateTexture::DryRefl1),
                texture_binding(7, canvas, StateTexture::DryHeight),
                texture_binding(8, canvas, StateTexture::DryTopPig0),
                texture_binding(9, canvas, StateTexture::DryTopPig1),
            ],
        });
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("dry"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.dry_pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(
            DRY_WORKGROUPS_PER_TILE_SIDE,
            DRY_WORKGROUPS_PER_TILE_SIDE,
            tile_list.len() as u32,
        );
        drop(pass);
        for tile in tiles {
            let x0 = (tile.x * TILE_SIZE) as f32;
            let y0 = (tile.y * TILE_SIZE) as f32;
            let last = (TILE_SIZE - 1) as f32;
            canvas.dirty.mark_rect(x0, y0, x0 + last, y0 + last);
        }
    }

    fn rag_bind_group(
        &self,
        gpu: &GpuContext,
        canvas: &Canvas,
        dabs: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rag"),
            layout: &self.rag_layout,
            entries: &[
                dab_binding(0, dabs),
                texture_binding(1, canvas, StateTexture::WetVol),
            ],
        })
    }

    fn exchange_bind_group(
        &self,
        gpu: &GpuContext,
        canvas: &Canvas,
        dabs: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("dab_exchange"),
            layout: &self.exchange_layout,
            entries: &[
                dab_binding(0, dabs),
                texture_binding(1, canvas, StateTexture::WetPig0),
                texture_binding(2, canvas, StateTexture::WetPig1),
                texture_binding(3, canvas, StateTexture::WetVol),
                texture_binding(4, canvas, StateTexture::DryHeight),
                buffer_binding(5, &self.tip),
                buffer_binding(6, &self.tip_delta),
                buffer_binding(7, &self.contact_reference),
            ],
        })
    }

    fn fit_scratch(&mut self, gpu: &GpuContext, canvas: &Canvas) {
        let side = max_dab_region_side();
        let size = [canvas.spec.width.min(side), canvas.spec.height.min(side)];
        if self
            .scratch
            .as_ref()
            .is_none_or(|scratch| scratch.size != size)
        {
            self.scratch = Some(Scratch::new(gpu, size));
        }
    }

    fn spreading_bind_groups(
        &self,
        gpu: &GpuContext,
        canvas: &Canvas,
        dabs: &wgpu::Buffer,
    ) -> (wgpu::BindGroup, wgpu::BindGroup) {
        let scratch = self
            .scratch
            .as_ref()
            .expect("scratch surfaces are fitted before spreading");
        let scratch_views = [&scratch.pig0.view, &scratch.pig1.view, &scratch.vol.view];
        let motion_view = &scratch.motion.view;
        let wet_views = [
            canvas.view(StateTexture::WetPig0),
            canvas.view(StateTexture::WetPig1),
            canvas.view(StateTexture::WetVol),
        ];
        let group = |label, layout, from: [&wgpu::TextureView; 3], to: [&wgpu::TextureView; 3]| {
            let views = from.into_iter().chain(to).chain([motion_view]);
            let entries: Vec<wgpu::BindGroupEntry> = std::iter::once(dab_binding(0, dabs))
                .chain(
                    (1..)
                        .zip(views)
                        .map(|(binding, view)| wgpu::BindGroupEntry {
                            binding,
                            resource: wgpu::BindingResource::TextureView(view),
                        }),
                )
                .collect();
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout,
                entries: &entries,
            })
        };
        (
            group("region_copy", &self.copy_layout, wet_views, scratch_views),
            group("advect", &self.advect_layout, scratch_views, wet_views),
        )
    }

    fn apply_bind_group(&self, gpu: &GpuContext, dabs: &wgpu::Buffer) -> wgpu::BindGroup {
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tip_apply"),
            layout: &self.apply_layout,
            entries: &[
                dab_binding(0, dabs),
                buffer_binding(1, &self.tip),
                buffer_binding(2, &self.tip_delta),
            ],
        })
    }

    fn refill_bind_group(&self, gpu: &GpuContext, dabs: &wgpu::Buffer) -> wgpu::BindGroup {
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tip_refill"),
            layout: &self.refill_layout,
            entries: &[
                dab_binding(0, dabs),
                buffer_binding(1, &self.tip),
                buffer_binding(2, &self.tip_delta),
                buffer_binding(3, &self.body),
                buffer_binding(4, &self.wanted),
                buffer_binding(5, &self.capillary_rows),
            ],
        })
    }
}

fn dip_thickness(mixture: &Mixture) -> Option<f32> {
    let thickness = mixture.volume_um;
    (thickness.is_finite() && thickness > 0.0).then(|| thickness.min(MAX_TIP_THICKNESS_UM))
}

fn dip_fractions(mixture: &Mixture) -> [f32; PIGMENT_CHANNELS] {
    mixture.fractions.map(|fraction| {
        if fraction.is_finite() {
            fraction.clamp(0.0, 1.0)
        } else {
            0.0
        }
    })
}

fn disk_texels(radius: f32) -> u32 {
    let radius_squared = radius * radius;
    let offsets = -(TIP_HALF as i32)..(TIP_SIZE - TIP_HALF) as i32;
    offsets
        .clone()
        .map(|y| {
            offsets
                .clone()
                .filter(|&x| {
                    let (x, y) = (x as f32, y as f32);
                    x * x + y * y <= radius_squared
                })
                .count() as u32
        })
        .sum()
}

fn tip_mean_from_sums(sums: &[f32]) -> Mixture {
    let volume = sums[VOLUME_CHANNEL];
    let texels = sums[TIP_TEXEL_COUNT_SUM];
    if volume < MIN_TIP_MEAN_VOLUME_UM || texels <= 0.0 {
        return Mixture {
            volume_um: 0.0,
            fractions: [0.0; PIGMENT_CHANNELS],
        };
    }
    Mixture {
        volume_um: volume / texels,
        fractions: std::array::from_fn(|slot| sums[slot] / volume),
    }
}

fn reserve_from_body(body: &[f32]) -> PaintTotals {
    let mut totals = PaintTotals::default();
    accumulate(&mut totals, &body[..TIP_CHANNELS]);
    totals
}

fn knife_from_load(load: &[f32]) -> PaintTotals {
    let mut totals = PaintTotals::default();
    for cell in load.as_chunks::<TIP_CHANNELS>().0 {
        accumulate(&mut totals, cell);
    }
    totals
}

fn accumulate(totals: &mut PaintTotals, amounts: &[f32]) {
    for (slot, amount) in amounts[..PIGMENT_CHANNELS].iter().enumerate() {
        totals.pigment[slot] += f64::from(*amount);
    }
    totals.volume += f64::from(amounts[VOLUME_CHANNEL]);
}

fn dab_binding(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
            buffer,
            offset: 0,
            size: wgpu::BufferSize::new(DAB_RECORD_SIZE),
        }),
    }
}

pub(crate) fn buffer_binding(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}

pub(crate) fn texture_binding(
    binding: u32,
    canvas: &Canvas,
    which: StateTexture,
) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: wgpu::BindingResource::TextureView(canvas.view(which)),
    }
}

impl PaintEngine for WetOilEngine {
    fn begin_stroke(&mut self) {
        self.rest_pending = true;
        self.previous_dab = None;
    }

    fn encode_dabs(
        &mut self,
        gpu: &GpuContext,
        canvas: &mut Canvas,
        brush: &BrushSettings,
        dabs: &[Dab],
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let mut placements = Vec::with_capacity(dabs.len());
        for dab in dabs {
            let (step_px, sweep) = self.step_since_previous_dab(dab);
            placements.extend(place(
                dab,
                step_px,
                sweep,
                brush,
                &self.params,
                self.loaded_thickness_um,
                canvas,
            ));
        }
        if placements.is_empty() {
            return;
        }
        if let Some(first_resting) = placements.iter_mut().find(|placement| !placement.wipes)
            && self.rest_pending
        {
            first_resting.record.rest_follow = FIRST_REST_FOLLOW;
            self.rest_pending = false;
        }
        let mut records = vec![0u8; placements.len() * DAB_RECORD_STRIDE as usize];
        for (slot, placement) in records
            .as_chunks_mut::<{ DAB_RECORD_STRIDE as usize }>()
            .0
            .iter_mut()
            .zip(&placements)
        {
            slot[..DAB_RECORD_SIZE as usize].copy_from_slice(bytemuck::bytes_of(&placement.record));
        }
        let dab_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("dab_records"),
                contents: &records,
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let exchange_group = self.exchange_bind_group(gpu, canvas, &dab_buffer);
        let apply_group = self.apply_bind_group(gpu, &dab_buffer);
        let refill_group = self.refill_bind_group(gpu, &dab_buffer);
        let rag_group = self.rag_bind_group(gpu, canvas, &dab_buffer);
        let blade_group = self.blade_bind_group(gpu, canvas, &dab_buffer);
        self.fit_scratch(gpu, canvas);
        debug_assert!(placements.iter().all(|placement| {
            let scratch_size = self.scratch.as_ref().map_or([0; 2], |scratch| scratch.size);
            (0..2).all(|axis| placement.record.canvas_size[axis] <= scratch_size[axis])
        }));
        let (copy_group, advect_group) = self.spreading_bind_groups(gpu, canvas, &dab_buffer);

        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("dab_exchange"),
            timestamp_writes: None,
        });
        for (index, placement) in placements.iter().enumerate() {
            let offset = (index as u64 * DAB_RECORD_STRIDE) as wgpu::DynamicOffset;
            if placement.wipes {
                pass.set_bind_group(0, &rag_group, &[offset]);
                pass.set_pipeline(&self.rag_pipeline);
                pass.dispatch_workgroups(placement.canvas_groups[0], placement.canvas_groups[1], 1);
                continue;
            }
            if placement.blades {
                pass.set_bind_group(0, &copy_group, &[offset]);
                pass.set_pipeline(&self.copy_pipeline);
                pass.dispatch_workgroups(placement.canvas_groups[0], placement.canvas_groups[1], 1);
                pass.set_bind_group(0, &advect_group, &[offset]);
                pass.set_pipeline(&self.advect_pipeline);
                pass.dispatch_workgroups(placement.spread_groups[0], placement.spread_groups[1], 1);
                let blade = &self.blade_pipelines;
                let [width, height] = placement.canvas_groups;
                pass.set_bind_group(0, &blade_group, &[offset]);
                for (pipeline, groups) in [
                    (&blade.rest, [1, 1]),
                    (&blade.want, [width, height]),
                    (&blade.grant, [BLADE_CELL_GROUPS, 1]),
                    (&blade.lay, [width, height]),
                    (&blade.settle, [1, 1]),
                    (&blade.spill, [width, height]),
                ] {
                    pass.set_pipeline(pipeline);
                    pass.dispatch_workgroups(groups[0], groups[1], 1);
                }
                continue;
            }
            pass.set_bind_group(0, &exchange_group, &[offset]);
            pass.set_pipeline(&self.reference_pipeline);
            pass.dispatch_workgroups(1, 1, 1);
            pass.set_pipeline(&self.exchange_pipeline);
            pass.dispatch_workgroups(placement.canvas_groups[0], placement.canvas_groups[1], 1);
            pass.set_bind_group(0, &apply_group, &[offset]);
            pass.set_pipeline(&self.apply_pipeline);
            pass.dispatch_workgroups(placement.tip_groups[0], placement.tip_groups[1], 1);
            pass.set_bind_group(0, &refill_group, &[offset]);
            let capillary = &self.capillary_pipelines;
            for (pipeline, groups) in [
                (&capillary.gather, placement.tip_groups),
                (&capillary.settle, [CAPILLARY_POOL_GROUPS, 1]),
                (&capillary.distribute, placement.tip_groups),
            ] {
                pass.set_pipeline(pipeline);
                pass.dispatch_workgroups(groups[0], groups[1], 1);
            }
            if placement.refills {
                pass.set_pipeline(&self.gather_pipeline);
                pass.dispatch_workgroups(placement.tip_groups[0], placement.tip_groups[1], 1);
                pass.set_pipeline(&self.settle_pipeline);
                pass.dispatch_workgroups(1, 1, 1);
                pass.set_pipeline(&self.distribute_pipeline);
                pass.dispatch_workgroups(placement.tip_groups[0], placement.tip_groups[1], 1);
            }
            if placement.spreads {
                pass.set_bind_group(0, &copy_group, &[offset]);
                pass.set_pipeline(&self.copy_pipeline);
                pass.dispatch_workgroups(placement.canvas_groups[0], placement.canvas_groups[1], 1);
                pass.set_bind_group(0, &advect_group, &[offset]);
                pass.set_pipeline(&self.advect_pipeline);
                pass.dispatch_workgroups(placement.spread_groups[0], placement.spread_groups[1], 1);
            }
        }
        drop(pass);

        for placement in &placements {
            let [x0, y0, x1, y1] = placement.canvas_rect;
            canvas.dirty.mark_rect(x0, y0, x1, y1);
            canvas.wet_tiles.mark_rect(x0, y0, x1, y1);
        }
    }

    fn params(&self) -> &SimParams {
        &self.params
    }

    fn params_mut(&mut self) -> &mut SimParams {
        &mut self.params
    }
}
