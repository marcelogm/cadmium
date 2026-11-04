use std::time::Instant;

use cadmium_engine::canvas::PaintTotals;
use cadmium_engine::draw_up::DrawUpRequest;
use cadmium_engine::pigments::mixture_srgb8;
use cadmium_engine::stroke::{StrokeSample, Tool};
use cadmium_engine::surface::Surface;
use cadmium_engine::tube::Nozzle;

use super::{AppState, StrokePreparation, mean_mixture, recipe_from_fractions, stroke_preparation};
use crate::tools::UiTool;

pub const LOAD_CANCEL_SCREEN_PX: f64 = 4.0;
const MAX_HOLD_STEP_SECONDS: f32 = 0.1;
const LOAD_RING_EMPTY_COLOUR: [u8; 3] = [128, 128, 128];

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
    Stroke,
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

impl AppState {
    pub fn is_stroking(&self) -> bool {
        self.contact.is_some()
    }

    pub fn is_holding(&self) -> bool {
        matches!(
            self.contact,
            Some(Contact::Loading { .. } | Contact::Squeezing { .. })
        )
    }

    pub fn set_tray_zoom(&mut self, zoom: f32) {
        if zoom.is_finite() && zoom > 0.0 {
            self.tray_zoom = zoom;
        }
    }

    pub fn holds_on(&self, surface: Surface) -> bool {
        self.hold_surface() == Some(surface)
    }

    fn hold_surface(&self) -> Option<Surface> {
        match self.contact? {
            Contact::Stroke => None,
            Contact::Loading { .. } => Some(Surface::Tray),
            Contact::Squeezing { surface, .. } => Some(surface),
        }
    }

    pub fn press(&mut self, surface: Surface, sample: StrokeSample, eraser: bool) -> bool {
        let began = match (eraser, self.ui_tool) {
            (true, _) => {
                self.begin_stroke_on(surface, Tool::Rag, sample);
                true
            }
            (false, UiTool::Move | UiTool::Paint(Tool::Eyedropper)) => false,
            (false, UiTool::Tube) => {
                let squeezing = Contact::Squeezing {
                    surface,
                    pen: sample,
                    nozzle_at: (sample.x, sample.y),
                    squeezed: false,
                };
                self.begin_hold(surface, sample, squeezing);
                true
            }
            (false, UiTool::Paint(tool @ (Tool::Brush | Tool::Knife)))
                if surface == Surface::Tray =>
            {
                let loading = Contact::Loading {
                    tool,
                    origin: (sample.x, sample.y),
                    pen: sample,
                    colour: None,
                    level: None,
                };
                self.begin_hold(surface, sample, loading);
                self.session.brush.tool = tool;
                true
            }
            (false, UiTool::Paint(tool)) => {
                self.begin_stroke_on(surface, tool, sample);
                true
            }
        };
        self.wake();
        began
    }

    pub fn begin_stroke(&mut self, tool: Tool, sample: StrokeSample) {
        self.begin_stroke_on(Surface::Painting, tool, sample);
    }

    fn begin_stroke_on(&mut self, surface: Surface, tool: Tool, sample: StrokeSample) {
        let preparation = stroke_preparation(tool, self.reload_each_stroke, self.clean_each_stroke);
        self.begin_prepared_stroke(surface, tool, sample, preparation);
    }

    fn begin_prepared_stroke(
        &mut self,
        surface: Surface,
        tool: Tool,
        sample: StrokeSample,
        preparation: StrokePreparation,
    ) {
        self.release();
        self.session.brush.tool = tool;
        match preparation {
            StrokePreparation::None => {}
            StrokePreparation::TopUp => self.top_up(),
            StrokePreparation::CleanAndLoad => self.clean_and_load(),
        }
        self.session.begin_stroke_on(&self.gpu, surface, sample);
        self.contact = Some(Contact::Stroke);
        self.last_sample_time_ms = None;
        self.note_samples(std::slice::from_ref(&sample));
        self.wake();
    }

    fn begin_hold(&mut self, surface: Surface, sample: StrokeSample, contact: Contact) {
        self.release();
        self.session.begin_hold(&self.gpu, surface);
        self.contact = Some(contact);
        self.hold_clock = None;
        self.last_sample_time_ms = None;
        self.note_samples(std::slice::from_ref(&sample));
    }

    pub fn add_samples(&mut self, samples: &[StrokeSample]) {
        if !matches!(self.contact, Some(Contact::Stroke)) || samples.is_empty() {
            return;
        }
        self.session.add_samples(samples);
        self.note_samples(samples);
        self.wake();
    }

