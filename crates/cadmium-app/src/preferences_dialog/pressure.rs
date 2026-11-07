//! The Preferences group that shapes the pen's pressure: the curve editor, a reset, and a pad
//! to try it on.

mod editing;
mod editor;
mod plot;
mod scribble;

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use crate::pressure_curve::PressureCurve;

const SPACING: i32 = 12;

/// The group, starting from `current`; `save` keeps each finished change.
pub(super) fn pressure_group(
    current: PressureCurve,
    save: impl Fn(PressureCurve) + 'static,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("Pen Pressure")
        .description(
            "How hard the brush presses for how hard you press. Drag a point to move it, \
             press on empty space to add one, double-click to remove it.",
        )
        .build();
    let save = Rc::new(save);
    let curve = Rc::new(Cell::new(current));
    let editor = editor::curve_editor(
        curve.clone(),
        glib::clone!(
            #[strong]
            save,
            move |changed| save(changed)
        ),
    );
    let reset = gtk::Button::with_label("Reset to Linear");
    reset.connect_clicked(glib::clone!(
        #[strong]
        curve,
        #[weak]
        editor,
        move |_| {
            if curve.get() != PressureCurve::linear() {
                curve.set(PressureCurve::linear());
                editor.queue_draw();
                save(PressureCurve::linear());
            }
        }
    ));
    let (pad, clear_pad) = scribble::scribble_pad(curve);
    let clear = gtk::Button::with_label("Clear");
    clear.connect_clicked(move |_| clear_pad());
    group.add(&layout(&editor, &reset, &pad, &clear));
    group
}

/// The editor over its reset button, beside the pad over its clear button.
fn layout(
    editor: &gtk::DrawingArea,
    reset: &gtk::Button,
    pad: &gtk::DrawingArea,
    clear: &gtk::Button,
) -> gtk::Box {
    let column = |area: &gtk::DrawingArea, button: &gtk::Button| {
        let column = gtk::Box::new(gtk::Orientation::Vertical, SPACING);
        column.append(area);
        column.append(button);
        column
    };
    let row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(SPACING)
        .homogeneous(true)
        .build();
    row.append(&column(editor, reset));
    row.append(&column(pad, clear));
    row
}
