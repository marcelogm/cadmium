//! The test sheet's widget: the sheet's canvas covering it edge to edge under faint grid
//! lines, painted by hand with a stylus or a mouse. The editor owns the sheet; the widget links to
//! it weakly, so once the editor lets it go the widget presents nothing more.

use std::cell::RefCell;
use std::rc::Rc;

use cadmium_engine::gpu::GpuContext;
use cadmium_engine::stroke::StrokeSample;
use cadmium_studio::TestSheet;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk};

use crate::handlers::run_handlers;
use crate::input::event_belongs_to_stroke;
use crate::presenter::{FramePresenter, PresentSource, SheetLink};
use crate::state::SharedState;
use crate::surface_scale::{device_scale, viewport_px};
use crate::view::{cover_view, viewport_to_canvas};

mod frames;
mod gesture;
mod imp;

const CURSOR: &str = "crosshair";
const DESCRIPTION: &str = "Test sheet: paint on it to try the brush";

/// What the widget paints on and presents with.
struct Link {
    state: SharedState,
    gpu: Rc<GpuContext>,
    sheet: SheetLink,
    /// The sheet's canvas size, in canvas px.
    size: (u32, u32),
}

// `pub`, as `TrayView` is: `type Type` in `imp` must not name a more private type. The
// `view` module is private, so it is still out of reach outside the editor.
glib::wrapper! {
    pub struct TestSheetView(ObjectSubclass<imp::TestSheetView>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl TestSheetView {
    /// A view of `sheet`, presented with `state`'s GPU and clock; it holds the sheet weakly.
    pub(in crate::brush_editor) fn new(
        state: SharedState,
        sheet: &Rc<RefCell<TestSheet>>,
    ) -> TestSheetView {
        let view: TestSheetView = glib::Object::new();
        let gpu = state.borrow().gpu.clone();
        let size = sheet.borrow().size();
        let link = Link {
            state,
            gpu,
            sheet: Rc::downgrade(sheet),
            size,
        };
        let source = PresentSource::TestSheet(link.sheet.clone());
        view.imp()
            .presenter
            .replace(FramePresenter::showing(source));
        if view.imp().link.set(link).is_err() {
            unreachable!("a fresh test sheet view has no link yet");
        }
        gesture::attach(&view);
        view
    }

    pub(in crate::brush_editor) fn connect_gpu_lost(
        &self,
        handler: impl Fn(&TestSheetView) + 'static,
    ) {
        self.imp()
            .gpu_lost_handlers
            .borrow_mut()
            .push(Box::new(handler));
    }

    pub(in crate::brush_editor) fn connect_presentation_fallback(
        &self,
        handler: impl Fn(&TestSheetView) + 'static,
    ) {
        self.imp()
            .fallback_handlers
            .borrow_mut()
            .push(Box::new(handler));
    }

    /// Runs as each stroke begins on the sheet.
    pub(in crate::brush_editor) fn connect_stroke_began(
        &self,
        handler: impl Fn(&TestSheetView) + 'static,
    ) {
        self.imp()
            .stroke_began_handlers
            .borrow_mut()
            .push(Box::new(handler));
    }

    /// The sheet, while the editor still has it.
    fn sheet(&self) -> Option<Rc<RefCell<TestSheet>>> {
        self.imp().link.get()?.sheet.upgrade()
    }

    /// Where a widget point lands on the sheet's canvas: through the cover it is shown with.
    fn canvas_point(&self, widget_x: f64, widget_y: f64) -> (f64, f64) {
        let size = self.imp().link.get().map_or((1, 1), |link| link.size);
        let scale = device_scale(self);
        let view = cover_view(size, viewport_px(self));
        viewport_to_canvas(view, widget_x * scale, widget_y * scale)
    }

    /// Starts a stroke at `sample`, painted by `device`; the sheet dips the brush afresh first.
    fn begin_stroke(&self, sample: StrokeSample, device: Option<gdk::Device>) {
        let imp = self.imp();
        let (Some(link), Some(sheet)) = (imp.link.get(), self.sheet()) else {
            return;
        };
        sheet.borrow_mut().begin_stroke(&link.gpu, sample);
        imp.stroking.set(true);
        imp.stroking_device.replace(device);
        self.request_frame();
        run_handlers(&imp.stroke_began_handlers, self);
    }

    fn extend_stroke(&self, samples: &[StrokeSample]) {
        if !self.imp().stroking.get() {
            return;
        }
        if let Some(sheet) = self.sheet() {
            sheet.borrow_mut().add_samples(samples);
        }
        self.request_frame();
    }

    /// Ends the stroke under way, if any.
    fn finish_stroke(&self) {
        let imp = self.imp();
        imp.stroking_device.replace(None);
        if !imp.stroking.replace(false) {
            return;
        }
        if let Some(sheet) = self.sheet() {
            sheet.borrow_mut().end_stroke();
        }
        self.request_frame();
    }

    /// Ends the stroke under way if `device` paints it: another pointer leaving leaves it be.
    fn finish_stroke_of(&self, device: Option<&gdk::Device>) {
        let stroking = self.imp().stroking_device.borrow().clone();
        if event_belongs_to_stroke(stroking.as_ref(), device) {
            self.finish_stroke();
        }
    }
}

/// Whether the view must present another frame: only while the sheet is there (`painting`
/// is `None` once the editor let it go), and then while it paints or settles, for a queued
/// present, or when the widget's pixel size no longer matches the presented texture.
fn frame_needed(painting: Option<bool>, needs_present: bool, size_matches: bool) -> bool {
    painting.is_some_and(|painting| painting || needs_present || !size_matches)
}

/// The presented canvas over the whole widget (it covers its target edge to edge).
fn append_sheet(snapshot: &gtk::Snapshot, texture: &gdk::Texture, scale: f64) {
    let scale = scale as f32;
    let bounds = graphene::Rect::new(
        0.0,
        0.0,
        texture.width() as f32 / scale,
        texture.height() as f32 / scale,
    );
    snapshot.append_scaled_texture(texture, gsk::ScalingFilter::Linear, &bounds);
}

#[cfg(test)]
mod tests;
