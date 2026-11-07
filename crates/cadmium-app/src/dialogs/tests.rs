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
fn pencil_lists_both_of_its_keys() {
    assert_eq!(
        tool_accelerator(UiTool::Paint(crate::tools::Tool::Pencil)),
        "7 p"
    );
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
        let (space_held, modifiers) = match keys {
            "space" => (true, ModifierType::empty()),
            "<Control>" => (false, ModifierType::CONTROL_MASK),
            "<Shift>" => (false, ModifierType::SHIFT_MASK),
            other => panic!("{label}: unexpected keys {other}"),
        };
        assert_eq!(
            crate::input::nav_drag::held_mode(space_held, modifiers),
            Some(mode),
            "{label}"
        );
    }
}

#[test]
fn tray_tool_accelerators_are_shifted_digits() {
    let accelerators: Vec<_> = TrayTool::ALL
        .into_iter()
        .map(tray_tool_accelerator)
        .collect();
    assert_eq!(
        accelerators,
        ["<Shift>1", "<Shift>2", "<Shift>4", "<Shift>5", "<Shift>6"]
    );
}
