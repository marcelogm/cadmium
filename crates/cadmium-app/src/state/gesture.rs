use cadmium_engine::stroke::{StrokeSample, Tool};
use cadmium_engine::surface::Surface;

mod contact;
mod hold;
mod press;
mod stroke;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoadRing {
    pub level: f32,
    pub pressure: f32,
    pub colour: [u8; 3],
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Contact {
    Stroke { surface: Surface },
    Loading(Loading),
    Squeezing(Squeezing),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Loading {
    tool: Tool,
    origin: (f64, f64),
    pen: StrokeSample,
    colour: Option<[u8; 3]>,
    level: Option<f32>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Squeezing {
    surface: Surface,
    pen: StrokeSample,
    nozzle_at: (f64, f64),
    squeezed: bool,
}
