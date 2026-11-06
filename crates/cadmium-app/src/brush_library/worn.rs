//! Which library brush the painter is wearing.

use cadmium_engine::brush::{BrushSettings, BrushSpec};

use super::BrushLibrary;

const CUSTOM_BRUSH_NAME: &str = "Custom Brush";

/// The library brush whose head and hair the settings wear, if any.
///
/// `remembered` is the brush last picked from the library; it wins over an earlier brush with
/// the same head and hair, as long as it still matches what is worn.
pub(crate) fn worn_brush<'a>(
    brushes: impl IntoIterator<Item = &'a BrushSpec>,
    remembered: Option<usize>,
    settings: &BrushSettings,
) -> Option<usize> {
    let mut first = None;
    let worn = brushes
        .into_iter()
        .enumerate()
        .filter(|(_, brush)| brush.head == settings.head && brush.hair == settings.hair);
    for (index, _) in worn {
        if Some(index) == remembered {
            return Some(index);
        }
        first.get_or_insert(index);
    }
    first
}

/// The name the worn brush is exported under, and its library index when it is a library brush.
pub(crate) fn export_name(
    brushes: &BrushLibrary,
    remembered: Option<usize>,
    settings: &BrushSettings,
) -> (String, Option<usize>) {
    let index = worn_brush(brushes.all(), remembered, settings);
    let name = index
        .and_then(|index| brushes.get(index))
        .map_or_else(|| CUSTOM_BRUSH_NAME.to_string(), |brush| brush.name.clone());
    (name, index)
}

/// Where a remembered library index points once the brush at `deleted` is gone.
pub(crate) fn index_after_delete(remembered: Option<usize>, deleted: usize) -> Option<usize> {
    match remembered? {
        index if index == deleted => None,
        index if index > deleted => Some(index - 1),
        index => Some(index),
    }
}

#[cfg(test)]
mod tests;
