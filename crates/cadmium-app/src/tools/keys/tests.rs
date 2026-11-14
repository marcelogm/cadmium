use super::*;
use crate::tools::Tool;

#[test]
fn keypad_digits_choose_the_same_tools_as_the_number_row() {
    for (keypad, tool) in KEYPAD_TOOL_KEYS {
        let digit = keypad.name().unwrap().replace("KP_", "");
        assert!(
            TOOL_KEYS
                .iter()
                .any(|(key, bound)| key.name().as_deref() == Some(digit.as_str())
                    && *bound == tool),
            "{keypad:?}"
        );
    }
}

#[test]
fn six_chooses_no_painting_tool() {
    assert!(!TOOL_KEYS.iter().any(|(key, _)| *key == gdk::Key::_6));
    assert!(
        !KEYPAD_TOOL_KEYS
            .iter()
            .any(|(key, _)| *key == gdk::Key::KP_6)
    );
}

#[test]
fn no_two_tools_share_a_key() {
    let mut keys: Vec<gdk::Key> = TOOL_KEYS
        .iter()
        .chain(&KEYPAD_TOOL_KEYS)
        .map(|(key, _)| *key)
        .collect();
    let bound = keys.len();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), bound);
}

const NONE: gdk::ModifierType = gdk::ModifierType::empty();
const SHIFT: gdk::ModifierType = gdk::ModifierType::SHIFT_MASK;

fn pick(levels: (gdk::Key, gdk::Key), modifiers: gdk::ModifierType) -> Option<UiTool> {
    digit_pick(Some(levels.0), Some(levels.1), modifiers)
}

#[test]
fn a_digit_picks_the_paintings_tool_on_either_keyboard_level() {
    let brush = Some(UiTool::Paint(Tool::Brush));
    assert_eq!(pick((gdk::Key::_1, gdk::Key::exclam), NONE), brush);
    assert_eq!(pick((gdk::Key::ampersand, gdk::Key::_1), NONE), brush);
}

#[test]
fn shift_and_a_digit_picks_nothing_now_that_the_tray_is_gone() {
    assert_eq!(pick((gdk::Key::_1, gdk::Key::exclam), SHIFT), None);
    assert_eq!(pick((gdk::Key::_4, gdk::Key::dollar), SHIFT), None);
}

#[test]
fn a_digit_no_tool_has_picks_nothing() {
    let us_six = (gdk::Key::_6, gdk::Key::asciicircum);
    assert_eq!(pick(us_six, NONE), None);
    assert_eq!(pick(us_six, SHIFT), None);
}

#[test]
fn caps_lock_does_not_change_the_pick() {
    let lock = gdk::ModifierType::LOCK_MASK;
    let us_four = (gdk::Key::_4, gdk::Key::dollar);
    assert_eq!(pick(us_four, lock), Some(UiTool::Paint(Tool::Rag)));
    assert_eq!(pick(us_four, SHIFT | lock), None);
}

#[test]
fn other_modifiers_keys_without_a_digit_and_the_keypad_pick_nothing_here() {
    let us_one = (gdk::Key::_1, gdk::Key::exclam);
    for modifiers in [
        gdk::ModifierType::CONTROL_MASK,
        SHIFT | gdk::ModifierType::CONTROL_MASK,
        SHIFT | gdk::ModifierType::ALT_MASK,
        gdk::ModifierType::SUPER_MASK,
    ] {
        assert_eq!(pick(us_one, modifiers), None, "{modifiers:?}");
    }
    assert_eq!(pick((gdk::Key::h, gdk::Key::H), NONE), None);
    assert_eq!(pick((gdk::Key::KP_End, gdk::Key::KP_1), NONE), None);
    assert_eq!(pick((gdk::Key::KP_End, gdk::Key::KP_1), SHIFT), None);
    assert_eq!(digit_pick(None, None, NONE), None);
}

#[test]
fn only_the_number_row_digits_are_number_row_digits() {
    for key in [gdk::Key::_0, gdk::Key::_1, gdk::Key::_9] {
        assert!(is_number_row_digit(key), "{key:?}");
    }
    for key in [gdk::Key::KP_1, gdk::Key::exclam, gdk::Key::h, gdk::Key::r] {
        assert!(!is_number_row_digit(key), "{key:?}");
    }
}
