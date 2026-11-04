use cadmium_engine::{
    draw_up::DrawUpRequest,
    gpu::GpuContext,
    pigments::{MAX_SLOTS, Mixture},
    session::PaintSession,
    stroke::Tool,
    surface::Surface,
    tube::Nozzle,
};

use crate::{load_slot, session, still_nozzle};

fn blob(gpu: &GpuContext, s: &mut PaintSession, slot: usize) {
    s.begin_hold(gpu, Surface::Tray);
    for _ in 0..5 {
        s.squeeze(gpu, still_nozzle(slot, 0.2));
    }
    s.end_hold();
}

fn touch(pressure: f32, seconds: f32) -> DrawUpRequest {
    DrawUpRequest {
        center: [300.0, 300.0],
        radius: 12.0,
        pressure,
        seconds,
    }
}

#[test]
fn drawing_up_moves_paint_from_the_tray_into_the_reservoir() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    blob(&gpu, &mut s, 1);
    s.clean_brush(&gpu);
    let tray_before = s.tray.wet_totals(&gpu).unwrap().volume;
    s.begin_hold(&gpu, Surface::Tray);
    let taken = s.draw_up(&gpu, touch(1.0, 0.1)).unwrap().taken;
    s.end_hold();
    let tray_after = s.tray.wet_totals(&gpu).unwrap().volume;
    let reserve = s.engine.reserve_totals(&gpu).unwrap();
    assert!(taken.volume > 0.0);
    assert!(((tray_before - tray_after) - taken.volume).abs() < 1e-2 * taken.volume);
    assert!((reserve.volume - taken.volume).abs() < 1e-2 * taken.volume);
    assert!((reserve.pigment[1] - reserve.volume).abs() < 1e-2 * reserve.volume);
}

#[test]
fn harder_touches_draw_up_faster() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    blob(&gpu, &mut s, 0);
    s.clean_brush(&gpu);
    s.begin_hold(&gpu, Surface::Tray);
    let light = s.draw_up(&gpu, touch(0.2, 0.05)).unwrap().taken.volume;
    let hard = s.draw_up(&gpu, touch(1.0, 0.05)).unwrap().taken.volume;
    s.end_hold();
    assert!(hard > 2.0 * light);
}

#[test]
fn drawing_up_stops_when_the_reservoir_is_full() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    let wide = |seconds| Nozzle {
        from: [300.0, 300.0],
        to: [300.0, 300.0],
        radius: 60.0,
        slot: 0,
        pressure: 1.0,
        seconds,
        still_reach_px: 0.0,
    };
    s.begin_hold(&gpu, Surface::Tray);
    for _ in 0..20 {
        s.squeeze(&gpu, wide(0.5));
    }
    s.end_hold();
    s.clean_brush(&gpu);
    let wide_touch = DrawUpRequest {
        center: [300.0, 300.0],
        radius: 60.0,
        pressure: 1.0,
        seconds: 0.1,
    };
    s.begin_hold(&gpu, Surface::Tray);
    for _ in 0..200 {
        let _ = s.draw_up(&gpu, wide_touch);
    }
    let last = s.draw_up(&gpu, wide_touch).unwrap();
    s.end_hold();
    let reserve = s.engine.reserve_totals(&gpu).unwrap().volume as f32;
    assert!(reserve <= s.engine.full_reserve_volume() * 1.001);
    assert!(reserve >= s.engine.full_reserve_volume() * 0.99);
    assert!(last.taken.volume < 1e-3 * f64::from(s.engine.full_reserve_volume()));
    assert!(last.level >= 0.99);
}

#[test]
fn the_reservoir_level_rises_with_each_draw_up() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    blob(&gpu, &mut s, 0);
    s.clean_brush(&gpu);
    s.begin_hold(&gpu, Surface::Tray);
    let first = s.draw_up(&gpu, touch(1.0, 0.05)).unwrap();
    let second = s.draw_up(&gpu, touch(1.0, 0.05)).unwrap();
    s.end_hold();
    let reserve = s.engine.reserve_totals(&gpu).unwrap().volume as f32;
    assert!(first.level > 0.0);
    assert!(second.level > first.level);
    assert!((second.level - reserve / s.engine.full_reserve_volume()).abs() < 1e-3);
}

#[test]
fn touching_a_clean_tray_still_reports_the_level() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    load_slot(&gpu, &mut s, 0);
    s.begin_hold(&gpu, Surface::Tray);
    let drawn = s.draw_up(&gpu, touch(1.0, 0.1)).unwrap();
    s.end_hold();
    let reserve = s.engine.reserve_totals(&gpu).unwrap().volume as f32;
    assert_eq!(drawn.taken.volume, 0.0);
    assert!((drawn.level - reserve / s.engine.full_reserve_volume()).abs() < 1e-3);
    assert!(drawn.level > 0.0);
}

