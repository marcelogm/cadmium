//! The title block under the drawing: the brush's name across the top, then what it is made
//! of, two cells to a row.

use super::super::controls::CUSTOM;
use super::Subject;
use super::canvas::{Canvas, Part};
use super::pen::{self, Anchor, INK, Lettering, Pen, THIN};
use super::sheet::{SHEET, TITLE_TOP};

const ROWS: usize = 4;
const ROW_HEIGHT: f64 = 22.0;
const LEFT: f64 = 6.0;
const MIDDLE: f64 = 160.0;
const TEXT_INSET: f64 = 6.0;
const KEY_DROP: f64 = 8.0;
const VALUE_DROP: f64 = 18.0;
const FRAME_PEN: Pen = Pen::new(INK, 0.8);
const RULE_PEN: Pen = Pen::new(INK, 0.6);
/// The longest name the top row shows before it is cut.
const MAX_NAME_CHARS: usize = 48;
const KEY: Lettering = Lettering {
    size: 6.0,
    spacing_em: 0.16,
    colour: THIN,
    anchor: Anchor::Start,
};
const VALUE: Lettering = Lettering {
    size: 9.0,
    spacing_em: 0.04,
    colour: INK,
    anchor: Anchor::Start,
};

pub(super) type Cell = (&'static str, String);

/// The cells' keys and values: the name first, then the rest left to right, row by row.
pub(super) fn cells(subject: &Subject) -> [Cell; 7] {
    let brush = subject.brush;
    let head = &brush.head;
    [
        ("NAME", clipped(&brush.name, MAX_NAME_CHARS)),
        ("HEAD", subject.head.unwrap_or(CUSTOM).to_uppercase()),
        ("HAIR", subject.hair.unwrap_or(CUSTOM).to_uppercase()),
        (
            "LENGTH · THICKNESS",
            format!("{:.2}R · {:.2}", head.length, head.thickness),
        ),
        (
            "FAN · SLANT",
            format!("{:.0}° · {:.2}", head.fan_deg, head.slant),
        ),
        ("SUGGESTED SIZE", format!("{:.0} px", brush.size_px)),
        ("DENSITY", format!("{:.1} /px", head.density_per_px)),
    ]
}

pub(super) fn draw(canvas: &Canvas, cells: &[Cell; 7]) {
    frame(canvas);
    let [name, rest @ ..] = cells;
    cell(canvas, [LEFT, TITLE_TOP], name);
    for (index, entry) in rest.iter().enumerate() {
        let x = if index % 2 == 0 { LEFT } else { MIDDLE };
        let y = TITLE_TOP + (1 + index / 2) as f64 * ROW_HEIGHT;
        cell(canvas, [x, y], entry);
    }
}

fn frame(canvas: &Canvas) {
    let right = SHEET[0] - LEFT;
    let bottom = TITLE_TOP + ROWS as f64 * ROW_HEIGHT;
    pen::rectangle(canvas, [LEFT, TITLE_TOP], [right, bottom], FRAME_PEN);
    for row in 1..ROWS {
        let y = TITLE_TOP + row as f64 * ROW_HEIGHT;
        pen::line(canvas, [LEFT, y], [right, y], RULE_PEN);
    }
    let below_name = TITLE_TOP + ROW_HEIGHT;
    pen::line(canvas, [MIDDLE, below_name], [MIDDLE, bottom], RULE_PEN);
    let bounds = [[LEFT, TITLE_TOP], [right, bottom]];
    canvas.mark(Part::TitleBlock, canvas.on_widget(bounds));
}

fn cell(canvas: &Canvas, at: [f64; 2], entry: &Cell) {
    let (key, value) = entry;
    pen::write(canvas, [at[0] + TEXT_INSET, at[1] + KEY_DROP], key, &KEY);
    pen::write(
        canvas,
        [at[0] + TEXT_INSET, at[1] + VALUE_DROP],
        value,
        &VALUE,
    );
}

/// `text`, cut to `max` characters with an ellipsis when longer.
fn clipped(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut cut: String = text.chars().take(max - 1).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests;
