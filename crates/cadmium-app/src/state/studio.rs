//! The 3D brush studio: the worn brush painting a looping demo stroke.

use cadmium_engine::brush::BrushSpec;
use cadmium_engine::engine::PaintEngine;
use cadmium_engine::gpu::GpuContext;
use cadmium_studio::{
    DemoStroke, OrbitCamera, PlayerRequest, SceneLayers, SceneRenderer, SceneRequest, StudioError,
    StudioPaint, StudioPlayer,
};

use super::AppState;
use crate::presets::WHITES;

/// The name of a worn brush that is not in the library.
const CURRENT_BRUSH_NAME: &str = "Current Brush";
/// The longest real time one frame advances the player by; a longer stall
/// (a hidden window, a debugger) resumes where it stopped. It binds only at
/// slow speeds: from speed 0.48 up the player's cap of 8 quanta (48 ms of
/// simulation) per advance binds first.
const MAX_FRAME_MS: f64 = 100.0;
const FALLBACK_LOAD: f32 = 1.0;
const DEMO_STROKE: DemoStroke = DemoStroke::SCurve;

/// The open studio: its player, the scene it is drawn in and the camera.
pub struct StudioSession {
    pub player: StudioPlayer,
    pub scene: SceneRenderer,
    pub camera: OrbitCamera,
    /// When the last frame advanced the player.
    last_ms: Option<f64>,
    /// The viewport's width / height at the last present.
    aspect: f32,
}

impl StudioSession {
    fn new(gpu: &GpuContext, request: PlayerRequest<'_>) -> Result<StudioSession, StudioError> {
        let player = StudioPlayer::new(gpu, request)?;
        let camera = OrbitCamera::framing(player.stroke_box(), player.reach_px(), 1.0);
        Ok(StudioSession {
            player,
            scene: SceneRenderer::new(gpu),
            camera,
            last_ms: None,
            aspect: 1.0,
        })
    }

    fn set_aspect(&mut self, aspect: f32) {
        self.aspect = aspect;
        self.camera.set_aspect(aspect);
    }

    /// The reset view framing the player's stroke box and brush.
    fn reframe(&mut self) {
        let player = &self.player;
        self.camera = OrbitCamera::framing(player.stroke_box(), player.reach_px(), self.aspect);
    }

    /// The real time since the last frame, clamped to 0..=`MAX_FRAME_MS`.
    fn elapsed_ms(&mut self, now_ms: f64) -> f64 {
        let elapsed_ms = self.last_ms.map_or(0.0, |last_ms| now_ms - last_ms);
        self.last_ms = Some(now_ms);
        elapsed_ms.clamp(0.0, MAX_FRAME_MS)
    }

    /// Restarts the loop with `paint`, keeping the brush and the stroke.
    fn repaint(&mut self, gpu: &GpuContext, paint: &StudioPaint) -> Result<(), StudioError> {
        let brush = self.player.brush().clone();
        let request = PlayerRequest {
            brush: &brush,
            paint,
            stroke: self.player.stroke(),
        };
        self.player.restart(gpu, request)
    }
}

impl AppState {
    /// The worn brush as a library brush at the current size.
    pub fn worn_brush_spec(&self) -> BrushSpec {
        let settings = &self.session.brush;
        let size_px = settings.geometry.max_diameter_px;
        match self.worn_library_brush().and_then(|i| self.brushes.get(i)) {
            Some(brush) => BrushSpec {
                size_px,
                ..brush.clone()
            },
            None => BrushSpec {
                name: CURRENT_BRUSH_NAME.into(),
                size_px,
                head: settings.head,
                hair: settings.hair,
            },
        }
    }

    /// The painting's canvas and paint, with a visible colour when the brush is
    /// empty: the first non-white slot, fully loaded.
    pub fn studio_paint(&self) -> StudioPaint {
        let spec = &self.session.canvas.spec;
        let brush = &self.session.brush;
        let empty = brush.recipe.is_empty() || brush.load <= 0.0;
        let (recipe, load) = if empty {
            (
                vec![(first_colour(&spec.pigment_slots), 1.0)],
                FALLBACK_LOAD,
            )
        } else {
            (brush.recipe.clone(), brush.load)
        };
        StudioPaint {
            slots: spec.pigment_slots.clone(),
            primer: spec.primer,
            weave: spec.weave,
            recipe,
            load,
            medium: brush.medium,
            params: self.session.engine.params().clone(),
        }
    }

