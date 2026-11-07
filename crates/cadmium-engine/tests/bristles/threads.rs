use cadmium_engine::canvas::{CanvasSpec, Primer, StateTexture, Weave};
use cadmium_engine::engine::BristleCell;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::StrokeSample;

use crate::support::{frame, write_png};

const WIDTH: usize = 2048;
const HEIGHT: usize = 256;
const ROW: f64 = 128.0;
const FROM_X: f64 = 60.0;
const STROKE_LENGTH_PX: f64 = 1500.0;
const SAMPLE_SPACING_PX: f64 = 3.6;
const SAMPLE_INTERVAL_MS: u32 = 4;
const MAX_DIAMETER_PX: f32 = 70.0;
const TILT_DEG: f32 = 25.0;
const PRESSURE: f32 = 0.8;
const LOAD: f32 = 0.2;
const PAINTED_UM: f32 = 0.5;
const SAMPLES_PER_FRAME: usize = 4;
const SAMPLES_PER_READING: usize = 10;
const DRY_FILM_UM: f32 = PAINTED_UM;
const MIN_DRY_CLUMPS: usize = 3;
const MIN_END_SPREAD_SHARE: f64 = 0.1;
const PREVIEW_GAIN: f32 = 4.0;
const DRAINING_SHARE: f64 = 0.5;
const BELLY_REACH_SHARE: f32 = 0.5;
const RIM_REACH_SHARE: f32 = 0.8;
const MIN_BELLY_TO_RIM_FILM: f32 = 2.0;

fn session(gpu: &GpuContext) -> PaintSession {
    let mut s = PaintSession::new(
        gpu,
        CanvasSpec {
            width: WIDTH as u32,
            height: HEIGHT as u32,
            primer: Primer::WhiteGesso,
            weave: Weave::Smooth,
            pigment_slots: vec!["ultramarine_blue".into(), "titanium_white".into()],
        },
    )
    .unwrap();
    s.brush.geometry.max_diameter_px = MAX_DIAMETER_PX;
    s.brush.recipe = vec![(0, 0.8), (1, 0.2)];
    s.brush.load = LOAD;
    s.reload_brush(gpu);
    s
}

fn line_samples() -> Vec<StrokeSample> {
    let count = (STROKE_LENGTH_PX / SAMPLE_SPACING_PX) as u32;
    (0..=count)
        .map(|i| StrokeSample {
            x: FROM_X + f64::from(i) * SAMPLE_SPACING_PX,
            y: ROW,
            pressure: PRESSURE,
            tilt_x_deg: TILT_DEG,
            tilt_y_deg: 0.0,
            time_ms: i * SAMPLE_INTERVAL_MS,
        })
        .collect()
}

struct Clump {
    films_um: Vec<f32>,
    touched: bool,
}

impl Clump {
    fn end_px(&self, readings_px: &[f64]) -> Option<f64> {
        let last_wet = self.films_um.iter().rposition(|&film| film > DRY_FILM_UM)?;
        Some(
            readings_px
                .get(last_wet + 1)
                .copied()
                .unwrap_or(STROKE_LENGTH_PX),
        )
    }
}

fn clump_film_um(cells: &[BristleCell], members: &[usize]) -> f32 {
    let volume: f32 = members.iter().map(|&b| cells[b].volume_um_texels).sum();
    let texels: u32 = members.iter().map(|&b| cells[b].texels).sum();
    volume / texels.max(1) as f32
}

struct ClumpTracks {
    clumps: Vec<Clump>,
    readings_px: Vec<f64>,
}

impl ClumpTracks {
    fn read(&mut self, gpu: &GpuContext, s: &PaintSession, sample: &StrokeSample) {
        let snapshot = s.engine.bristle_snapshot(gpu).unwrap();
        let cells = s.engine.bristle_cells(gpu).unwrap();
        let count = snapshot
            .clusters
            .iter()
            .map(|&c| c as usize + 1)
            .max()
            .unwrap_or(0);
        self.clumps.resize_with(count, || Clump {
            films_um: Vec::new(),
            touched: false,
        });
        for (index, clump) in self.clumps.iter_mut().enumerate() {
            let members: Vec<usize> = (0..cells.len())
                .filter(|&bristle| snapshot.clusters[bristle] as usize == index)
                .collect();
            clump.films_um.push(clump_film_um(&cells, &members));
            clump.touched |= members.iter().any(|&b| snapshot.touching[b]);
        }
        self.readings_px.push(sample.x - FROM_X);
    }

    fn ends_px(&self) -> Vec<Option<f64>> {
        self.clumps
            .iter()
            .filter(|clump| clump.touched)
            .map(|clump| clump.end_px(&self.readings_px))
            .collect()
    }
}

