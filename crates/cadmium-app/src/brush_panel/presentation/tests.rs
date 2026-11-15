use super::*;
use crate::tools::knife::KNIFE_HANDLING;
use crate::tools::panel::panel_tool;
use crate::tools::{Tool, UiTool};
use cadmium_engine::brush::{BrushSpec, HairSpec, HeadSpec, factory_brushes};
use cadmium_engine::palette::paint_recipe;
use cadmium_engine::pigments::MAX_SLOTS;

#[test]
fn recipe_description_names_a_single_pigment() {
    assert_eq!(describe_recipe(&["White", "Blue"], &[(1, 1.0)]), "Blue");
}

#[test]
fn recipe_description_lists_shares_largest_first() {
    assert_eq!(
        describe_recipe(&["White", "Blue", "Yellow"], &[(1, 1.0), (2, 3.0)]),
        "Yellow 75% · Blue 25%"
    );
}

#[test]
fn empty_recipe_reads_as_a_clean_brush() {
    assert_eq!(describe_recipe(&["White"], &[]), CLEAN_LABEL);
    assert_eq!(describe_recipe(&["White"], &[(0, 0.0)]), CLEAN_LABEL);
}

#[test]
fn brush_shows_every_control_and_eyedropper_only_the_hint() {
    let brush = Tool::Brush.controls();
    assert!(brush.pigments && brush.loaded && brush.size && brush.load && brush.medium);
    assert!(brush.shape && brush.stroke_loading && brush.clean && !brush.hint);
    assert!(!brush.plate_gap);
    let eyedropper = Tool::Eyedropper.controls();
    assert!(eyedropper.hint);
    assert!(!eyedropper.pigments && !eyedropper.loaded && !eyedropper.size);
    assert!(!eyedropper.load && !eyedropper.medium && !eyedropper.plate_gap);
    assert!(!eyedropper.shape && !eyedropper.clean);
    assert!(!eyedropper.stroke_loading);
}

#[test]
fn knife_loads_from_the_palette_and_sets_its_plate_but_has_no_medium_or_shape() {
    let knife = Tool::Knife.controls();
    assert!(knife.pigments && knife.loaded && knife.size && knife.load);
    assert!(knife.plate_gap && knife.stroke_loading && knife.clean && knife.hint);
    assert!(!knife.medium && !knife.shape);
}

#[test]
fn the_rag_and_the_pencil_show_only_the_size() {
    for tool in [Tool::Rag, Tool::Pencil] {
        let shown = tool.controls();
        assert!(shown.size, "{tool:?}");
        assert!(!shown.pigments && !shown.loaded && !shown.load && !shown.medium);
        assert!(!shown.plate_gap && !shown.shape && !shown.stroke_loading);
        assert!(!shown.clean && !shown.hint, "{tool:?}");
    }
}

#[test]
fn only_the_pencil_shows_its_hardness() {
    for tool in UiTool::ALL {
        let panel = panel_tool(tool, Tool::Brush);
        let pencil = tool == UiTool::Paint(Tool::Pencil);
        assert_eq!(panel.controls().hardness, pencil, "{tool:?}");
    }
}

#[test]
fn hardness_reads_as_pencil_grades_from_b_to_h() {
    assert_eq!(hardness_grade(0.0), "8B");
    assert_eq!(hardness_grade(0.5), "HB");
    assert_eq!(hardness_grade(1.0), "8H");
    assert_eq!(hardness_grade(0.3), "4B");
    assert_eq!(hardness_grade(2.0), "8H");
    assert_eq!(hardness_grade(f32::NAN), "HB");
}

#[test]
fn knife_panel_speaks_of_the_knife_and_how_to_hold_it() {
    let knife = Tool::Knife.wording();
    assert_eq!(knife.clean_button, "Clean Knife");
    assert_eq!(knife.loaded_title, "On the Knife");
    assert_eq!(knife.size_title, "Size");
    assert_eq!(tool_hint(Tool::Knife), KNIFE_HANDLING);
    assert_eq!(Tool::Brush.wording().clean_button, "Clean Brush");
    assert_eq!(tool_hint(Tool::Eyedropper), EYEDROPPER_HINT);
}

#[test]
fn the_selected_colour_highlights_the_paint_or_pinned_colour_it_equals() {
    let mut mix = [0.0; MAX_SLOTS];
    mix[0] = 0.5;
    mix[1] = 0.5;
    let palette = Palette {
        my_colours: vec![mix],
        circle: vec![],
    };
    assert_eq!(
        highlighted_colour(&palette, Some(&paint_recipe(1))),
        Some(ColourRef::Paint(1))
    );
    assert_eq!(
        highlighted_colour(&palette, Some(&mix)),
        Some(ColourRef::Mine(0))
    );
    let mut other = mix;
    other[0] = 0.4;
    other[1] = 0.6;
    assert_eq!(highlighted_colour(&palette, Some(&other)), None);
    assert_eq!(highlighted_colour(&palette, None), None);
}

