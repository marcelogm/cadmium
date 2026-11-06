use cadmium_engine::brush::{BrushSpec, HeadSpec, factory_brushes};
use cadmium_studio::{HairPreset, HeadPreset};

use super::{Draft, Origin, SavePlan};
use crate::brush_editor::closing::Closing;
use crate::brush_editor::controls::{Control, Part};

fn factory() -> Draft {
    Draft::open(factory_brushes().swap_remove(0), Origin::Factory)
}

fn mine() -> BrushSpec {
    BrushSpec {
        name: "Mine".into(),
        ..factory_brushes().swap_remove(3)
    }
}

fn user() -> Draft {
    Draft::open(mine(), Origin::User(17))
}

fn added(plan: SavePlan) -> BrushSpec {
    match plan {
        SavePlan::AddNew(brush) => brush,
        SavePlan::Replace(index) => panic!("replaces {index}"),
    }
}

#[test]
fn a_fresh_draft_is_unchanged() {
    assert!(!factory().is_dirty());
}

#[test]
fn a_change_and_its_undoing_leave_the_draft_unchanged() {
    let mut draft = factory();
    draft.set_size(77.0);
    assert!(draft.is_dirty());
    draft.set_size(factory_brushes()[0].size_px);
    assert!(!draft.is_dirty());
}

#[test]
fn the_size_stays_in_range() {
    let mut draft = factory();
    draft.set_size(1.0);
    assert_eq!(draft.current().size_px, BrushSpec::SIZE_RANGE_PX[0]);
    draft.set_size(9000.0);
    assert_eq!(draft.current().size_px, BrushSpec::SIZE_RANGE_PX[1]);
}

#[test]
fn cancelling_a_changed_draft_asks_first() {
    let mut draft = factory();
    draft.set_name("Other");
    assert!(draft.asks_before(Closing::Cancel));
}

#[test]
fn cancelling_an_unchanged_draft_does_not_ask() {
    assert!(!factory().asks_before(Closing::Cancel));
}

#[test]
fn saving_or_deleting_never_asks_even_with_changes() {
    let mut draft = user();
    draft.set_size(99.0);
    assert!(!draft.asks_before(Closing::Done));
}

#[test]
fn a_user_brush_is_saved_over_itself() {
    let mut draft = user();
    draft.set_size(12.0);
    assert_eq!(draft.save_plan(), SavePlan::Replace(17));
    assert_eq!(draft.saved_brush().size_px, 12.0);
    assert_eq!(draft.saved_brush().name, "Mine");
}

#[test]
fn an_unrenamed_factory_brush_is_saved_as_a_copy() {
    let brush = added(factory().save_plan());
    assert_eq!(brush.name, "Round · Synthetic (copy)");
}

#[test]
fn a_renamed_factory_brush_keeps_its_new_name() {
    let mut draft = factory();
    draft.set_name("  Big Round ");
    assert_eq!(added(draft.save_plan()).name, "Big Round");
}

#[test]
fn a_blank_name_falls_back_to_the_original() {
    let mut draft = user();
    draft.set_name("   ");
    assert_eq!(draft.chosen_name(), "Mine");
}

#[test]
fn save_as_copy_adds_copy_to_the_chosen_name() {
    let mut draft = user();
    draft.set_name("Other");
    assert_eq!(draft.copy().name, "Other (copy)");
}

#[test]
fn only_a_user_brush_can_be_deleted() {
    assert_eq!(user().deletable(), Some(17));
    assert_eq!(factory().deletable(), None);
}

#[test]
fn a_name_changed_only_by_spaces_leaves_the_draft_unchanged() {
    let mut draft = user();
    draft.set_name("  Mine ");
    assert!(!draft.is_dirty());
    assert!(!draft.asks_before(Closing::Cancel));
}

#[test]
fn a_copy_of_a_copy_is_not_called_copy_twice() {
    let draft = Draft::open(
        BrushSpec {
            name: "Mine (copy)".into(),
            ..mine()
        },
        Origin::User(17),
    );
    assert_eq!(draft.copy().name, "Mine (copy)");
}

