use super::*;

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
fn six_chooses_the_tube_on_both_rows() {
    assert!(TOOL_KEYS.contains(&(gdk::Key::_6, UiTool::Tube)));
    assert!(KEYPAD_TOOL_KEYS.contains(&(gdk::Key::KP_6, UiTool::Tube)));
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