fn paint_and_track(gpu: &GpuContext, s: &mut PaintSession) -> ClumpTracks {
    let mut tracks = ClumpTracks {
        clumps: Vec::new(),
        readings_px: Vec::new(),
    };
    for (index, sample) in line_samples().iter().enumerate() {
        if index == 0 {
            s.begin_stroke(*sample);
        } else {
            s.add_samples(&[*sample]);
        }
        if index % SAMPLES_PER_FRAME == 0 {
            frame(gpu, s);
        }
        if index > 0 && index % SAMPLES_PER_READING == 0 {
            tracks.read(gpu, s, sample);
        }
    }
    s.end_stroke();
    frame(gpu, s);
    tracks
}

fn preview(volume: &[f32]) -> Vec<u8> {
    volume
        .iter()
        .flat_map(|&volume| {
            let shade = 255.0 - (volume * PREVIEW_GAIN).min(255.0);
            [shade as u8, shade as u8, 255, 255]
        })
        .collect()
}

#[test]
fn the_clumps_of_a_long_stroke_run_dry_at_clearly_different_points() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let tracks = paint_and_track(&gpu, &mut s);
    let volume = s.canvas.read_f32(&gpu, StateTexture::WetVol).unwrap();
    write_png(
        "bristle-threads.png",
        WIDTH as u32,
        HEIGHT as u32,
        &preview(&volume),
    );
    let ends_px = tracks.ends_px();
    eprintln!("clump track ends {ends_px:?} px");
    let never_wet = ends_px.iter().filter(|end| end.is_none()).count();
    assert_eq!(
        never_wet, 0,
        "{never_wet} clumps touched the canvas but were never wet"
    );
    let ends_px: Vec<f64> = ends_px.into_iter().flatten().collect();
    let dry = ends_px
        .iter()
        .filter(|&&end| end < STROKE_LENGTH_PX)
        .count();
    assert!(
        dry >= MIN_DRY_CLUMPS,
        "only {dry} clumps ran dry before the stroke ended"
    );
    let first = ends_px.iter().cloned().fold(f64::INFINITY, f64::min);
    let last = ends_px.iter().cloned().fold(0.0, f64::max);
    let spread = (last - first) / STROKE_LENGTH_PX;
    assert!(
        spread >= MIN_END_SPREAD_SHARE,
        "the clumps run dry within {spread:.3} of the stroke of one another"
    );
}

fn mean_film_um(cells: &[BristleCell], members: &[usize]) -> f32 {
    members
        .iter()
        .map(|&bristle| cells[bristle].volume_um_texels / cells[bristle].texels.max(1) as f32)
        .sum::<f32>()
        / members.len().max(1) as f32
}

fn root_reach_shares(roots_px: &[[f32; 2]]) -> Vec<f32> {
    let count = roots_px.len().max(1) as f32;
    let centre = roots_px.iter().fold([0.0, 0.0], |sum, root| {
        [sum[0] + root[0] / count, sum[1] + root[1] / count]
    });
    let reaches: Vec<f32> = roots_px
        .iter()
        .map(|root| (root[0] - centre[0]).hypot(root[1] - centre[1]))
        .collect();
    let widest = reaches.iter().cloned().fold(f32::MIN_POSITIVE, f32::max);
    reaches.iter().map(|reach| reach / widest).collect()
}

#[test]
fn a_draining_brush_keeps_its_belly_wet_while_its_outer_hairs_run_dry() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let samples = line_samples();
    let draining = &samples[..(samples.len() as f64 * DRAINING_SHARE) as usize];
    for (index, sample) in draining.iter().enumerate() {
        if index == 0 {
            s.begin_stroke(*sample);
        } else {
            s.add_samples(&[*sample]);
        }
        if index % SAMPLES_PER_FRAME == 0 {
            frame(&gpu, &mut s);
        }
    }
    frame(&gpu, &mut s);
    let snapshot = s.engine.bristle_snapshot(&gpu).unwrap();
    let cells = s.engine.bristle_cells(&gpu).unwrap();
    let shares = root_reach_shares(&snapshot.roots_px);
    let belly: Vec<usize> = (0..shares.len())
        .filter(|&bristle| shares[bristle] < BELLY_REACH_SHARE)
        .collect();
    let rim: Vec<usize> = (0..shares.len())
        .filter(|&bristle| shares[bristle] > RIM_REACH_SHARE)
        .collect();
    let (belly_um, rim_um) = (mean_film_um(&cells, &belly), mean_film_um(&cells, &rim));
    eprintln!("belly hairs hold {belly_um:.3} um, outer hairs {rim_um:.3} um");
    assert!(
        belly_um > MIN_BELLY_TO_RIM_FILM * rim_um,
        "the outer hairs hold as much paint as the belly: {belly_um:.3} vs {rim_um:.3} um"
    );
}
