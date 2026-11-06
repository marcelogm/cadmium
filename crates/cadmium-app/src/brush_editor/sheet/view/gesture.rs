//! The sheet's own small gesture: a stylus or a mouse paints, with its pressure and tilt.

use gtk::glib;
use gtk::prelude::*;

use super::TestSheetView;
use crate::input::samples::{event_sample, is_primary_contact, motion_samples};

pub(super) fn attach(view: &TestSheetView) {
    let stylus = gtk::GestureStylus::new();
    stylus.set_stylus_only(false);
    stylus.connect_down(glib::clone!(
        #[weak]
        view,
        move |stylus, x, y| down(&view, stylus, x, y)
    ));
    stylus.connect_motion(glib::clone!(
        #[weak]
        view,
        move |stylus, x, y| motion(&view, stylus, x, y)
    ));
    stylus.connect_up(glib::clone!(
        #[weak]
        view,
        move |_, _, _| view.finish_stroke()
    ));
    stylus.connect_cancel(glib::clone!(
        #[weak]
        view,
        move |_, _| view.finish_stroke()
    ));
    view.add_controller(stylus);
    let pointer = gtk::EventControllerMotion::new();
    pointer.connect_leave(glib::clone!(
        #[weak]
        view,
        move |_| view.finish_stroke()
    ));
    view.add_controller(pointer);
}

fn down(view: &TestSheetView, stylus: &gtk::GestureStylus, x: f64, y: f64) {
    let Some(event) = stylus.current_event() else {
        return;
    };
    if is_primary_contact(&event) {
        view.begin_stroke(event_sample(&event, x, y, |x, y| view.canvas_point(x, y)));
    }
}

/// Motion also comes while a pen only hovers: the view ignores it then.
fn motion(view: &TestSheetView, stylus: &gtk::GestureStylus, x: f64, y: f64) {
    let Some(event) = stylus.current_event() else {
        return;
    };
    view.extend_stroke(&motion_samples(&event, x, y, |x, y| {
        view.canvas_point(x, y)
    }));
}
