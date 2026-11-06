use super::import_toast;
use crate::brush_library::Imported;

fn imported(renamed: bool, adjusted: bool, saved: bool) -> Imported {
    Imported {
        index: 15,
        renamed,
        adjusted,
        saved,
    }
}

#[test]
fn a_clean_import_names_the_brush() {
    let toast = import_toast("Soft Dome", &imported(false, false, true));
    assert_eq!(toast, "Imported Soft Dome");
}

#[test]
fn an_import_mentions_everything_that_happened_to_the_brush() {
    let toast = import_toast("Soft Dome (2)", &imported(true, true, false));
    assert_eq!(
        toast,
        "Imported Soft Dome (2) (renamed); some values were out of range and were adjusted; \
         couldn't save it to your brush folder, it will be gone after you quit"
    );
}
