use cadmium_engine::brush::{HairSpec, HeadSpec, factory_brushes};
use cadmium_studio::TipGeometry;

use super::{
    CAPTION_HEIGHT, FIGURES_BELOW, FRONT_X, SHEET, SHEET_MARGIN, SIDE_X, Sheet, TITLE_TOP,
};

const CLOSE: f64 = 1e-9;

fn laid_out(head: HeadSpec) -> (TipGeometry, Sheet) {
    let tip = TipGeometry::new(&head, &HairSpec::SYNTHETIC);
    let sheet = Sheet::of(&tip);
    (tip, sheet)
}

#[test]
fn a_round_and_the_factory_fan_draw_at_the_mockups_full_scale() {
    assert_eq!(laid_out(HeadSpec::ROUND).1.scale, 46.0);
    assert_eq!(laid_out(HeadSpec::FAN).1.scale, 46.0);
}

#[test]
fn a_rigger_is_fitted_to_the_height_as_in_the_mockup() {
    let (_, sheet) = laid_out(HeadSpec::RIGGER);
    assert!(
        (sheet.scale - 300.0 / (6.0 + 2.6)).abs() < CLOSE,
        "{sheet:?}"
    );
}

#[test]
fn a_wide_long_fan_stays_on_the_left_of_the_sheet() {
    let (_, sheet) = laid_out(HeadSpec {
        fan_deg: 90.0,
        length: 6.0,
        ..HeadSpec::FLAT
    });
    assert!(
        FRONT_X - sheet.half_width * sheet.scale >= SHEET_MARGIN - CLOSE,
        "{sheet:?}"
    );
}

#[test]
fn a_full_belly_stays_on_the_right_of_the_sheet() {
    let (tip, sheet) = laid_out(HeadSpec {
        belly: 1.0,
        ..HeadSpec::ROUND
    });
    let [min, max] = tip.side.extent();
    let half = f64::from(min[0].abs().max(max[0]));
    assert!(
        SIDE_X + half * sheet.scale <= SHEET[0] - SHEET_MARGIN + CLOSE,
        "{sheet:?}"
    );
}

#[test]
fn a_wide_fan_beside_a_full_belly_leaves_room_between_the_views() {
    let (tip, sheet) = laid_out(HeadSpec {
        fan_deg: 90.0,
        belly: 1.0,
        ..HeadSpec::ROUND
    });
    let [min, max] = tip.side.extent();
    let side_half = f64::from(min[0].abs().max(max[0]));
    let front_right = FRONT_X + sheet.half_width * sheet.scale;
    let side_left = SIDE_X - side_half * sheet.scale;
    assert!(
        side_left - front_right >= super::BETWEEN_VIEWS - CLOSE,
        "{sheet:?}"
    );
}

#[test]
fn the_footprint_sits_above_the_title_block() {
    let long_slant = HeadSpec {
        length: 6.0,
        slant: 1.0,
        ..HeadSpec::ROUND
    };
    let heads = factory_brushes().into_iter().map(|brush| brush.head);
    for head in heads.chain([long_slant]) {
        let (tip, sheet) = laid_out(head);
        let half_height = f64::from(tip.dimensions.thickness) * 1.06 * sheet.footprint_scale + 1.0;
        assert!(
            sheet.footprint_y + half_height < TITLE_TOP,
            "{head:?} {sheet:?}"
        );
    }
}

#[test]
fn a_wide_tip_keeps_its_width_figure_above_the_footprints_caption() {
    let (_, sheet) = laid_out(HeadSpec::MOP);
    let lowest_figure = sheet.roots_y + sheet.front_depth * sheet.scale + FIGURES_BELOW;
    assert!(
        sheet.footprint_caption_y - CAPTION_HEIGHT >= lowest_figure - CLOSE,
        "{sheet:?}"
    );
}

#[test]
fn a_narrow_tip_leaves_the_footprint_where_the_mockup_has_it() {
    let (_, sheet) = laid_out(HeadSpec::ROUND);
    let deepest = sheet.roots_y + 2.5 * sheet.scale;
    assert!(
        (sheet.footprint_y - (deepest + 92.0)).abs() < CLOSE,
        "{sheet:?}"
    );
    assert!(
        (sheet.footprint_caption_y - (sheet.footprint_y - 68.0)).abs() < 1e-6,
        "{sheet:?}"
    );
}
