use cadmium_engine::brush::{BrushSpec, factory_brushes};

use super::{DELETE, MenuState, SAVE_COPY};
use crate::brush_editor::draft::{Draft, Origin};

fn menu_of(draft: &Draft) -> [bool; 3] {
    let menu = MenuState::of(draft);
    [SAVE_COPY, "export", DELETE].map(|name| menu.enables(name))
}

#[test]
fn a_new_brush_can_only_be_exported() {
    assert_eq!(menu_of(&Draft::new_brush()), [false, true, false]);
}

#[test]
fn a_factory_brush_can_be_copied_and_exported_but_not_deleted() {
    let draft = Draft::open(factory_brushes().swap_remove(0), Origin::Factory);
    assert_eq!(menu_of(&draft), [true, true, false]);
}

#[test]
fn a_user_brush_can_be_copied_exported_and_deleted() {
    let mine = BrushSpec {
        name: "Mine".into(),
        ..factory_brushes().swap_remove(3)
    };
    assert_eq!(
        menu_of(&Draft::open(mine, Origin::User(17))),
        [true, true, true]
    );
}
