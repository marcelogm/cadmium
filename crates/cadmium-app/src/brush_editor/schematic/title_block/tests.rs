use cadmium_engine::brush::{BrushSpec, factory_brushes};
use cadmium_studio::{hair_name, head_name};

use super::super::Subject;
use super::cells;

fn subject(brush: &BrushSpec) -> Subject<'_> {
    Subject {
        brush,
        head: head_name(&brush.head),
        hair: hair_name(&brush.hair),
    }
}

#[test]
fn a_factory_brush_fills_the_title_block() {
    let fan = factory_brushes().swap_remove(8);
    assert_eq!(
        cells(&subject(&fan)),
        [
            ("NAME", "Fan · Hog Bristle".to_string()),
            ("HEAD", "FAN".to_string()),
            ("HAIR", "HOG BRISTLE".to_string()),
            ("LENGTH · THICKNESS", "2.00R · 0.12".to_string()),
            ("FAN · SLANT", "50° · 0.00".to_string()),
            ("SUGGESTED SIZE", "64 px".to_string()),
            ("DENSITY", "1.6 /px".to_string()),
        ]
    );
}

#[test]
fn a_part_off_its_preset_reads_custom() {
    let brush = factory_brushes().swap_remove(0);
    let custom = Subject {
        head: None,
        ..subject(&brush)
    };
    assert_eq!(cells(&custom)[1], ("HEAD", "CUSTOM".to_string()));
}

#[test]
fn a_long_name_is_cut_with_an_ellipsis() {
    let brush = BrushSpec {
        name: "x".repeat(60),
        ..factory_brushes().swap_remove(0)
    };
    let name = &cells(&subject(&brush))[0].1;
    assert_eq!(name.chars().count(), 48);
    assert!(name.ends_with('…'));
}