    pub fn motion(&mut self, samples: &[StrokeSample]) {
        let Some(&pen) = samples.last() else {
            return;
        };
        match self.contact {
            None => return,
            Some(Contact::Stroke) => {
                self.add_samples(samples);
                return;
            }
            Some(Contact::Squeezing {
                surface,
                nozzle_at,
                squeezed,
                ..
            }) => {
                self.contact = Some(Contact::Squeezing {
                    surface,
                    pen,
                    nozzle_at,
                    squeezed,
                });
            }
            Some(Contact::Loading {
                tool,
                origin,
                colour,
                level,
                ..
            }) => {
                let threshold_px = LOAD_CANCEL_SCREEN_PX / f64::from(self.tray_zoom);
                let cancelled_at = samples
                    .iter()
                    .position(|sample| load_cancelled(origin, (sample.x, sample.y), threshold_px));
                if let Some(index) = cancelled_at {
                    self.begin_prepared_stroke(
                        Surface::Tray,
                        tool,
                        samples[index],
                        StrokePreparation::None,
                    );
                    self.add_samples(&samples[index + 1..]);
                    return;
                }
                self.contact = Some(Contact::Loading {
                    tool,
                    origin,
                    pen,
                    colour,
                    level,
                });
            }
        }
        self.note_samples(samples);
        self.wake();
    }

    pub fn advance_hold(&mut self, seconds: f32) {
        if seconds.is_nan() || seconds <= 0.0 {
            return;
        }
        let seconds = seconds.min(MAX_HOLD_STEP_SECONDS);
        let radius = self.session.brush.geometry.max_diameter_px / 2.0;
        match self.contact {
            Some(Contact::Squeezing {
                surface,
                pen,
                nozzle_at,
                squeezed,
            }) => {
                let squeezed_now = self.session.squeeze(
                    &self.gpu,
                    Nozzle {
                        from: [nozzle_at.0 as f32, nozzle_at.1 as f32],
                        to: [pen.x as f32, pen.y as f32],
                        radius,
                        slot: self.tube_slot,
                        pressure: pen.pressure,
                        seconds,
                    },
                );
                self.contact = Some(Contact::Squeezing {
                    surface,
                    pen,
                    nozzle_at: (pen.x, pen.y),
                    squeezed: squeezed || squeezed_now,
                });
            }
            Some(Contact::Loading {
                tool,
                origin,
                pen,
                colour,
                ..
            }) => {
                let request = DrawUpRequest {
                    center: [pen.x as f32, pen.y as f32],
                    radius,
                    pressure: pen.pressure,
                    seconds,
                };
                let Ok(drawn) = self.session.draw_up(&self.gpu, request) else {
                    self.release();
                    return;
                };
                let taken = (drawn.taken.volume > 0.0).then(|| self.colour_of(&drawn.taken));
                self.contact = Some(Contact::Loading {
                    tool,
                    origin,
                    pen,
                    colour: taken.or(colour),
                    level: Some(drawn.level),
                });
            }
            Some(Contact::Stroke) | None => return,
        }
        self.wake();
    }

    pub(super) fn advance_hold_by_clock(&mut self, frame_surface: Surface) {
        if self.hold_surface() != Some(frame_surface) {
            return;
        }
        let now = Instant::now();
        let seconds = self
            .hold_clock
            .replace(now)
            .map_or(0.0, |last| now.duration_since(last).as_secs_f32());
        self.advance_hold(seconds);
    }

    fn colour_of(&self, totals: &PaintTotals) -> [u8; 3] {
        mixture_srgb8(
            &self.session.db,
            &self.session.canvas.spec.pigment_slots,
            &mean_mixture(totals).fractions,
        )
    }

    fn held_level(&self, tool: Tool) -> f32 {
        if tool == Tool::Knife {
            self.knife_level()
        } else {
            self.reserve_level()
        }
    }

    pub fn load_ring(&self) -> Option<LoadRing> {
        let Some(Contact::Loading {
            tool,
            pen,
            colour,
            level,
            ..
        }) = self.contact
        else {
            return None;
        };
        Some(LoadRing {
            level: level.unwrap_or_else(|| self.held_level(tool)),
            pressure: pen.pressure,
            colour: colour.unwrap_or(LOAD_RING_EMPTY_COLOUR),
        })
    }

    pub fn release(&mut self) {
        let Some(contact) = self.contact.take() else {
            return;
        };
        match contact {
            Contact::Stroke => self.finish_stroke(),
            Contact::Squeezing { squeezed, .. } => {
                self.session.end_hold();
                if squeezed {
                    self.edits.record();
                }
            }
            Contact::Loading { tool, colour, .. } => {
                self.session.end_hold();
                self.finish_loading(tool, colour.is_some());
            }
        }
        self.wake();
    }

    fn finish_stroke(&mut self) {
        self.session.end_stroke();
        self.edits.record();
        self.measure_brush();
    }

    fn finish_loading(&mut self, tool: Tool, loaded: bool) {
        let readout = self.measure_brush();
        if !loaded {
            return;
        }
        self.edits.record();
        let Some(readout) = readout else {
            return;
        };
        let held = if tool == Tool::Knife {
            readout.knife
        } else {
            readout.reserve
        };
        let recipe = recipe_from_fractions(&mean_mixture(&held).fractions);
        if !recipe.is_empty() {
            self.session.brush.recipe = recipe;
        }
    }
}

