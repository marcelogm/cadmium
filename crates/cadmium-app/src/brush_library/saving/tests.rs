use cadmium_engine::brush::{BrushSpec, HeadSpec, brush_to_json, factory_brushes};

use super::super::test_dir::TempDir;
use crate::brush_library::{BrushLibrary, LibraryError};

fn custom(name: &str) -> BrushSpec {
    BrushSpec {
        name: name.into(),
        size_px: 30.0,
        head: HeadSpec {
            dome: 0.3,
            ..HeadSpec::FLAT
        },
        hair: Default::default(),
    }
}

fn resized(brush: BrushSpec, size_px: f32) -> BrushSpec {
    BrushSpec { size_px, ..brush }
}

/// A library in `dir` holding one user brush called `name`, and that brush's index.
fn holding(dir: &TempDir, name: &str) -> (BrushLibrary, usize) {
    let (mut library, _) = BrushLibrary::load(dir.path());
    let index = library.import(&brush_to_json(&custom(name))).unwrap().index;
    (library, index)
}

#[test]
fn saving_a_user_brush_rewrites_its_own_file() {
    let dir = TempDir::new();
    let (mut library, index) = holding(&dir, "Soft Flat");
    let edited = resized(custom("Soft Flat"), 72.0);
    let saved = library.save(index, &custom("Soft Flat"), &edited).unwrap();
    assert_eq!(saved.index, index);
    assert!(!saved.renamed && saved.save_error.is_none());
    assert_eq!(dir.files(), ["soft-flat.cadbrush"]);
    let (reloaded, warnings) = BrushLibrary::load(dir.path());
    assert!(warnings.is_empty());
    assert_eq!(reloaded.get(index), Some(&edited));
}

#[test]
fn a_renamed_brush_keeps_its_file() {
    let dir = TempDir::new();
    let (mut library, index) = holding(&dir, "Soft Flat");
    let saved = library
        .save(index, &custom("Soft Flat"), &custom("Wide Flat"))
        .unwrap();
    assert_eq!(saved.name, "Wide Flat");
    assert_eq!(dir.files(), ["soft-flat.cadbrush"]);
    let (reloaded, _) = BrushLibrary::load(dir.path());
    assert_eq!(reloaded.get(index).unwrap().name, "Wide Flat");
}

#[test]
fn keeping_its_own_name_is_no_clash() {
    let dir = TempDir::new();
    let (mut library, index) = holding(&dir, "Soft Flat");
    let edited = resized(custom("Soft Flat"), 50.0);
    let saved = library.save(index, &custom("Soft Flat"), &edited).unwrap();
    assert_eq!(saved.name, "Soft Flat");
    assert!(!saved.renamed);
}

#[test]
fn taking_another_brushs_name_numbers_it() {
    let dir = TempDir::new();
    let (mut library, index) = holding(&dir, "Soft Flat");
    library
        .import(&brush_to_json(&custom("Wide Flat")))
        .unwrap();
    let saved = library
        .save(index, &custom("Soft Flat"), &custom("Wide Flat"))
        .unwrap();
    assert_eq!(saved.name, "Wide Flat (2)");
    assert!(saved.renamed);
}

#[test]
fn a_brush_that_changed_meanwhile_is_not_saved() {
    let dir = TempDir::new();
    let (mut library, index) = holding(&dir, "Soft Flat");
    let stale = custom("Someone Else");
    let saved = library.save(index, &stale, &custom("Mine"));
    assert!(matches!(saved, Err(LibraryError::Changed)));
    assert_eq!(library.get(index), Some(&custom("Soft Flat")));
}

#[test]
fn a_factory_brush_is_never_saved_over() {
    let mut library = BrushLibrary::factory_only();
    let factory = factory_brushes().swap_remove(0);
    let saved = library.save(0, &factory, &custom("Mine"));
    assert!(matches!(saved, Err(LibraryError::Changed)));
    assert_eq!(library.get(0), Some(&factory));
}

#[test]
fn a_leftover_temporary_file_is_neither_listed_nor_kept() {
    let dir = TempDir::new();
    std::fs::write(dir.0.join(".soft-flat.cadbrush.tmp"), "half a brush").unwrap();
    let (mut library, index) = holding(&dir, "Soft Flat");
    assert_eq!(library.all().count(), factory_brushes().len() + 1);
    let edited = resized(custom("Soft Flat"), 50.0);
    library.save(index, &custom("Soft Flat"), &edited).unwrap();
    assert_eq!(dir.files(), ["soft-flat.cadbrush"]);
}

#[test]
fn an_unwritable_folder_keeps_the_saved_brush_for_the_session() {
    let dir = TempDir::new();
    let blocker = dir.0.join("not-a-folder");
    std::fs::write(&blocker, "").unwrap();
    let (mut library, _) = BrushLibrary::load(blocker);
    let index = library
        .import(&brush_to_json(&custom("Soft Flat")))
        .unwrap()
        .index;
    let edited = resized(custom("Soft Flat"), 72.0);
    let saved = library.save(index, &custom("Soft Flat"), &edited).unwrap();
    assert!(saved.save_error.is_some());
    assert_eq!(library.get(index), Some(&edited));
}

#[test]
fn a_brush_without_a_file_gets_one_once_the_folder_is_writable() {
    let dir = TempDir::new();
    let folder = dir.0.join("brushes");
    std::fs::write(&folder, "").unwrap();
    let (mut library, _) = BrushLibrary::load(folder.clone());
    let index = library
        .import(&brush_to_json(&custom("Soft Flat")))
        .unwrap()
        .index;
    std::fs::remove_file(&folder).unwrap();
    let edited = resized(custom("Soft Flat"), 72.0);
    let saved = library.save(index, &custom("Soft Flat"), &edited).unwrap();
    assert!(saved.save_error.is_none());
    let (reloaded, _) = BrushLibrary::load(folder);
    assert_eq!(reloaded.all().last(), Some(&edited));
}

#[test]
fn a_new_brush_gets_its_own_file_and_a_free_name() {
    let dir = TempDir::new();
    let (mut library, _) = BrushLibrary::load(dir.path());
    let factory_name = factory_brushes()[0].name.clone();
    let saved = library.add_new(BrushSpec {
        name: factory_name.clone(),
        ..custom("")
    });
    assert_eq!(saved.name, format!("{factory_name} (2)"));
    assert!(saved.renamed && saved.save_error.is_none());
    assert!(library.is_user(saved.index));
    assert_eq!(dir.files().len(), 1);
}
