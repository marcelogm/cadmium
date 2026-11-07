//! The tray's blender and rag work a tip of their own: what they pick up stays with them,
//! and the painting brush holds exactly what it held.

use cadmium_engine::{
    engine::Tip, gpu::GpuContext, session::PaintSession, stroke::Tool, surface::Surface,
};

use crate::{frame, load_slot, sample, session, stroke_on};

/// A stroke on the tray with the tray's `tool`, worked by the tray's tip.
fn tray_tool_stroke(gpu: &GpuContext, s: &mut PaintSession, tool: Tool, y: f64) {
    let painting_tool = s.brush.tool;
    s.brush.tool = tool;
    s.begin_stroke_with(Surface::Tray, Tip::Tray, sample(40.0, y, 0));
    for i in 1..=40 {
        s.add_samples(&[sample(40.0 + 10.0 * f64::from(i), y, 8 * i)]);
        if i % 4 == 0 {
            frame(gpu, s);
        }
    }
    s.end_stroke();
    frame(gpu, s);
    s.brush.tool = painting_tool;
}

/// Paints a band of slot 1 across the tray with the painting brush, then loads slot 0.
fn tray_with_paint(gpu: &GpuContext) -> PaintSession {
    let mut s = session(gpu);
    load_slot(gpu, &mut s, 1);
    stroke_on(gpu, &mut s, Surface::Tray, 100.0);
    load_slot(gpu, &mut s, 0);
    s
}

#[test]
fn the_tray_blender_leaves_the_painting_brush_holding_what_it_held() {
    let gpu = GpuContext::new().unwrap();
    let mut s = tray_with_paint(&gpu);
    let before = s.read_brush(&gpu).unwrap();
    tray_tool_stroke(&gpu, &mut s, Tool::Blender, 100.0);
    assert_eq!(s.read_brush(&gpu).unwrap(), before);
    stroke_on(&gpu, &mut s, Surface::Painting, 100.0);
    assert_eq!(
        s.canvas.wet_totals(&gpu).unwrap().pigment[1],
        0.0,
        "the next painting stroke carries none of the tray's paint"
    );
}

#[test]
fn the_tray_blender_keeps_its_own_mix_from_stroke_to_stroke() {
    let gpu = GpuContext::new().unwrap();
    let mut s = tray_with_paint(&gpu);
    assert!(s.read_tip(&gpu, Tip::Tray).unwrap().tip_mean.is_empty());
    tray_tool_stroke(&gpu, &mut s, Tool::Blender, 100.0);
    let picked = s.read_tip(&gpu, Tip::Tray).unwrap().tip_mean;
    assert!(picked.fractions[1] > 0.0, "{picked:?}");
    // A painting stroke in between, then a tray stroke over bare glass.
    stroke_on(&gpu, &mut s, Surface::Painting, 100.0);
    tray_tool_stroke(&gpu, &mut s, Tool::Blender, 400.0);
    let kept = s.read_tip(&gpu, Tip::Tray).unwrap().tip_mean;
    assert!(kept.fractions[1] > 0.0, "{kept:?}");
    s.clean_tray_tip(&gpu);
    assert!(s.read_tip(&gpu, Tip::Tray).unwrap().tip_mean.is_empty());
    assert!(
        !s.read_brush(&gpu).unwrap().tip_mean.is_empty(),
        "cleaning it leaves the brush"
    );
}

#[test]
fn the_tray_rag_leaves_the_painting_brush_holding_what_it_held() {
    let gpu = GpuContext::new().unwrap();
    let mut s = tray_with_paint(&gpu);
    let before = s.read_brush(&gpu).unwrap();
    tray_tool_stroke(&gpu, &mut s, Tool::Rag, 100.0);
    assert_eq!(s.read_brush(&gpu).unwrap(), before);
}
