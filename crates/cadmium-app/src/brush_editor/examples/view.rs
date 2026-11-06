//! The example strokes' picture: the sheet over blank paper (shown until the first sheet
//! is painted) in a rounded card, with two labels over it.

use adw::prelude::*;
use gtk::{gdk, glib};

use crate::brush_editor::paper;

/// How faint the old picture is while the new one is painted.
const DIMMED: f64 = 0.45;
const LABEL_COLOUR: &str = "#8b8b84";
const LABEL_LEFT: i32 = 10;
/// Each label and its top, in widget px: "STROKE" in the top-left corner; the ramp's under
/// the S-curve's lowest reach and above the ramp's light end.
const LABELS: [(&str, i32); 2] = [("STROKE", 8), ("PRESSURE · LIGHT → HEAVY", 86)];
/// The arrow, which the monospace face draws too small at the labels' size, in a sans face.
const ARROW: &str = "→";
const SANS_ARROW: &str = "<span font_family=\"sans\">→</span>";
const DESCRIPTION: &str = "Example strokes: an S-curve, and a line pressed from light to heavy";

pub(in crate::brush_editor) struct ExamplesView {
    root: gtk::Overlay,
    picture: gtk::Picture,
}

impl ExamplesView {
    pub(in crate::brush_editor) fn new() -> ExamplesView {
        let picture = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Contain)
            .can_shrink(true)
            .build();
        let root = gtk::Overlay::builder()
            .child(&paper::examples_paper())
            .overflow(gtk::Overflow::Hidden)
            .accessible_role(gtk::AccessibleRole::Img)
            .build();
        root.update_property(&[gtk::accessible::Property::Label(DESCRIPTION)]);
        root.add_css_class("card");
        root.add_overlay(&picture);
        for (text, top) in LABELS {
            root.add_overlay(&label(text, top));
        }
        ExamplesView { root, picture }
    }

    pub(in crate::brush_editor) fn widget(&self) -> &gtk::Overlay {
        &self.root
    }

    pub(in crate::brush_editor) fn show(&self, sheet: &gdk::Texture) {
        self.picture.set_paintable(Some(sheet));
        self.picture.set_opacity(1.0);
    }

    pub(in crate::brush_editor) fn dim(&self) {
        self.picture.set_opacity(DIMMED);
    }
}

fn label(text: &str, top: i32) -> gtk::Label {
    gtk::Label::builder()
        .use_markup(true)
        .label(markup(text))
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Start)
        .margin_start(LABEL_LEFT)
        .margin_top(top)
        .can_target(false)
        .accessible_role(gtk::AccessibleRole::Presentation)
        .build()
}

/// Uppercase mono, 9.5 px (7 pt), weight 500, letter-spaced, grey; the arrow in sans.
fn markup(text: &str) -> String {
    let text = glib::markup_escape_text(text).replace(ARROW, SANS_ARROW);
    format!(
        "<span font_family=\"monospace\" size=\"7pt\" weight=\"500\" letter_spacing=\"1024\" \
         foreground=\"{LABEL_COLOUR}\">{text}</span>"
    )
}
