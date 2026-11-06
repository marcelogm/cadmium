use super::*;

#[test]
fn every_accelerator_is_listed_exactly_once() {
    for (action, _) in ACCELERATORS {
        let listed = ACTION_SHORTCUTS
            .iter()
            .flat_map(|(_, entries)| entries.iter())
            .filter(|(_, listed)| *listed == action)
            .count();
        assert_eq!(listed, 1, "{action}");
    }
}

#[test]
fn every_listed_action_has_an_accelerator() {
    for (_, entries) in ACTION_SHORTCUTS {
        for (label, action) in entries {
            assert!(action_accelerator(action).is_some(), "{label}: {action}");
        }
    }
}

#[test]
fn every_tool_key_is_listed() {
    for tool in UiTool::ALL {
        assert!(!tool_accelerator(tool).is_empty(), "{tool:?}");
    }
}

#[test]
fn rag_lists_both_of_its_keys() {
    assert_eq!(
        tool_accelerator(UiTool::Paint(crate::tools::Tool::Rag)),
        "4 r"
    );
}

#[test]
fn the_tube_answers_to_six() {
    assert_eq!(tool_accelerator(UiTool::Tube), "6");
}

#[test]
fn zoom_in_lists_both_of_its_accelerators() {
    assert_eq!(
        action_accelerator("win.zoom-in").as_deref(),
        Some("<Control>plus <Control>equal")
    );
}

#[test]
fn listed_navigation_keys_start_the_mode_they_name() {
    use gtk::gdk::ModifierType;
    // `gtk::accelerator_parse` needs GTK initialised, so the three spellings are read by hand.
    for (label, keys, mode) in NAVIGATION_GESTURES {
        let (modifiers, key) = match keys.strip_prefix("<Control>") {
            Some(key) => (ModifierType::CONTROL_MASK, key),
            None => match keys.strip_prefix("<Shift>") {
                Some(key) => (ModifierType::SHIFT_MASK, key),
                None => (ModifierType::empty(), keys),
            },
        };
        assert_eq!(key, "space", "{label}");
        assert_eq!(
            crate::input::nav_drag::space_mode(modifiers),
            mode,
            "{label}"
        );
    }
}