#[test]
fn drawing_up_into_a_loaded_brush_keeps_its_load_thickness() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    blob(&gpu, &mut s, 1);
    s.brush.load = 0.3;
    load_slot(&gpu, &mut s, 0);
    let loaded = s.engine.reservoir_mixture(&gpu).unwrap().volume_um;
    s.begin_hold(&gpu, Surface::Tray);
    assert!(s.draw_up(&gpu, touch(1.0, 0.05)).unwrap().taken.volume > 0.0);
    s.end_hold();
    assert_eq!(s.engine.reservoir_mixture(&gpu).unwrap().volume_um, loaded);
}

#[test]
fn drawing_up_into_a_clean_brush_gives_it_the_default_load_thickness() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    blob(&gpu, &mut s, 1);
    s.brush.load = 0.3;
    load_slot(&gpu, &mut s, 0);
    let thin = s.engine.reservoir_mixture(&gpu).unwrap().volume_um;
    s.clean_brush(&gpu);
    s.begin_hold(&gpu, Surface::Tray);
    assert!(s.draw_up(&gpu, touch(1.0, 0.05)).unwrap().taken.volume > 0.0);
    s.end_hold();
    assert!(s.engine.reservoir_mixture(&gpu).unwrap().volume_um > thin);
}

#[test]
fn drawing_up_from_a_clean_tray_takes_nothing() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    s.clean_brush(&gpu);
    s.begin_hold(&gpu, Surface::Tray);
    let taken = s.draw_up(&gpu, touch(1.0, 0.1)).unwrap().taken;
    s.end_hold();
    assert_eq!(taken.volume, 0.0);
    assert!(!s.history.can_undo());
}

#[test]
fn drawing_up_beside_the_paint_leaves_no_undo_step() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    blob(&gpu, &mut s, 0);
    s.clean_brush(&gpu);
    s.begin_hold(&gpu, Surface::Tray);
    let mut beside = touch(1.0, 0.1);
    beside.center = [326.0, 300.0];
    let taken = s.draw_up(&gpu, beside).unwrap().taken;
    s.end_hold();
    assert_eq!(taken.volume, 0.0);
    assert!(s.undo(&gpu));
    assert!(!s.history.can_undo());
}

#[test]
fn drawing_up_without_a_hold_takes_nothing() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    blob(&gpu, &mut s, 0);
    s.clean_brush(&gpu);
    let taken = s.draw_up(&gpu, touch(1.0, 0.1)).unwrap().taken;
    assert_eq!(taken.volume, 0.0);
    assert_eq!(s.engine.reserve_totals(&gpu).unwrap().volume, 0.0);
}

#[test]
fn undoing_a_draw_up_puts_the_paint_back_on_the_tray() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    blob(&gpu, &mut s, 0);
    let squeezed = s.tray.wet_totals(&gpu).unwrap().volume;
    s.begin_hold(&gpu, Surface::Tray);
    s.draw_up(&gpu, touch(1.0, 0.1)).unwrap();
    s.end_hold();
    assert!(s.tray.wet_totals(&gpu).unwrap().volume < squeezed);
    assert!(s.undo(&gpu));
    assert_eq!(s.tray.wet_totals(&gpu).unwrap().volume, squeezed);
}

#[test]
fn the_knife_draws_up_into_its_blade() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    blob(&gpu, &mut s, 1);
    s.brush.tool = Tool::Knife;
    s.clean_brush(&gpu);
    s.begin_hold(&gpu, Surface::Tray);
    let taken = s.draw_up(&gpu, touch(1.0, 0.1)).unwrap().taken;
    s.end_hold();
    let knife = s.engine.knife_totals(&gpu).unwrap();
    assert!(taken.volume > 0.0);
    assert!((knife.volume - taken.volume).abs() < 1e-2 * taken.volume);
}

#[test]
fn the_blender_and_the_rag_draw_up_nothing() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    blob(&gpu, &mut s, 0);
    let squeezed = s.tray.wet_totals(&gpu).unwrap().volume;
    for tool in [Tool::Blender, Tool::Rag] {
        s.brush.tool = tool;
        s.begin_hold(&gpu, Surface::Tray);
        let taken = s.draw_up(&gpu, touch(1.0, 0.1)).unwrap().taken;
        s.end_hold();
        assert_eq!(taken.volume, 0.0);
    }
    assert_eq!(s.tray.wet_totals(&gpu).unwrap().volume, squeezed);
}

#[test]
fn a_blade_full_at_its_middle_closes_the_loading_ring() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    blob(&gpu, &mut s, 1);
    s.brush.tool = Tool::Knife;
    s.clean_brush(&gpu);
    s.brush.geometry.max_diameter_px = 10.0;
    let mut fractions = [0.0; MAX_SLOTS];
    fractions[0] = 1.0;
    s.dip(
        &gpu,
        &Mixture {
            volume_um: 200.0,
            fractions,
        },
        1.0,
    )
    .unwrap();
    s.brush.geometry.max_diameter_px = 40.0;
    s.begin_hold(&gpu, Surface::Tray);
    let drawn = s.draw_up(&gpu, touch(1.0, 0.1)).unwrap();
    s.end_hold();
    assert!(drawn.taken.volume < 1.0);
    assert_eq!(drawn.level, 1.0);
}
