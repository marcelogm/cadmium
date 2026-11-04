use cadmium_engine::stroke::{StrokeSample, Tool};
use cadmium_engine::surface::Surface;

mod contact;
mod hold;
mod press;
mod stroke;

pub const LOAD_CANCEL_SCREEN_PX: f64 = 4.0;
const STILL_TUBE_SCREEN_PX: f32 = 3.0;
const MAX_HOLD_STEP_SECONDS: f32 = 0.1;
const FULL_LEVEL: f32 = 0.999;

pub fn load_cancelled(origin: (f64, f64), current: (f64, f64), threshold_px: f64) -> bool {
    (current.0 - origin.0).hypot(current.1 - origin.1) > threshold_px
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoadRing {
    pub level: f32,
    pub pressure: f32,
    pub colour: [u8; 3],
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Contact {
    Stroke {
        surface: Surface,
    },
    Loading {
        tool: Tool,
        origin: (f64, f64),
        pen: StrokeSample,
        colour: Option<[u8; 3]>,
        level: Option<f32>,
    },
    Squeezing {
        surface: Surface,
        pen: StrokeSample,
        nozzle_at: (f64, f64),
        squeezed: bool,
    },
}

#[cfg(test)]
mod tests {
    use cadmium_engine::stroke::Tool;
    use cadmium_engine::surface::Surface;

    use super::*;
    use crate::state::AppState;
    use crate::state::test_support::{
        WHITE_SLOT, brush_totals, painting_state, present_one_frame, present_one_tray_frame,
        sample_at, settle_both_surfaces, squeeze_blob_on_tray, wet_volume,
    };

    use crate::tools::UiTool;

    #[test]
    fn load_is_cancelled_past_the_threshold() {
        assert!(!load_cancelled((10.0, 10.0), (14.0, 10.0), 4.0));
        assert!(load_cancelled((10.0, 10.0), (14.01, 10.0), 4.0));
        assert!(!load_cancelled((10.0, 10.0), (10.0, 10.0), 4.0));
    }

    #[test]
    fn the_cancel_distance_is_measured_on_screen() {
        for (drag, cancels) in [(10.0, false), (20.0, true)] {
            let mut state = painting_state();
            state.set_tray_zoom(0.25);
            state.set_ui_tool(UiTool::Paint(Tool::Brush));
            state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
            state.motion(&[sample_at(300.0 + drag, 300.0, 0.9)]);
            assert_eq!(!state.holds_on(Surface::Tray), cancels, "{drag}");
            state.release();
        }
    }

    #[test]
    fn a_tray_zoom_that_is_not_a_scale_is_ignored() {
        let mut state = painting_state();
        for zoom in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            state.set_tray_zoom(zoom);
        }
        state.set_ui_tool(UiTool::Paint(Tool::Brush));
        state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
        state.motion(&[sample_at(305.0, 300.0, 0.9)]);
        assert!(!state.holds_on(Surface::Tray));
        state.release();
    }

    #[test]
    fn a_hold_step_of_no_real_time_squeezes_nothing() {
        let mut state = painting_state();
        state.set_ui_tool(UiTool::Tube);
        state.press(Surface::Tray, sample_at(300.0, 300.0, 1.0), false);
        state.advance_hold(f32::NAN);
        state.release();
        assert_eq!(
            state.session.tray.wet_totals(&state.gpu).unwrap().volume,
            0.0
        );
        assert!(!state.has_unsaved_edits());
    }

    #[test]
    fn squeezing_off_the_canvas_is_not_an_edit() {
        let mut state = painting_state();
        state.set_ui_tool(UiTool::Tube);
        state.press(Surface::Painting, sample_at(-1000.0, -1000.0, 1.0), false);
        state.advance_hold(MAX_HOLD_STEP_SECONDS);
        state.release();
        assert!(!state.has_unsaved_edits());
    }

    const FRAME_GAP: std::time::Duration = std::time::Duration::from_millis(20);

    fn tray_volume(state: &AppState) -> f64 {
        state.session.tray.wet_totals(&state.gpu).unwrap().volume
    }

    #[test]
    fn a_hold_on_the_painting_advances_only_with_painting_frames() {
        let mut state = painting_state();
        state.set_ui_tool(UiTool::Tube);
        state.press(Surface::Painting, sample_at(100.0, 100.0, 1.0), false);
        for _ in 0..2 {
            present_one_tray_frame(&mut state);
            std::thread::sleep(FRAME_GAP);
        }
        assert_eq!(wet_volume(&state), 0.0);
        for _ in 0..2 {
            present_one_frame(&mut state);
            std::thread::sleep(FRAME_GAP);
        }
        state.release();
        assert!(wet_volume(&state) > 0.0);
    }

    #[test]
    fn a_hold_on_the_tray_advances_only_with_tray_frames() {
        let mut state = painting_state();
        state.set_ui_tool(UiTool::Tube);
        state.press(Surface::Tray, sample_at(300.0, 300.0, 1.0), false);
        for _ in 0..2 {
            present_one_frame(&mut state);
            std::thread::sleep(FRAME_GAP);
        }
        assert_eq!(tray_volume(&state), 0.0);
        for _ in 0..2 {
            present_one_tray_frame(&mut state);
            std::thread::sleep(FRAME_GAP);
        }
        state.release();
        assert!(tray_volume(&state) > 0.0);
    }

    #[test]
    fn a_still_touch_with_the_knife_names_the_pile_after_what_the_blade_took() {
        let mut state = painting_state();
        squeeze_blob_on_tray(&mut state, WHITE_SLOT);
        state.set_ui_tool(UiTool::Paint(Tool::Knife));
        state.clean_brush();
        state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
        state.advance_hold(MAX_HOLD_STEP_SECONDS);
        state.release();
        assert!(state.knife_level() > 0.0);
        assert_eq!(state.session.brush.recipe.len(), 1);
        assert_eq!(state.session.brush.recipe[0].0, WHITE_SLOT);
    }

    #[test]
    fn a_still_touch_on_the_tray_loads_the_brush_from_it() {
        let mut state = painting_state();
        squeeze_blob_on_tray(&mut state, 1);
        state.clean_brush();
        state.set_ui_tool(UiTool::Paint(Tool::Brush));
        assert!(state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false));
        state.advance_hold(0.2);
        let ring = state.load_ring().unwrap();
        state.release();
        assert!(ring.level > 0.0);
        assert!(state.reserve_level() > 0.0);
        let slots: Vec<usize> = state
            .session
            .brush
            .recipe
            .iter()
            .map(|(slot, _)| *slot)
            .collect();
        assert_eq!(slots, vec![1]);
    }

    #[test]
    fn a_hold_is_on_the_surface_it_began_on() {
        let mut state = painting_state();
        state.set_ui_tool(UiTool::Tube);
        state.press(Surface::Painting, sample_at(100.0, 100.0, 1.0), false);
        assert!(state.holds_on(Surface::Painting));
        assert!(!state.holds_on(Surface::Tray));
        state.set_ui_tool(UiTool::Paint(Tool::Brush));
        state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
        assert!(state.holds_on(Surface::Tray));
        assert!(!state.holds_on(Surface::Painting));
        state.release();
        assert!(!state.holds_on(Surface::Tray));
    }

    #[test]
    fn a_contact_is_on_the_surface_it_began_on() {
        let mut state = painting_state();
        state.set_ui_tool(UiTool::Paint(Tool::Brush));
        state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
        assert!(state.contact_on(Surface::Tray));
        assert!(!state.contact_on(Surface::Painting));
        state.release();
        state.press(Surface::Painting, sample_at(100.0, 100.0, 0.9), false);
        assert!(state.contact_on(Surface::Painting));
        assert!(!state.contact_on(Surface::Tray));
        state.release();
        assert!(!state.contact_on(Surface::Painting));
    }

    #[test]
    fn touching_the_tray_during_a_painting_stroke_leaves_only_the_tray_contact() {
        let mut state = painting_state();
        state.set_ui_tool(UiTool::Paint(Tool::Brush));
        state.press(Surface::Painting, sample_at(100.0, 100.0, 0.9), false);
        state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
        assert!(state.contact_on(Surface::Tray));
        assert!(!state.contact_on(Surface::Painting));
        state.release();
    }

    #[test]
    fn a_stroke_is_not_a_hold_on_any_surface() {
        let mut state = painting_state();
        state.set_ui_tool(UiTool::Paint(Tool::Brush));
        state.press(Surface::Painting, sample_at(100.0, 100.0, 0.9), false);
        assert!(!state.holds_on(Surface::Painting));
        assert!(!state.holds_on(Surface::Tray));
        state.release();
    }

    #[test]
    fn a_hold_counts_as_a_contact_until_the_pen_lifts() {
        let mut state = painting_state();
        state.set_ui_tool(UiTool::Paint(Tool::Brush));
        state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
        assert!(state.is_stroking());
        assert!(state.holds_on(Surface::Tray));
        state.release();
        assert!(!state.is_stroking());
        assert!(!state.holds_on(Surface::Tray));
    }

    #[test]
    fn a_load_that_takes_nothing_is_not_an_edit() {
        let mut state = painting_state();
        let recipe = state.session.brush.recipe.clone();
        state.set_ui_tool(UiTool::Paint(Tool::Brush));
        state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
        state.advance_hold(0.2);
        state.release();
        assert!(!state.has_unsaved_edits());
        assert_eq!(state.session.brush.recipe, recipe);
    }

    #[test]
    fn a_tray_stroke_leaves_the_painting_with_no_pending_work() {
        let mut state = painting_state();
        settle_both_surfaces(&mut state);
        state.set_ui_tool(UiTool::Paint(Tool::Blender));
        state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
        state.motion(&[sample_at(340.0, 300.0, 0.9)]);
        assert!(state.session.tray_has_pending_work());
        assert!(!state.session.has_pending_work());
        state.release();
        assert!(state.session.tray_has_pending_work());
        assert!(!state.session.has_pending_work());
        present_one_tray_frame(&mut state);
        assert!(!state.session.tray_has_pending_work());
    }

    #[test]
    fn a_painting_stroke_leaves_the_tray_with_no_pending_work() {
        let mut state = painting_state();
        settle_both_surfaces(&mut state);
        state.set_ui_tool(UiTool::Paint(Tool::Brush));
        state.press(Surface::Painting, sample_at(100.0, 100.0, 0.9), false);
        state.motion(&[sample_at(140.0, 100.0, 0.9)]);
        state.release();
        assert!(state.session.has_pending_work());
        assert!(!state.session.tray_has_pending_work());
    }

    #[test]
    fn a_tray_hold_leaves_the_painting_with_no_pending_work() {
        let mut state = painting_state();
        settle_both_surfaces(&mut state);
        state.set_ui_tool(UiTool::Tube);
        state.press(Surface::Tray, sample_at(300.0, 300.0, 1.0), false);
        state.advance_hold(MAX_HOLD_STEP_SECONDS);
        assert!(state.session.tray_has_pending_work());
        assert!(!state.session.has_pending_work());
        state.release();
        assert!(!state.session.has_pending_work());
    }

    #[test]
    fn a_still_touch_on_a_clean_tray_shows_no_ring() {
        let mut state = painting_state();
        state.clean_brush();
        state.set_ui_tool(UiTool::Paint(Tool::Brush));
        state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
        assert_eq!(state.load_ring(), None);
        state.advance_hold(0.2);
        assert_eq!(state.load_ring(), None);
        state.release();
    }

    #[test]
    fn a_full_brush_closes_the_ring_at_once_on_a_clean_tray() {
        let mut state = painting_state();
        state.set_load(1.0);
        state.reload_brush();
        assert!(state.reserve_level() >= FULL_LEVEL);
        state.set_ui_tool(UiTool::Paint(Tool::Brush));
        state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
        let ring = state.load_ring().expect("a full brush rings at once");
        assert!((ring.level - 1.0).abs() < 1e-3, "{}", ring.level);
        state.advance_hold(0.2);
        let ring = state.load_ring().expect("still full");
        assert!((ring.level - 1.0).abs() < 1e-3, "{}", ring.level);
        state.release();
        assert!(!state.has_unsaved_edits());
    }

    #[test]
    fn the_ring_appears_once_paint_is_drawn_up() {
        let mut state = painting_state();
        squeeze_blob_on_tray(&mut state, 1);
        state.clean_brush();
        state.set_ui_tool(UiTool::Paint(Tool::Brush));
        state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
        assert_eq!(state.load_ring(), None);
        state.advance_hold(0.2);
        let ring = state.load_ring().expect("paint was drawn up");
        assert!(ring.level > 0.0);
        assert_eq!(ring.pressure, 0.9);
        state.release();
    }

    #[test]
    fn dragging_past_four_pixels_turns_the_load_into_a_stroke() {
        let mut state = painting_state();
        squeeze_blob_on_tray(&mut state, 1);
        state.set_ui_tool(UiTool::Paint(Tool::Brush));
        state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
        state.motion(&[sample_at(303.0, 300.0, 0.9)]);
        assert!(state.holds_on(Surface::Tray));
        state.motion(&[sample_at(310.0, 300.0, 0.9)]);
        assert!(!state.holds_on(Surface::Tray));
        assert!(state.is_stroking());
        state.release();
        assert!(!state.is_stroking());
    }

    #[test]
    fn a_stroke_grown_from_a_load_keeps_what_was_loaded() {
        let mut state = painting_state();
        squeeze_blob_on_tray(&mut state, WHITE_SLOT);
        state.clean_brush();
        state.set_ui_tool(UiTool::Paint(Tool::Brush));
        state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
        state.advance_hold(0.1);
        let loaded = brush_totals(&state);
        state.motion(&[sample_at(310.0, 300.0, 0.9)]);
        assert!(state.reserve_level() < 1.0);
        assert_eq!(brush_totals(&state), loaded);
        assert!(loaded.pigment[WHITE_SLOT] > 0.0);
        state.release();
    }

    #[test]
    fn the_tube_squeezes_its_own_pigment_and_leaves_the_brush_alone() {
        let mut state = painting_state();
        state.clean_brush();
        state.set_ui_tool(UiTool::Tube);
        state.set_tube_slot(1);
        state.press(Surface::Painting, sample_at(100.0, 100.0, 1.0), false);
        state.advance_hold(0.2);
        state.release();
        let totals = state.session.canvas.wet_totals(&state.gpu).unwrap();
        assert!(totals.pigment[1] > 0.0 && totals.pigment[0] == 0.0);
        assert!(state.brush_is_clean());
        assert!(state.has_unsaved_edits());
    }

    fn painting_height_at(state: &AppState, x: usize, y: usize) -> f32 {
        let volume = state
            .session
            .canvas
            .read_f32(&state.gpu, cadmium_engine::canvas::StateTexture::WetVol)
            .unwrap();
        volume[y * state.session.canvas.spec.width as usize + x]
    }

    #[test]
    #[expect(clippy::excessive_nesting)]
    fn a_tube_on_a_zoomed_out_painting_keeps_growing_through_a_trembling_hand() {
        let mut state = painting_state();
        state.set_brush_size(10.0);
        state.view.zoom = 0.25;
        state.set_ui_tool(UiTool::Tube);
        let one_screen_px = 1.0 / f64::from(state.view.zoom);
        let shaken = |frame: usize| {
            let shake = if frame.is_multiple_of(2) {
                one_screen_px
            } else {
                -one_screen_px
            };
            sample_at(200.5 + shake, 200.5, 0.5)
        };
        let tremble = |state: &mut AppState, frames: std::ops::Range<usize>| {
            for frame in frames {
                state.motion(&[shaken(frame)]);
                state.advance_hold(0.05);
            }
        };
        state.press(Surface::Painting, sample_at(200.5, 200.5, 0.5), false);
        tremble(&mut state, 0..10);
        let early = painting_height_at(&state, 200, 200);
        tremble(&mut state, 10..30);
        let later = painting_height_at(&state, 200, 200);
        state.release();
        assert!(later > early, "{early} -> {later}");
    }

    #[test]
    fn a_hold_step_never_lasts_longer_than_a_tenth_of_a_second() {
        let mut once = painting_state();
        let mut long = painting_state();
        for state in [&mut once, &mut long] {
            state.set_ui_tool(UiTool::Tube);
            state.press(Surface::Tray, sample_at(300.0, 300.0, 1.0), false);
        }
        once.advance_hold(MAX_HOLD_STEP_SECONDS);
        long.advance_hold(10.0);
        once.release();
        long.release();
        let squeezed = |state: &AppState| state.session.tray.wet_totals(&state.gpu).unwrap().volume;
        assert_eq!(squeezed(&long), squeezed(&once));
    }

    #[test]
    fn the_eraser_wipes_the_tray_with_the_rag() {
        let mut state = painting_state();
        squeeze_blob_on_tray(&mut state, 0);
        let before = state.session.tray.wet_totals(&state.gpu).unwrap().volume;
        state.set_ui_tool(UiTool::Paint(Tool::Brush));
        state.press(Surface::Tray, sample_at(280.0, 300.0, 1.0), true);
        state.motion(&[sample_at(320.0, 300.0, 1.0)]);
        state.release();
        present_one_tray_frame(&mut state);
        assert!(state.session.tray.wet_totals(&state.gpu).unwrap().volume < before);
    }

    #[test]
    fn the_move_tool_and_the_eyedropper_do_not_touch_down() {
        let mut state = painting_state();
        for tool in [UiTool::Move, UiTool::Paint(Tool::Eyedropper)] {
            state.set_ui_tool(tool);
            assert!(!state.press(Surface::Painting, sample_at(100.0, 100.0, 1.0), false));
            assert!(!state.is_stroking());
        }
    }
}
