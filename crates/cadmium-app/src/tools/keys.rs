use gtk::gdk;

use super::UiTool;

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