    /// Opens the studio on the worn brush and wakes its views; an error is kept
    /// for `take_studio_error`.
    pub fn open_studio(&mut self) {
        let brush = self.worn_brush_spec();
        let paint = self.studio_paint();
        let request = PlayerRequest {
            brush: &brush,
            paint: &paint,
            stroke: DEMO_STROKE,
        };
        self.studio_error = None;
        match StudioSession::new(&self.gpu, request) {
            Ok(studio) => self.studio = Some(studio),
            Err(error) => self.fail_studio(error),
        }
        self.wake();
    }

    /// Closes the studio, freeing its canvas and scene; its views drop their frame.
    pub fn close_studio(&mut self) {
        self.studio = None;
        self.wake();
    }

    pub fn studio_open(&self) -> bool {
        self.studio.is_some()
    }

    pub fn studio(&self) -> Option<&StudioSession> {
        self.studio.as_ref()
    }

    pub fn studio_mut(&mut self) -> Option<&mut StudioSession> {
        self.studio.as_mut()
    }

    /// The error that closed the studio, once.
    pub fn take_studio_error(&mut self) -> Option<StudioError> {
        self.studio_error.take()
    }

    /// Advances the open studio by the real time since its last frame,
    /// restarting it first when the painting's paint changed.
    ///
    /// Only `encode_studio_present` calls this, so every error closes the studio.
    fn encode_studio_frame(&mut self) -> Result<(), StudioError> {
        if self.studio.is_none() {
            return Ok(());
        }
        let now_ms = self.now_ms();
        let paint = self.studio_paint();
        let AppState { studio, gpu, .. } = self;
        let Some(studio) = studio else {
            return Ok(());
        };
        if *studio.player.paint() != paint {
            studio.repaint(gpu, &paint)?;
        }
        let elapsed_ms = studio.elapsed_ms(now_ms);
        studio.player.advance(gpu, elapsed_ms)
    }

    /// Draws the studio scene into `target`.
    ///
    /// Only an open studio is presented: the studio widget presents only while
    /// it is open, so a closed studio leaves `target` untouched. An error
    /// closes the studio and is kept for `take_studio_error`.
    pub fn encode_studio_present(
        &mut self,
        gpu: &GpuContext,
        target: &wgpu::Texture,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if let Err(error) = self.encode_studio_frame() {
            self.fail_studio(error);
        }
        let AppState {
            studio: Some(studio),
            session,
            ..
        } = self
        else {
            return;
        };
        let size = (target.width(), target.height());
        studio.set_aspect(size.0 as f32 / size.1.max(1) as f32);
        let request = SceneRequest {
            player: &studio.player,
            camera: &studio.camera,
            size,
            layers: SceneLayers { brush: true },
        };
        let frame = studio.scene.render(gpu, request, encoder);
        session
            .presenter
            .encode_present(gpu, frame.present_request(), target, encoder);
    }

    /// Plays `stroke` from the start of a loop, keeping the brush, the speed,
    /// the pause and the view: the stroke box depends only on the brush.
    pub fn set_studio_stroke(&mut self, stroke: DemoStroke) {
        let Some(brush) = self.studio.as_ref().map(|s| s.player.brush().clone()) else {
            return;
        };
        self.restart_studio(&brush, stroke);
    }

    /// Shows `brush` from the start of a loop, keeping the stroke, the speed
    /// and the pause; the view is reset to frame the new brush.
    pub fn set_studio_brush(&mut self, brush: &BrushSpec) {
        let Some(stroke) = self.studio.as_ref().map(|s| s.player.stroke()) else {
            return;
        };
        self.restart_studio(brush, stroke);
        if let Some(studio) = self.studio.as_mut() {
            studio.reframe();
        }
    }

    /// Restarts the open studio with the painting's paint; an error closes it.
    fn restart_studio(&mut self, brush: &BrushSpec, stroke: DemoStroke) {
        let paint = self.studio_paint();
        let AppState { studio, gpu, .. } = self;
        let Some(studio) = studio else {
            return;
        };
        let request = PlayerRequest {
            brush,
            paint: &paint,
            stroke,
        };
        if let Err(error) = studio.player.restart(gpu, request) {
            self.fail_studio(error);
        }
    }

    fn fail_studio(&mut self, error: StudioError) {
        self.studio = None;
        self.studio_error = Some(error);
    }
}

/// The first slot that is not a white, or slot 0 when all are.
fn first_colour(slots: &[String]) -> usize {
    slots
        .iter()
        .position(|id| !WHITES.contains(&id.as_str()))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