#[test]
fn a_pinned_colour_that_is_one_paint_highlights_the_paint() {
    let palette = Palette {
        my_colours: vec![paint_recipe(2)],
        circle: vec![],
    };
    assert_eq!(
        highlighted_colour(&palette, Some(&paint_recipe(2))),
        Some(ColourRef::Paint(2))
    );
}

#[test]
fn a_colour_within_a_thousandth_of_a_pinned_one_per_paint_is_that_colour() {
    let mut mix = [0.0; MAX_SLOTS];
    mix[0] = 0.5;
    mix[1] = 0.5;
    let palette = Palette {
        my_colours: vec![paint_recipe(3), mix],
        circle: vec![],
    };
    let mut near = mix;
    near[0] = 0.5005;
    near[1] = 0.4995;
    assert_eq!(
        highlighted_colour(&palette, Some(&near)),
        Some(ColourRef::Mine(1))
    );
    let mut far = mix;
    far[0] = 0.502;
    far[1] = 0.498;
    assert_eq!(highlighted_colour(&palette, Some(&far)), None);
    let mut last_differs = mix;
    last_differs[0] = 0.4;
    last_differs[MAX_SLOTS - 1] = 0.1;
    assert_eq!(highlighted_colour(&palette, Some(&last_differs)), None);
}

#[test]
fn the_palette_views_are_named_as_their_toggles_are() {
    for view in [PaletteView::Colours, PaletteView::Circle] {
        assert_eq!(palette_view_named(palette_view_name(view)), Some(view));
    }
    assert_eq!(palette_view_name(PaletteView::Colours), "colours");
    assert_eq!(palette_view_name(PaletteView::Circle), "circle");
    assert_eq!(palette_view_named("wheel"), None);
    let named: Vec<&str> = PALETTE_VIEWS
        .iter()
        .map(|(view, _)| palette_view_name(*view))
        .collect();
    assert_eq!(named, ["colours", "circle"]);
}

#[test]
fn the_circle_shows_only_in_its_view_and_for_a_tool_that_takes_paint() {
    assert!(circle_shows(Tool::Brush, PaletteView::Circle));
    assert!(circle_shows(Tool::Knife, PaletteView::Circle));
    assert!(!circle_shows(Tool::Brush, PaletteView::Colours));
    assert!(!circle_shows(Tool::Pencil, PaletteView::Circle));
}

fn wheel_key(darken: f32) -> WheelKey {
    WheelKey::of(
        &Palette {
            my_colours: vec![],
            circle: vec![ColourRef::Paint(1)],
        },
        &["titanium_white".to_string(), "ultramarine_blue".to_string()],
        darken,
    )
}

#[test]
fn the_wheel_is_drawn_again_only_when_it_shows_and_what_it_draws_changed() {
    let key = wheel_key(0.25);
    assert!(wheel_is_stale(true, None, &key));
    assert!(!wheel_is_stale(false, None, &key));
    assert!(!wheel_is_stale(true, Some(&key), &wheel_key(0.25)));
    assert!(wheel_is_stale(true, Some(&key), &wheel_key(0.5)));
    assert!(!wheel_is_stale(false, Some(&key), &wheel_key(0.5)));
}

#[test]
fn the_wheel_depends_on_the_circle_the_pinned_colours_the_paints_and_the_darkening() {
    let key = wheel_key(0.25);
    let mut other_circle = key.clone();
    other_circle.palette.circle.push(ColourRef::Paint(0));
    let mut other_pinned = key.clone();
    other_pinned.palette.my_colours.push(paint_recipe(1));
    let mut other_paints = key.clone();
    other_paints.slots.push("ivory_black".to_string());
    for other in [other_circle, other_pinned, other_paints, wheel_key(0.26)] {
        assert_ne!(key, other);
    }
}

#[test]
fn moving_keeps_the_panel_of_the_tool_in_hand() {
    assert_eq!(panel_tool(UiTool::Move, Tool::Knife), Tool::Knife);
    assert_eq!(panel_tool(UiTool::Paint(Tool::Rag), Tool::Brush), Tool::Rag);
}