#[cfg(test)]
mod tests {
    use cadmium_engine::stroke::{StrokeSample, Tool};
    use cadmium_engine::surface::Surface;

    use super::*;
    use crate::presets::starting_canvas_spec;
    use crate::state::tests::{
        WHITE_SLOT, brush_totals, painting_state, present_one_frame, wet_volume,
    };

    fn sample_at(x: f64, y: f64, pressure: f32) -> StrokeSample {
        StrokeSample {
            x,
            y,
            pressure,
            tilt_x_deg: 0.0,
            tilt_y_deg: 0.0,
            time_ms: 0,
        }
    }

    fn present_one_tray_frame(state: &mut AppState) {
        let gpu = state.gpu.clone();
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        state.encode_tray_frame(&mut encoder);
        gpu.queue.submit([encoder.finish()]);
    }

    fn squeeze_blob_on_tray(state: &mut AppState, slot: usize) {
        state.set_ui_tool(UiTool::Tube);
        state.set_tube_slot(slot);
        state.press(Surface::Tray, sample_at(300.0, 300.0, 1.0), false);
        state.advance_hold(1.0);
        state.release();
    }

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
            assert_eq!(state.load_ring().is_none(), cancels, "{drag}");
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
        assert!(state.load_ring().is_none());
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
    fn the_tube_falls_back_to_the_first_pigment_on_a_smaller_palette() {
        let mut state = painting_state();
        let last = state.slot_count() - 1;
        state.set_tube_slot(last);
        let mut spec = state.session.canvas.spec.clone();
        spec.pigment_slots.truncate(2);
        state.start_painting(spec).unwrap();
        assert_eq!(state.tube_slot, 0);
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
        assert!(state.is_holding());
        state.release();
        assert!(!state.is_stroking());
        assert!(!state.is_holding());
    }

    #[test]
    fn a_load_that_takes_nothing_is_not_an_edit() {
        let mut state = painting_state();
        let recipe = state.session.brush.recipe.clone();
        state.set_ui_tool(UiTool::Paint(Tool::Brush));
        state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
        state.advance_hold(0.2);
        assert_eq!(state.load_ring().unwrap().colour, LOAD_RING_EMPTY_COLOUR);
        state.release();
        assert!(!state.has_unsaved_edits());
        assert_eq!(state.session.brush.recipe, recipe);
    }

    #[test]
    fn dragging_past_four_pixels_turns_the_load_into_a_stroke() {
        let mut state = painting_state();
        squeeze_blob_on_tray(&mut state, 1);
        state.set_ui_tool(UiTool::Paint(Tool::Brush));
        state.press(Surface::Tray, sample_at(300.0, 300.0, 0.9), false);
        state.motion(&[sample_at(303.0, 300.0, 0.9)]);
        assert!(state.load_ring().is_some());
        state.motion(&[sample_at(310.0, 300.0, 0.9)]);
        assert!(state.load_ring().is_none());
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
    fn choosing_a_tube_does_not_change_the_brush() {
        let mut state = painting_state();
        let recipe = state.session.brush.recipe.clone();
        state.set_tube_slot(1);
        assert_eq!(state.session.brush.recipe, recipe);
        assert_eq!(state.tube_slot, 1);
    }

    #[test]
    fn a_tube_beyond_the_palette_is_ignored() {
        let mut state = painting_state();
        state.set_tube_slot(state.slot_count());
        assert_eq!(state.tube_slot, 0);
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

    #[test]
    fn undo_lifts_a_hold_that_is_still_down() {
        let mut state = painting_state();
        state.set_ui_tool(UiTool::Tube);
        state.press(Surface::Painting, sample_at(100.0, 100.0, 1.0), false);
        state.advance_hold(0.1);
        assert!(state.undo());
        assert!(!state.is_holding());
        assert_eq!(wet_volume(&state), 0.0);
    }

    #[test]
    fn clearing_the_tray_is_an_edit() {
        let mut state = painting_state();
        squeeze_blob_on_tray(&mut state, 0);
        state.clear_tray();
        assert_eq!(
            state.session.tray.wet_totals(&state.gpu).unwrap().volume,
            0.0
        );
        assert!(state.has_unsaved_edits());
    }

    #[test]
    fn clearing_a_clean_tray_is_not_an_edit() {
        let mut state = painting_state();
        state.clear_tray();
        assert!(!state.has_unsaved_edits());
    }

    #[test]
    fn a_new_painting_starts_with_a_clean_tray() {
        let mut state = painting_state();
        squeeze_blob_on_tray(&mut state, 0);
        state.start_painting(starting_canvas_spec()).unwrap();
        assert_eq!(
            state.session.tray.wet_totals(&state.gpu).unwrap().volume,
            0.0
        );
    }
}
