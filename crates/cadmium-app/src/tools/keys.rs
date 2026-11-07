use gtk::gdk;

use super::{TrayTool, UiTool};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
enum KeyRow {
    Main,
    Keypad,
}

pub(crate) const TOOL_KEYS: [(gdk::Key, UiTool); key_count(KeyRow::Main)] =
    bound_keys(KeyRow::Main);
pub(crate) const KEYPAD_TOOL_KEYS: [(gdk::Key, UiTool); key_count(KeyRow::Keypad)] =
    bound_keys(KeyRow::Keypad);

const fn keys_on(tool: UiTool, row: KeyRow) -> &'static [gdk::Key] {
    let profile = tool.profile();
    match row {
        KeyRow::Main => profile.keys,
        KeyRow::Keypad => profile.keypad_keys,
    }
}

const fn key_count(row: KeyRow) -> usize {
    let mut count = 0;
    let mut index = 0;
    while index < UiTool::ALL.len() {
        count += keys_on(UiTool::ALL[index], row).len();
        index += 1;
    }
    count
}

const fn bound_keys<const N: usize>(row: KeyRow) -> [(gdk::Key, UiTool); N] {
    let mut bound = [(gdk::Key::VoidSymbol, UiTool::Move); N];
    let mut next = 0;
    let mut index = 0;
    while index < UiTool::ALL.len() {
        let tool = UiTool::ALL[index];
        let keys = keys_on(tool, row);
        let mut key = 0;
        while key < keys.len() {
            bound[next] = (keys[key], tool);
            next += 1;
            key += 1;
        }
        index += 1;
    }
    bound
}

/// Whether `key` is a number-row digit; the window's digit keys take those (see
/// [`digit_pick`]), so the tool shortcuts leave them out.
pub(crate) fn is_number_row_digit(key: gdk::Key) -> bool {
    (gdk::Key::_0..=gdk::Key::_9).contains(&key)
}

/// What a number-row press picks: a digit picks the painting's tool, Shift and a digit the
/// tray's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DigitPick {
    Painting(UiTool),
    Tray(TrayTool),
}

const SHORTCUT_MODIFIERS: gdk::ModifierType = gdk::ModifierType::SHIFT_MASK
    .union(gdk::ModifierType::CONTROL_MASK)
    .union(gdk::ModifierType::ALT_MASK)
    .union(gdk::ModifierType::SUPER_MASK)
    .union(gdk::ModifierType::HYPER_MASK)
    .union(gdk::ModifierType::META_MASK);

/// The pick for a press on the key giving `unshifted` alone and `shifted` with Shift. The
/// physical key decides: its digit may sit on either level (`1`/`!` on US, `&`/`1` on
/// AZERTY), so Shift is read from `modifiers`, not from which level the digit is on.
pub(crate) fn digit_pick(
    unshifted: Option<gdk::Key>,
    shifted: Option<gdk::Key>,
    modifiers: gdk::ModifierType,
) -> Option<DigitPick> {
    let digit = [unshifted, shifted]
        .into_iter()
        .flatten()
        .find(|&key| is_number_row_digit(key))?;
    let modifiers = modifiers & SHORTCUT_MODIFIERS;
    if modifiers.is_empty() {
        TOOL_KEYS
            .into_iter()
            .find(|&(key, _)| key == digit)
            .map(|(_, tool)| DigitPick::Painting(tool))
    } else if modifiers == gdk::ModifierType::SHIFT_MASK {
        TrayTool::ALL
            .into_iter()
            .find(|tool| tool.key() == digit)
            .map(DigitPick::Tray)
    } else {
        None
    }
}