#[test]
fn ctrl_mixes_a_colour_in_and_a_plain_click_loads_it() {
    assert_eq!(
        colour_click(gdk::ModifierType::CONTROL_MASK),
        ColourClick::MixIn
    );
    assert_eq!(
        colour_click(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK),
        ColourClick::MixIn
    );
    assert_eq!(
        colour_click(gdk::ModifierType::SHIFT_MASK),
        ColourClick::Load
    );
    assert_eq!(colour_click(gdk::ModifierType::empty()), ColourClick::Load);
}

#[test]
fn a_pick_on_the_wheel_loads_as_it_drags_but_mixes_in_only_at_the_press() {
    let plain = gdk::ModifierType::empty();
    let ctrl = gdk::ModifierType::CONTROL_MASK;
    let ctrl_shift = ctrl | gdk::ModifierType::SHIFT_MASK;
    assert_eq!(
        circle_click(WheelPick::Press, plain),
        Some(ColourClick::Load)
    );
    assert_eq!(
        circle_click(WheelPick::Drag, plain),
        Some(ColourClick::Load)
    );
    assert_eq!(
        circle_click(WheelPick::Press, ctrl),
        Some(ColourClick::MixIn)
    );
    assert_eq!(circle_click(WheelPick::Drag, ctrl), None);
    assert_eq!(
        circle_click(WheelPick::Press, ctrl_shift),
        Some(ColourClick::MixIn)
    );
    assert_eq!(circle_click(WheelPick::Drag, ctrl_shift), None);
    let shift = gdk::ModifierType::SHIFT_MASK;
    assert_eq!(
        circle_click(WheelPick::Drag, shift),
        Some(ColourClick::Load)
    );
}

#[test]
fn size_slider_is_left_alone_while_its_value_rounds_to_the_brush_size() {
    let dragged = size_to_slider(40.0) + SIZE_STEP / 10.0;
    assert!(!slider_needs_sync(dragged, 40.0));
    assert!(slider_needs_sync(size_to_slider(80.0), 40.0));
}

#[test]
fn size_slider_is_logarithmic_and_clamped() {
    assert_eq!(slider_to_size(size_to_slider(40.0)), 40.0);
    assert_eq!(slider_to_size(size_to_slider(1.0)), MIN_SIZE_PX as f32);
    assert_eq!(slider_to_size(size_to_slider(9000.0)), MAX_SIZE_PX as f32);
    let middle = slider_to_size((size_to_slider(4.0) + size_to_slider(500.0)) / 2.0);
    assert!(middle > 40.0 && middle < 50.0, "{middle}");
}

#[test]
fn plate_gap_control_drives_the_knife_gap_parameter() {
    assert_eq!(plate_gap_spec().key, PLATE_GAP_KEY);
    assert_eq!(plate_gap_text(80.0), "80 µm");
}

fn user_brush(head: HeadSpec, hair: HairSpec) -> BrushSpec {
    BrushSpec {
        name: "My Brush".to_string(),
        size_px: 40.0,
        head,
        hair,
    }
}

#[test]
fn a_factory_row_shows_its_head_then_its_hair() {
    let brush = &factory_brushes()[1];
    let entry = rack_entry(brush, false);
    assert_eq!(entry.title, "Round");
    assert_eq!(entry.subtitle, "Kolinsky Sable");
    assert_eq!(entry.name, "Round · Kolinsky Sable");
}

#[test]
fn a_user_row_shows_its_name_then_head_and_hair() {
    let entry = rack_entry(&user_brush(HeadSpec::FLAT, HairSpec::HOG), true);
    assert_eq!(entry.title, "My Brush");
    assert_eq!(entry.subtitle, "Flat · Hog Bristle");
    assert_eq!(entry.name, "My Brush");
}

#[test]
fn a_user_row_calls_a_changed_head_or_hair_custom() {
    let head = HeadSpec {
        dome: 0.5,
        ..HeadSpec::FLAT
    };
    let hair = HairSpec {
        stiffness: 0.51,
        ..HairSpec::HOG
    };
    let subtitle = |head, hair| rack_entry(&user_brush(head, hair), true).subtitle;
    assert_eq!(subtitle(head, HairSpec::HOG), "Custom · Hog Bristle");
    assert_eq!(subtitle(HeadSpec::FLAT, hair), "Flat · Custom");
    assert_eq!(subtitle(head, hair), "Custom · Custom");
}

#[test]
fn a_row_carries_the_brush_shape_for_its_tip_icon() {
    let entry = rack_entry(&user_brush(HeadSpec::FAN, HairSpec::SQUIRREL), true);
    assert_eq!(entry.head, HeadSpec::FAN);
    assert_eq!(entry.hair, HairSpec::SQUIRREL);
}
