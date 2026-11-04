use cadmium_engine::{
    canvas::StateTexture,
    engine::PaintEngine,
    gpu::GpuContext,
    stroke::{StrokeSample, Tool},
};

use crate::support::{drag, line, session, stroke, write_preview};
use crate::{CONSERVATION_TOLERANCE, column, cross_section, relative_drift, total};

const STROKE_EXTENT_SHARE: f32 = 0.05;

#[test]
fn spreading_conserves_volume() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    let before = total(&gpu, &s);
    s.brush.tool = Tool::Blender;
    stroke(&gpu, &mut s, 128.0, 470.0, 40.0);
    let after = total(&gpu, &s);
    let (volume, pigment) = relative_drift(&before, &after);
    eprintln!("blender volume drift {volume:e}, pigment drift {pigment:?}");
    assert!(volume.abs() < CONSERVATION_TOLERANCE);
}

#[test]
fn brush_and_knife_spreading_conserve_volume_and_each_pigment() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let before = total(&gpu, &s);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    stroke(&gpu, &mut s, 140.0, 470.0, 40.0);
    s.brush.tool = Tool::Knife;
    drag(&gpu, &mut s, &line(134.0, 40.0, 470.0, 1.0, 60.0));
    drag(&gpu, &mut s, &line(120.0, 470.0, 40.0, 1.0, 0.0));
    let after = total(&gpu, &s);
    let (volume, pigment) = relative_drift(&before, &after);
    eprintln!("brush+knife volume drift {volume:e}, pigment drift {pigment:?}");
    assert!(volume.abs() < CONSERVATION_TOLERANCE);
    assert!(pigment.iter().all(|drift| drift.abs() < 5e-3));
}

fn band_means(profile: &[f32]) -> (f32, f32) {
    let peak = profile.iter().cloned().fold(0.0, f32::max);
    let painted: Vec<usize> = (0..profile.len())
        .filter(|&row| profile[row] > STROKE_EXTENT_SHARE * peak)
        .collect();
    let (first, last) = (painted[0], painted[painted.len() - 1]);
    let band = (last - first + 1) / 4;
    let mean =
        |rows: Vec<usize>| rows.iter().map(|&row| profile[row]).sum::<f32>() / rows.len() as f32;
    let outer = mean(
        (first..first + band)
            .chain(last + 1 - band..=last)
            .collect(),
    );
    let central = mean((first + band..=last - band).collect());
    (outer, central)
}

fn diagonal(from: (f64, f64), to: (f64, f64), pressure: f32, tilt_x_deg: f32) -> Vec<StrokeSample> {
    line(0.0, 0.0, 1.0, pressure, tilt_x_deg)
        .into_iter()
        .map(|sample| StrokeSample {
            x: from.0 + (to.0 - from.0) * sample.x,
            y: from.1 + (to.1 - from.1) * sample.x,
            ..sample
        })
        .collect()
}

#[test]
fn strokes_running_off_the_canvas_edges_conserve_volume() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let before = total(&gpu, &s);
    drag(&gpu, &mut s, &line(3.0, -30.0, 540.0, 1.0, 20.0));
    drag(&gpu, &mut s, &column(508.0, 280.0, -30.0, 1.0, 50.0));
    let edged = total(&gpu, &s);
    s.brush.geometry.max_diameter_px = 500.0;
    s.reload_brush(&gpu);
    let reloaded = total(&gpu, &s);
    drag(
        &gpu,
        &mut s,
        &diagonal((-40.0, 300.0), (560.0, -40.0), 1.0, 60.0),
    );
    s.brush.tool = Tool::Knife;
    drag(
        &gpu,
        &mut s,
        &diagonal((560.0, 290.0), (-40.0, -30.0), 1.0, 0.0),
    );
    let after = total(&gpu, &s);
    let (first, _) = relative_drift(&before, &edged);
    let (second, pigment) = relative_drift(&reloaded, &after);
    eprintln!("edge volume drift {first:e} then {second:e}, pigment drift {pigment:?}");
    assert!(first.abs() < CONSERVATION_TOLERANCE && second.abs() < CONSERVATION_TOLERANCE);
}

#[test]
fn brush_stroke_piles_paint_at_its_edges() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.engine.params_mut().set("bristle_contrast", 0.0);
    stroke(&gpu, &mut s, 128.0, 40.0, 470.0);
    let profile = cross_section(&gpu, &s, 96..161);
    eprintln!("cross-section {profile:?}");
    let (outer, central) = band_means(&profile);
    assert!(outer > central, "outer band {outer} central band {central}");
}

#[test]
fn spread_mixtures_keep_their_fractions_within_the_volume() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.brush.medium = 0.0;
    s.brush.recipe = vec![(0, 0.31), (1, 0.27), (2, 0.42)];
    s.reload_brush(&gpu);
    stroke(&gpu, &mut s, 120.0, 40.0, 470.0);
    s.brush.recipe = vec![(1, 0.6), (2, 0.4)];
    s.reload_brush(&gpu);
    stroke(&gpu, &mut s, 132.0, 470.0, 40.0);
    s.brush.tool = Tool::Blender;
    s.clean_brush(&gpu);
    drag(&gpu, &mut s, &column(256.0, 60.0, 200.0, 1.0, 20.0));
    s.brush.tool = Tool::Knife;
    drag(&gpu, &mut s, &line(126.0, 40.0, 470.0, 1.0, 60.0));
    let low = s.canvas.read_f32(&gpu, StateTexture::WetPig0).unwrap();
    let high = s.canvas.read_f32(&gpu, StateTexture::WetPig1).unwrap();
    let worst = low
        .chunks(4)
        .zip(high.chunks(4))
        .map(|(low, high)| low.iter().chain(high).sum::<f32>())
        .fold(0.0, f32::max);
    assert!(worst <= 1.0, "fractions sum to {worst}");
}

#[test]
fn blender_previews_show_smeared_colours() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    stroke(&gpu, &mut s, 100.0, 40.0, 470.0);
    s.brush.recipe = vec![(1, 1.0)];
    s.reload_brush(&gpu);
    stroke(&gpu, &mut s, 150.0, 40.0, 470.0);
    s.brush.tool = Tool::Blender;
    s.clean_brush(&gpu);
    for (x, from, to) in [
        (120.0, 60.0, 200.0),
        (200.0, 200.0, 60.0),
        (280.0, 60.0, 200.0),
        (360.0, 200.0, 60.0),
    ] {
        drag(&gpu, &mut s, &column(x, from, to, 1.0, 20.0));
    }
    write_preview(&gpu, &s, "engine-blend.png");
    assert!(s.canvas.wet_totals(&gpu).unwrap().volume > 0.0);
}
