//! The widget showing the 3D brush studio: the worn brush painting its demo
//! stroke, orbited by dragging and zoomed by scrolling.

use std::cell::{Cell, OnceCell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk};

use crate::presenter::{FramePresenter, PresentSource};
use crate::state::SharedState;
use crate::surface_scale::{SurfaceScaleWatch, device_scale};

const STUDIO_MIN_WIDTH: i32 = 360;
const STUDIO_MIN_HEIGHT: i32 = 240;
const STUDIO_CURSOR: &str = "grab";
const STUDIO_FRAME_CLASS: &str = "card";

type StudioHandler = Box<dyn Fn(&StudioView)>;

mod frames;
mod imp;

glib::wrapper! {
    pub struct StudioView(ObjectSubclass<imp::StudioView>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl StudioView {
    pub fn new(state: SharedState) -> Self {
        let view: StudioView = glib::Object::new();
        let weak = view.downgrade();
        state
            .borrow_mut()
            .add_waker(Rc::new(move || wake_studio(&weak)));
        if view.imp().state.set(state).is_err() {
            unreachable!("a fresh studio view has no state yet");
        }
        view.attach_camera_controls();
        view
    }

    pub fn state(&self) -> SharedState {
        self.imp()
            .state
            .get()
            .expect("studio view is created with its state")
            .clone()
    }

    pub fn presented_frames(&self) -> u64 {
        self.imp().presented_frames.get()
    }

    pub fn connect_gpu_lost(&self, handler: impl Fn(&StudioView) + 'static) {
        self.imp()
            .gpu_lost_handlers
            .borrow_mut()
            .push(Box::new(handler));
    }

    /// Runs only when the studio fails while presenting a frame, which closes
    /// it; `AppState::take_studio_error` says why. A normal `close_studio`
    /// does not run it.
    pub fn connect_studio_failed(&self, handler: impl Fn(&StudioView) + 'static) {
        self.imp()
            .studio_failed_handlers
            .borrow_mut()
            .push(Box::new(handler));
    }

    pub fn connect_presentation_fallback(&self, handler: impl Fn(&StudioView) + 'static) {
        self.imp()
            .fallback_handlers
            .borrow_mut()
            .push(Box::new(handler));
    }

    fn device_scale(&self) -> f64 {
        device_scale(self)
    }

    fn follow_scale_change(&self) {
        self.queue_present();
    }

    fn attach_camera_controls(&self) {
        let drag = gtk::GestureDrag::new();
        drag.set_button(0);
        drag.connect_drag_begin(glib::clone!(
            #[weak(rename_to = view)]
            self,
            move |_, _, _| view.imp().drag_offset.set((0.0, 0.0))
        ));
        drag.connect_drag_update(glib::clone!(
            #[weak(rename_to = view)]
            self,
            move |_, x, y| view.follow_drag(x, y)
        ));
        self.add_controller(drag);
        let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        scroll.connect_scroll(glib::clone!(
            #[weak(rename_to = view)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, _, dy| view.zoom_by(dy)
        ));
        self.add_controller(scroll);
    }

    /// Orbits by the drag's movement since its last update.
    fn follow_drag(&self, offset_x: f64, offset_y: f64) {
        let (last_x, last_y) = self.imp().drag_offset.replace((offset_x, offset_y));
        let (dx, dy) = ((offset_x - last_x) as f32, (offset_y - last_y) as f32);
        self.move_camera(|camera| camera.orbit(dx, dy));
    }

    fn zoom_by(&self, dy: f64) -> glib::Propagation {
        self.move_camera(|camera| camera.zoom(-dy as f32));
        glib::Propagation::Stop
    }

    fn move_camera(&self, change: impl FnOnce(&mut cadmium_studio::OrbitCamera)) {
        let Some(state) = self.imp().state.get() else {
            return;
        };
        // A move made while the state is busy is dropped; the next event moves again.
        let Ok(mut state) = state.try_borrow_mut() else {
            return;
        };
        if let Some(studio) = state.studio_mut() {
            change(&mut studio.camera);
        }
        drop(state);
        self.queue_present();
    }
}

/// Whether the studio view must present another frame: only while the studio
/// is open, so readback never hands GTK a texture nothing was drawn into, and
/// then while it plays, for a queued present, or when the widget's pixel size
/// no longer matches the presented texture.
pub(crate) fn frame_needed(
    studio_open: bool,
    playing: bool,
    needs_present: bool,
    size_matches: bool,
) -> bool {
    studio_open && (playing || needs_present || !size_matches)
}

fn wake_studio(view: &glib::WeakRef<StudioView>) {
    if let Some(view) = view.upgrade() {
        view.request_frame();
        view.queue_draw();
    }
}

fn append_studio_image(snapshot: &gtk::Snapshot, texture: &gdk::Texture, scale: f64) {
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
