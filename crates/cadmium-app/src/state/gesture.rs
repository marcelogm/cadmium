use cadmium_engine::surface::Surface;

mod contact;
mod press;
mod stroke;

#[derive(Clone, Copy, Debug)]
pub(super) enum Contact {
    Stroke { surface: Surface },
}
