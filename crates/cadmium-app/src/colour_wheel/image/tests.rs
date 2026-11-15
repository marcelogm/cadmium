use super::*;
use cadmium_engine::palette::{ColourRef, circle_axes, paint_recipe};

const NEAR_WHITE_LEVELS: u8 = 40;

#[test]
fn the_image_is_a_disc_whose_centre_is_the_white_and_whose_top_is_the_first_colour() {
    let db = PigmentDb::builtin();
    let slots: Vec<String> = ["cadmium_red", "ultramarine_blue", "titanium_white"]
        .map(String::from)
        .to_vec();
    let palette = Palette {
        my_colours: vec![],
        circle: vec![ColourRef::Paint(0), ColourRef::Paint(1)],
    };
    let image = wheel_image(&WheelPaint {
        db: &db,
        slots: &slots,
        palette: &palette,
        axes: circle_axes(&slots),
        darken: 0.0,
    });
    let cell = |column: usize, row: usize| image.cells[row * WHEEL_CELLS + column];
    assert_eq!(image.cells.len(), WHEEL_CELLS * WHEEL_CELLS);
    assert_eq!(cell(0, 0), None);
    let white = mixture_srgb8(&db, &slots, &paint_recipe(2));
    let centre = cell(WHEEL_CELLS / 2, WHEEL_CELLS / 2).unwrap();
    assert!(
        centre
            .iter()
            .zip(white)
            .all(|(a, b)| a.abs_diff(b) <= NEAR_WHITE_LEVELS),
        "{centre:?} vs {white:?}"
    );
    let top = cell(WHEEL_CELLS / 2, 0).unwrap();
    let red = db.masstone_srgb8("cadmium_red");
    assert!(
        top.iter().zip(red).all(|(a, b)| a.abs_diff(b) <= 4),
        "{top:?} vs {red:?}"
    );
    assert_eq!(image.rim, [red, db.masstone_srgb8("ultramarine_blue")]);
}

#[test]
fn darkening_without_a_black_on_the_painting_changes_nothing() {
    let db = PigmentDb::builtin();
    let slots: Vec<String> = ["cadmium_red", "ultramarine_blue", "titanium_white"]
        .map(String::from)
        .to_vec();
    let palette = Palette {
        my_colours: vec![],
        circle: vec![ColourRef::Paint(0), ColourRef::Paint(1)],
    };
    let image_at = |darken: f32| {
        wheel_image(&WheelPaint {
            db: &db,
            slots: &slots,
            palette: &palette,
            axes: circle_axes(&slots),
            darken,
        })
    };
    assert_eq!(image_at(0.8), image_at(0.0));
}

#[test]
fn an_empty_circle_draws_no_cell() {
    let db = PigmentDb::builtin();
    let slots = vec!["titanium_white".to_string()];
    let image = wheel_image(&WheelPaint {
        db: &db,
        slots: &slots,
        palette: &Palette::default(),
        axes: circle_axes(&slots),
        darken: 0.0,
    });
    assert!(image.cells.iter().all(Option::is_none));
    assert!(image.rim.is_empty());
}

#[test]
fn the_cairo_pixels_are_opaque_colour_inside_the_disc_and_transparent_outside() {
    let mut cells = vec![None; WHEEL_CELLS * WHEEL_CELLS];
    cells[WHEEL_CELLS + 2] = Some([0x11, 0x22, 0x33]);
    let image = WheelImage { cells, rim: vec![] };
    let stride = WHEEL_CELLS * 4 + 8;
    let bytes = image.argb32(stride);
    assert_eq!(bytes.len(), stride * WHEEL_CELLS);
    let pixel = |column: usize, row: usize| {
        let at = row * stride + column * 4;
        u32::from_ne_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
    };
    assert_eq!(pixel(2, 1), 0xFF11_2233);
    assert_eq!(pixel(1, 2), 0);
    assert_eq!(bytes.iter().filter(|byte| **byte != 0).count(), 4);
    assert!(image.has_colour());
    assert!(
        !WheelImage {
            cells: vec![None; 4],
            rim: vec![]
        }
        .has_colour()
    );
}

#[test]
fn a_circle_naming_a_colour_that_is_gone_draws_no_cell_and_no_rim() {
    let db = PigmentDb::builtin();
    let slots: Vec<String> = ["cadmium_red", "titanium_white"].map(String::from).to_vec();
    let palette = Palette {
        my_colours: vec![],
        circle: vec![ColourRef::Paint(0), ColourRef::Mine(3)],
    };
    let image = wheel_image(&WheelPaint {
        db: &db,
        slots: &slots,
        palette: &palette,
        axes: circle_axes(&slots),
        darken: 0.0,
    });
    assert!(!image.has_colour());
    assert!(image.rim.is_empty());
}

#[test]
fn a_painting_with_neither_white_nor_black_still_draws_its_circle() {
    let db = PigmentDb::builtin();
    let slots = vec!["ultramarine_blue".to_string()];
    let palette = Palette {
        my_colours: vec![],
        circle: vec![ColourRef::Paint(0)],
    };
    let image = wheel_image(&WheelPaint {
        db: &db,
        slots: &slots,
        palette: &palette,
        axes: circle_axes(&slots),
        darken: 0.5,
    });
    let blue = db.masstone_srgb8("ultramarine_blue");
    let inside = image.cells.iter().flatten();
    assert!(inside.clone().count() > WHEEL_CELLS * WHEEL_CELLS / 2);
    assert!(inside.clone().all(|cell| *cell == blue));
}

#[test]
fn darkening_with_a_black_on_the_painting_darkens_the_wheel() {
    let db = PigmentDb::builtin();
    let slots: Vec<String> = ["cadmium_red", "ivory_black"].map(String::from).to_vec();
    let palette = Palette {
        my_colours: vec![],
        circle: vec![ColourRef::Paint(0)],
    };
    let top_at = |darken: f32| {
        let image = wheel_image(&WheelPaint {
            db: &db,
            slots: &slots,
            palette: &palette,
            axes: circle_axes(&slots),
            darken,
        });
        image.cells[WHEEL_CELLS / 2].unwrap()
    };
    assert!(top_at(0.8)[0] < top_at(0.0)[0]);
}

#[test]
fn the_disc_is_centred_on_the_grid() {
    let db = PigmentDb::builtin();
    let slots = vec!["ultramarine_blue".to_string()];
    let palette = Palette {
        my_colours: vec![],
        circle: vec![ColourRef::Paint(0)],
    };
    let image = wheel_image(&WheelPaint {
        db: &db,
        slots: &slots,
        palette: &palette,
        axes: circle_axes(&slots),
        darken: 0.0,
    });
    let inside = |column: usize, row: usize| image.cells[row * WHEEL_CELLS + column].is_some();
    let last = WHEEL_CELLS - 1;
    for (column, row) in
        (0..WHEEL_CELLS).flat_map(|row| (0..WHEEL_CELLS).map(move |column| (column, row)))
    {
        assert_eq!(
            inside(column, row),
            inside(last - column, row),
            "({column}, {row})"
        );
        assert_eq!(
            inside(column, row),
            inside(column, last - row),
            "({column}, {row})"
        );
    }
}