#[test]
fn a_brush_out_of_range_opens_in_range_and_unchanged() {
    let draft = Draft::open(
        BrushSpec {
            size_px: 1.0,
            ..mine()
        },
        Origin::Factory,
    );
    assert_eq!(draft.original().size_px, BrushSpec::SIZE_RANGE_PX[0]);
    assert!(!draft.is_dirty());
}

#[test]
fn a_factory_brush_opens_on_its_presets() {
    let draft = factory();
    assert_eq!(draft.head_preset(), Some(HeadPreset::Round));
    assert_eq!(draft.hair_preset(), Some(HairPreset::Synthetic));
    assert!(!draft.is_custom(Part::Head) && !draft.is_custom(Part::Hair));
}

#[test]
fn a_changed_head_opens_as_custom() {
    let brush = BrushSpec {
        head: HeadSpec {
            thickness: 0.7,
            ..HeadSpec::ROUND
        },
        ..factory_brushes().swap_remove(0)
    };
    let draft = Draft::open(brush, Origin::Factory);
    assert_eq!(draft.head_preset(), None);
    assert!(draft.is_custom(Part::Head) && !draft.is_custom(Part::Hair));
}

#[test]
fn a_head_out_of_range_opens_on_the_preset_it_clamps_to() {
    let brush = BrushSpec {
        head: HeadSpec {
            thickness: 5.0,
            ..HeadSpec::ROUND
        },
        ..factory_brushes().swap_remove(0)
    };
    let draft = Draft::open(brush, Origin::Factory);
    assert_eq!(draft.head_preset(), Some(HeadPreset::Round));
}

#[test]
fn a_preset_locks_its_sliders() {
    let mut draft = factory();
    assert!(!draft.set(Control::Thickness, 0.5));
    assert!(!draft.is_dirty());
}

#[test]
fn custom_keeps_the_values_and_unlocks_them() {
    let mut draft = factory();
    draft.pick_head(None);
    assert_eq!(draft.current().head, HeadSpec::ROUND);
    assert!(!draft.is_dirty());
    assert!(draft.set(Control::Thickness, 0.5));
    assert_eq!(draft.current().head.thickness, 0.5);
    assert!(draft.is_dirty());
}

#[test]
fn picking_a_preset_takes_its_values() {
    let mut draft = factory();
    draft.pick_head(Some(HeadPreset::Fan));
    assert_eq!(draft.current().head, HeadSpec::FAN);
    assert!(draft.is_dirty());
    draft.pick_head(Some(HeadPreset::Round));
    assert!(!draft.is_dirty());
}

#[test]
fn picking_a_hair_preset_takes_its_values() {
    let mut draft = factory();
    draft.pick_hair(Some(HairPreset::Hog));
    assert_eq!(draft.current().hair, HairPreset::Hog.spec());
    assert_eq!(draft.hair_preset(), Some(HairPreset::Hog));
    assert!(draft.is_dirty());
}

#[test]
fn custom_values_that_match_a_preset_stay_custom() {
    let mut draft = factory();
    draft.pick_head(None);
    draft.set(Control::Thickness, 0.5);
    draft.set(Control::Thickness, 1.0);
    assert_eq!(draft.current().head, HeadSpec::ROUND);
    assert_eq!(draft.head_preset(), None);
}

#[test]
fn unlocking_one_part_leaves_the_other_locked() {
    let mut draft = factory();
    draft.pick_hair(None);
    assert!(draft.set(Control::Stiffness, 0.3));
    assert!(!draft.set(Control::Length, 3.0));
}

#[test]
fn a_drag_that_ends_near_the_original_value_leaves_the_draft_unchanged() {
    let mut draft = factory();
    draft.pick_head(None);
    draft.set(Control::Thickness, 0.9987);
    assert!(!draft.is_dirty());
}

#[test]
fn a_brush_dragged_onto_a_presets_values_opens_on_that_preset() {
    let mut draft = factory();
    draft.pick_head(None);
    draft.set(Control::Thickness, 0.4372);
    draft.set(Control::Thickness, 0.9987);
    let reopened = Draft::open(draft.saved_brush(), Origin::Factory);
    assert_eq!(reopened.head_preset(), Some(HeadPreset::Round));
}
