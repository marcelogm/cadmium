use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use cadmium_engine::brush::{BrushSpec, HeadSpec, brush_to_json, factory_brushes};

use super::{BrushLibrary, LibraryError, file_name_for};

static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cadmium-brushes-{}-{}",
        std::process::id(),
        NEXT_DIR.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

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

#[test]
fn a_fresh_library_lists_only_the_factory_brushes() {
    let (library, warnings) = BrushLibrary::load(temp_dir());
    assert!(warnings.is_empty());
    let names: Vec<_> = library.all().map(|b| b.name.clone()).collect();
    let factory: Vec<_> = factory_brushes().into_iter().map(|b| b.name).collect();
    assert_eq!(names, factory);
}

#[test]
fn an_imported_brush_is_saved_and_listed_after_the_factory_ones() {
    let dir = temp_dir();
    let (mut library, _) = BrushLibrary::load(dir.clone());
    let imported = library
        .import(&brush_to_json(&custom("Soft Flat")))
        .unwrap();
    assert_eq!(imported.index, factory_brushes().len());
    assert!(imported.saved && !imported.renamed && !imported.adjusted);
    assert!(library.is_user(imported.index));
    let (reloaded, warnings) = BrushLibrary::load(dir);
    assert!(warnings.is_empty());
    assert_eq!(reloaded.all().last().unwrap(), &custom("Soft Flat"));
}

#[test]
fn a_clashing_name_gets_a_number() {
    let (mut library, _) = BrushLibrary::load(temp_dir());
    let factory_name = factory_brushes()[0].name.clone();
    let first = library
        .import(&brush_to_json(&custom(&factory_name)))
        .unwrap();
    let second = library
        .import(&brush_to_json(&custom(&factory_name)))
        .unwrap();
    assert!(first.renamed && second.renamed);
    let names: Vec<_> = library.all().map(|b| b.name.clone()).collect();
    assert!(names.contains(&format!("{factory_name} (2)")));
    assert!(names.contains(&format!("{factory_name} (3)")));
}

#[test]
fn two_brushes_whose_names_slug_alike_get_separate_files() {
    let dir = temp_dir();
    let (mut library, _) = BrushLibrary::load(dir.clone());
    library
        .import(&brush_to_json(&custom("Soft/Flat")))
        .unwrap();
    library
        .import(&brush_to_json(&custom("Soft:Flat")))
        .unwrap();
    assert_eq!(std::fs::read_dir(dir).unwrap().count(), 2);
}

#[test]
fn a_bad_file_in_the_folder_is_skipped_with_a_warning() {
    let dir = temp_dir();
    std::fs::write(dir.join("broken.cadbrush"), "{ nope").unwrap();
    std::fs::write(dir.join("notes.txt"), "ignored").unwrap();
    let (library, warnings) = BrushLibrary::load(dir);
    assert_eq!(library.all().count(), factory_brushes().len());
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("broken.cadbrush"));
}

#[test]
fn a_bad_import_is_refused_and_nothing_changes() {
    let (mut library, _) = BrushLibrary::load(temp_dir());
    let before = library.all().count();
    assert!(matches!(
        library.import("{ nope"),
        Err(LibraryError::Unreadable(_))
    ));
    assert_eq!(library.all().count(), before);
}

#[test]
fn deleting_a_user_brush_removes_its_file() {
    let dir = temp_dir();
    let (mut library, _) = BrushLibrary::load(dir.clone());
    let index = library
        .import(&brush_to_json(&custom("Gone")))
        .unwrap()
        .index;
    library.delete(index).unwrap();
    assert_eq!(library.all().count(), factory_brushes().len());
    assert_eq!(std::fs::read_dir(dir).unwrap().count(), 0);
}

#[test]
fn factory_brushes_cannot_be_deleted() {
    let (mut library, _) = BrushLibrary::load(temp_dir());
    assert!(matches!(
        library.delete(0),
        Err(LibraryError::NotAUserBrush)
    ));
}

#[test]
fn an_unwritable_folder_keeps_the_brush_for_the_session() {
    let blocker = temp_dir().join("a-file-not-a-folder");
    std::fs::write(&blocker, "").unwrap();
    let (mut library, _) = BrushLibrary::load(blocker);
    let imported = library
        .import(&brush_to_json(&custom("Session Only")))
        .unwrap();
    assert!(!imported.saved);
    assert!(library.is_user(imported.index));
}

#[test]
fn file_names_are_safe_slugs() {
    assert_eq!(
        file_name_for("Round · Kolinsky Sable"),
        "round-kolinsky-sable.cadbrush"
    );
    assert_eq!(file_name_for("../../etc"), "etc.cadbrush");
    assert_eq!(file_name_for("···"), "brush.cadbrush");
}
