use super::*;
use crate::tools::knife::KNIFE_HANDLING;
use crate::tools::mixing_knife::MIXING_KNIFE_HANDLING;
use crate::tools::panel::panel_tool;
use crate::tools::{Tool, UiTool};
use cadmium_engine::brush::{BrushSpec, HairSpec, HeadSpec, factory_brushes};

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
fn mixing_knife_shows_what_its_face_carries_but_loads_no_pigment() {
    let shown = Tool::MixingKnife.controls();
    assert!(shown.loaded && shown.size && shown.clean && shown.hint);
    assert!(!shown.pigments && !shown.load && !shown.medium);
    assert!(!shown.plate_gap && !shown.shape && !shown.stroke_loading);
}

#[test]
fn mixing_knife_panel_speaks_of_the_mixing_knife_and_how_to_hold_it() {
    let wording = Tool::MixingKnife.wording();
    assert_eq!(wording.loaded_title, "On the Mixing Knife");
    assert_eq!(wording.clean_button, "Clean Mixing Knife");
    assert_eq!(wording.size_subtitle, Some("Blade length"));
    assert_eq!(tool_hint(Tool::MixingKnife), MIXING_KNIFE_HANDLING);
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
fn a_pile_is_highlighted_while_the_recipe_has_some_of_it() {
    let recipe = [(0, 1.0), (2, 0.5), (3, 0.0)];
    assert!(pile_is_highlighted(2, &recipe));
    assert!(!pile_is_highlighted(1, &recipe));
    assert!(!pile_is_highlighted(3, &recipe));
}

#[test]
fn moving_keeps_the_panel_of_the_tool_in_hand() {
    assert_eq!(panel_tool(UiTool::Move, Tool::Knife), Tool::Knife);
    assert_eq!(panel_tool(UiTool::Paint(Tool::Rag), Tool::Brush), Tool::Rag);
}

#[test]
fn modifiers_choose_how_a_swatch_click_loads_the_brush() {
    assert_eq!(
        pigment_click(gdk::ModifierType::empty()),
        PigmentClick::Choose
    );
    assert_eq!(
        pigment_click(gdk::ModifierType::CONTROL_MASK),
        PigmentClick::Mix
    );
    assert_eq!(
        pigment_click(gdk::ModifierType::SHIFT_MASK),
        PigmentClick::CleanAndLoad
    );
    assert_eq!(
        pigment_click(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK),
        PigmentClick::Mix
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

#[test]
fn the_mixing_knife_swatch_shows_only_its_face() {
    let (palette, held) = ([10, 20, 30], [200, 100, 0]);
    assert_eq!(
        loaded_swatch_colours(Tool::MixingKnife, palette, held),
        (held, None)
    );
    for tool in [Tool::Brush, Tool::Knife] {
        assert_eq!(
            loaded_swatch_colours(tool, palette, held),
            (palette, Some(held))
        );
    }
    let wording = Tool::MixingKnife.wording();
    assert!(!wording.loaded_tooltip.contains("palette"));
}
