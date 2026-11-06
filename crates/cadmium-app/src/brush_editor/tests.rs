use cadmium_engine::brush::{BrushSpec, factory_brushes};

use super::{CUSTOM, summary_rows};

#[test]
fn the_summary_names_a_factory_brush() {
    let brush = factory_brushes().into_iter().next().unwrap();
    let rows = summary_rows(&brush);
    let head = cadmium_studio::head_name(&brush.head).unwrap();
    let hair = cadmium_studio::hair_name(&brush.hair).unwrap();
    assert_eq!(rows[0], ("Name", brush.name.clone()));
    assert_eq!(rows[1], ("Head", head.to_owned()));
    assert_eq!(rows[2], ("Hair", hair.to_owned()));
    assert_eq!(rows[3], ("Size", format!("{:.0} px", brush.size_px)));
}

#[test]
fn the_summary_calls_changed_parts_custom() {
    let mut brush: BrushSpec = factory_brushes().into_iter().next().unwrap();
    brush.head.thickness += 0.123;
    brush.hair.stiffness += 0.123;
    brush.size_px = 23.6;
    let rows = summary_rows(&brush);
    assert_eq!(rows[1].1, CUSTOM);
    assert_eq!(rows[2].1, CUSTOM);
    assert_eq!(rows[3].1, "24 px");
}
