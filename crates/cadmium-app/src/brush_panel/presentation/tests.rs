use super::*;
use crate::tools::knife::KNIFE_HANDLING;
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
    let brush = visible_controls(PanelTool::Paint(Tool::Brush));
    assert!(brush.pigments && brush.loaded && brush.size && brush.load && brush.medium);
    assert!(brush.shape && brush.stroke_loading && brush.clean && !brush.hint);
    assert!(!brush.plate_gap);
    let eyedropper = visible_controls(PanelTool::Paint(Tool::Eyedropper));
    assert!(eyedropper.hint);
    assert!(!eyedropper.pigments && !eyedropper.loaded && !eyedropper.size);
    assert!(!eyedropper.load && !eyedropper.medium && !eyedropper.plate_gap);
    assert!(!eyedropper.shape && !eyedropper.clean);
    assert!(!eyedropper.stroke_loading);
}

#[test]
fn knife_loads_from_the_palette_and_sets_its_plate_but_has_no_medium_or_shape() {
    let knife = visible_controls(PanelTool::Paint(Tool::Knife));
    assert!(knife.pigments && knife.loaded && knife.size && knife.load);
    assert!(knife.plate_gap && knife.stroke_loading && knife.clean && knife.hint);
    assert!(!knife.medium && !knife.shape);
}

#[test]
fn rag_shows_only_the_size() {
    let shown = visible_controls(PanelTool::Paint(Tool::Rag));
    assert!(shown.size);
    assert!(!shown.pigments && !shown.loaded && !shown.load && !shown.medium);
    assert!(!shown.plate_gap && !shown.shape && !shown.stroke_loading);
    assert!(!shown.clean && !shown.hint);
}

#[test]
fn knife_panel_speaks_of_the_knife_and_how_to_hold_it() {
    let knife = tool_wording(PanelTool::Paint(Tool::Knife));
    assert_eq!(knife.clean_button, "Clean Knife");
    assert_eq!(knife.loaded_title, "On the Knife");
    assert_eq!(knife.size_title, "Size");
    assert_eq!(tool_hint(PanelTool::Paint(Tool::Knife)), KNIFE_HANDLING);
    assert_eq!(
        tool_wording(PanelTool::Paint(Tool::Brush)).clean_button,
        "Clean Brush"
    );
    assert_eq!(
        tool_hint(PanelTool::Paint(Tool::Eyedropper)),
        EYEDROPPER_HINT
    );
}

#[test]
fn the_tube_shows_only_pigments_and_nozzle_size() {
    let tube = visible_controls(PanelTool::Tube);
    assert!(tube.pigments && tube.size && tube.tool_group());
    assert!(!tube.loaded && !tube.load && !tube.medium && !tube.plate_gap);
    assert!(!tube.shape && !tube.stroke_loading && !tube.clean && !tube.hint);
}

#[test]
fn the_tube_names_its_size_nozzle_without_a_subtitle() {
    let tube = tool_wording(PanelTool::Tube);
    assert_eq!(tube.size_title, "Nozzle");
    assert_eq!(tube.size_subtitle, None);
}

#[test]
fn the_tube_asks_for_a_pile_to_squeeze() {
    assert_eq!(
        tool_wording(PanelTool::Tube).pigments,
        "Click to pick the tube"
    );
}

#[test]
fn moving_keeps_the_panel_of_the_tool_in_hand() {
    assert_eq!(
        panel_tool(UiTool::Move, Tool::Knife),
        PanelTool::Paint(Tool::Knife)
    );
    assert_eq!(panel_tool(UiTool::Tube, Tool::Knife), PanelTool::Tube);
    assert_eq!(
        panel_tool(UiTool::Paint(Tool::Rag), Tool::Brush),
        PanelTool::Paint(Tool::Rag)
    );
}

#[test]
fn the_tube_highlights_its_own_pile_not_the_recipe() {
    let recipe = [(0, 1.0), (2, 0.5)];
    assert!(pile_is_highlighted(PanelTool::Tube, 1, 1, &recipe));
    assert!(!pile_is_highlighted(PanelTool::Tube, 0, 1, &recipe));
    let brush = PanelTool::Paint(Tool::Brush);
    assert!(pile_is_highlighted(brush, 2, 1, &recipe));
    assert!(!pile_is_highlighted(brush, 1, 1, &recipe));
}

#[test]
fn the_tube_group_names_the_chosen_pigment() {
    let names = ["White", "Blue"];
    assert_eq!(
        tool_group_description(PanelTool::Tube, &names, 1),
        Some("Blue")
    );
    assert_eq!(
        tool_group_description(PanelTool::Paint(Tool::Brush), &names, 1),
        None
    );
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
fn a_clean_brush_reads_as_clean() {
    let wording = tool_wording(PanelTool::Paint(Tool::Brush));
    assert_eq!(
        loaded_text(&wording, true, &["Titanium White"], &[(0, 1.0)]),
        wording.clean
    );
}

#[test]
fn a_loaded_brush_reads_its_recipe() {
    let wording = tool_wording(PanelTool::Paint(Tool::Brush));
    assert_eq!(
        loaded_text(&wording, false, &["Titanium White"], &[(0, 1.0)]),
        "Titanium White"
    );
}

#[test]
fn the_size_reads_in_whole_pixels() {
    assert_eq!(size_text(12.4), "12 px");
    assert_eq!(size_text(12.6), "13 px");
}

#[test]
fn the_add_button_shows_while_there_is_room_and_something_to_add() {
    assert!(offers_more_pigments(1, 3));
    assert!(!offers_more_pigments(MAX_SLOTS, 3));
    assert!(!offers_more_pigments(1, 0));
}

#[test]
fn the_rack_refills_only_when_its_brushes_change() {
    let brush = &factory_brushes()[1];
    let entries = vec![rack_entry(brush, false)];
    let keys = vec![ThumbnailKey::of(brush)];
    assert!(rack_needs_refill(None, &entries, &keys));
    let shown = (entries.clone(), keys.clone());
    assert!(!rack_needs_refill(Some(&shown), &entries, &keys));
    assert!(rack_needs_refill(Some(&shown), &[], &[]));
    assert!(rack_needs_refill(Some(&shown), &entries, &[]));
}
