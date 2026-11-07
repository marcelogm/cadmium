use std::cell::RefCell;

use gtk::prelude::*;
use gtk::{gdk, glib};

pub(crate) fn device_scale(widget: &impl IsA<gtk::Widget>) -> f64 {
    widget
        .native()
        .and_then(|native| native.surface())
        .map_or(f64::from(widget.scale_factor()), |surface| surface.scale())
}

/// The widget's centre, in widget px.
pub(crate) fn widget_centre(widget: &impl IsA<gtk::Widget>) -> (f64, f64) {
    (
        f64::from(widget.width()) / 2.0,
        f64::from(widget.height()) / 2.0,
    )
}

pub(crate) fn viewport_px(widget: &impl IsA<gtk::Widget>) -> (u32, u32) {
    let scale = device_scale(widget);
    (
        (f64::from(widget.width()) * scale).ceil() as u32,
        (f64::from(widget.height()) * scale).ceil() as u32,
    )
}

#[derive(Default)]
pub struct SurfaceScaleWatch {
    watched: RefCell<Option<(glib::WeakRef<gdk::Surface>, glib::SignalHandlerId)>>,
}

impl SurfaceScaleWatch {
    pub(crate) fn start<W: IsA<gtk::Widget>>(&self, widget: &W, on_change: fn(&W)) {
        self.stop();
        let Some(surface) = widget.native().and_then(|native| native.surface()) else {
            return;
        };
        let widget = widget.downgrade();
        let handler =
            surface.connect_scale_notify(move |_| notify_scale_change(&widget, on_change));
        self.watched.replace(Some((surface.downgrade(), handler)));
    }

    pub(crate) fn stop(&self) {
        if let Some((surface, handler)) = self.watched.take()
            && let Some(surface) = surface.upgrade()
        {
            surface.disconnect(handler);
        }
    }
}

fn notify_scale_change<W: IsA<gtk::Widget>>(widget: &glib::WeakRef<W>, on_change: fn(&W)) {
    if let Some(widget) = widget.upgrade() {
        on_change(&widget);
    }
}
