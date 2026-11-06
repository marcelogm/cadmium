use cadmium_engine::brush::{BrushSettings, BrushSpec, HeadSpec, brush_to_json, factory_brushes};

use super::{BrushLibrary, LibraryError, export_name, file_name_for};
use crate::test_dir::TempDir;

fn names(library: &BrushLibrary) -> Vec<String> {
    library.all().map(|brush| brush.name.clone()).collect()
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
    let dir = TempDir::new();
    let (library, warnings) = BrushLibrary::load(dir.path());
    assert!(warnings.is_empty());
    let names: Vec<_> = library.all().map(|b| b.name.clone()).collect();
    let factory: Vec<_> = factory_brushes().into_iter().map(|b| b.name).collect();
    assert_eq!(names, factory);
}

#[test]
fn an_imported_brush_is_saved_and_listed_after_the_factory_ones() {
    let dir = TempDir::new();
    let (mut library, _) = BrushLibrary::load(dir.path());
    let imported = library
        .import(&brush_to_json(&custom("Soft Flat")))
        .unwrap();
    assert_eq!(imported.index, factory_brushes().len());
    assert!(imported.saved() && !imported.renamed && !imported.adjusted);
    assert!(library.is_user(imported.index));
    let (reloaded, warnings) = BrushLibrary::load(dir.path());
    assert!(warnings.is_empty());
    assert_eq!(reloaded.all().last().unwrap(), &custom("Soft Flat"));
}

#[test]
fn a_clashing_name_gets_a_number() {
    let mut library = BrushLibrary::factory_only();
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
    let dir = TempDir::new();
    let (mut library, _) = BrushLibrary::load(dir.path());
    library
        .import(&brush_to_json(&custom("Soft/Flat")))
        .unwrap();
    library
        .import(&brush_to_json(&custom("Soft:Flat")))
        .unwrap();
    assert_eq!(dir.files(), ["soft-flat-2.cadbrush", "soft-flat.cadbrush"]);
    let (reloaded, _) = BrushLibrary::load(dir.path());
    let reloaded = names(&reloaded);
    assert!(reloaded.contains(&"Soft/Flat".to_string()));
    assert!(reloaded.contains(&"Soft:Flat".to_string()));
}

#[test]
fn a_bad_file_in_the_folder_is_skipped_with_a_warning() {
    let dir = TempDir::new();
    std::fs::write(dir.0.join("broken.cadbrush"), "{ nope").unwrap();
    std::fs::write(dir.0.join("notes.txt"), "ignored").unwrap();
    let (library, warnings) = BrushLibrary::load(dir.path());
    assert_eq!(library.all().count(), factory_brushes().len());
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("broken.cadbrush"));
}

#[test]
fn a_bad_import_is_refused_and_nothing_changes() {
    let mut library = BrushLibrary::factory_only();
    let before = library.all().count();
    assert!(matches!(
        library.import("{ nope"),
        Err(LibraryError::Unreadable(_))
    ));
    assert_eq!(library.all().count(), before);
}

#[test]
fn deleting_a_user_brush_removes_its_file() {
    let dir = TempDir::new();
    let (mut library, _) = BrushLibrary::load(dir.path());
    let index = library
        .import(&brush_to_json(&custom("Gone")))
        .unwrap()
        .index;
    library.delete(index).unwrap();
    assert_eq!(library.all().count(), factory_brushes().len());
    assert!(dir.files().is_empty());
    let (reloaded, _) = BrushLibrary::load(dir.path());
    assert!(!names(&reloaded).contains(&"Gone".to_string()));
}

#[test]
fn deleting_a_brush_whose_file_is_already_gone_succeeds() {
    let dir = TempDir::new();
    let (mut library, _) = BrushLibrary::load(dir.path());
    let index = library
        .import(&brush_to_json(&custom("Gone")))
        .unwrap()
        .index;
    std::fs::remove_file(dir.0.join("gone.cadbrush")).unwrap();
    library.delete(index).unwrap();
    assert_eq!(library.all().count(), factory_brushes().len());
}

#[test]
fn a_brush_whose_file_cannot_be_removed_stays_listed() {
    let dir = TempDir::new();
    let (mut library, _) = BrushLibrary::load(dir.path());
    let index = library
        .import(&brush_to_json(&custom("Stuck")))
        .unwrap()
        .index;
    let path = dir.0.join("stuck.cadbrush");
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("inside"), "").unwrap();
    assert!(matches!(library.delete(index), Err(LibraryError::Io(_))));
    assert_eq!(library.get(index).unwrap().name, "Stuck");
}

#[test]
fn a_factory_only_library_keeps_imports_for_the_session() {
    let mut library = BrushLibrary::factory_only();
    let imported = library
        .import(&brush_to_json(&custom("Session Only")))
        .unwrap();
    assert!(!imported.saved());
    assert!(library.is_user(imported.index));
    library.delete(imported.index).unwrap();
    assert_eq!(library.all().count(), factory_brushes().len());
}

#[test]
fn an_import_never_overwrites_a_file_already_there() {
    let dir = TempDir::new();
    let (mut library, _) = BrushLibrary::load(dir.path());
    std::fs::write(dir.0.join("soft-flat.cadbrush"), "not mine").unwrap();
    library
        .import(&brush_to_json(&custom("Soft Flat")))
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.0.join("soft-flat.cadbrush")).unwrap(),
        "not mine"
    );
    assert_eq!(dir.files(), ["soft-flat-2.cadbrush", "soft-flat.cadbrush"]);
}

#[test]
fn brushes_on_disk_with_the_same_name_are_told_apart() {
    let dir = TempDir::new();
    for file in ["a.cadbrush", "b.cadbrush"] {
        std::fs::write(dir.0.join(file), brush_to_json(&custom("Twin"))).unwrap();
    }
    let (library, _) = BrushLibrary::load(dir.path());
    let names = names(&library);
    assert!(names.contains(&"Twin".to_string()));
    assert!(names.contains(&"Twin (2)".to_string()));
}

#[test]
fn factory_brushes_cannot_be_deleted() {
    let mut library = BrushLibrary::factory_only();
    assert!(matches!(
        library.delete(0),
        Err(LibraryError::NotAUserBrush)
    ));
}

#[test]
fn an_unwritable_folder_keeps_the_brush_for_the_session() {
    let dir = TempDir::new();
    let blocker = dir.0.join("a-file-not-a-folder");
    std::fs::write(&blocker, "").unwrap();
    let (mut library, _) = BrushLibrary::load(blocker);
    let imported = library
        .import(&brush_to_json(&custom("Session Only")))
        .unwrap();
    assert!(imported.save_error.is_some());
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

#[test]
fn long_file_names_are_cut_without_a_trailing_dash() {
    let slug = "a".repeat(63);
    assert_eq!(
        file_name_for(&format!("{slug} bcd")),
        format!("{slug}.cadbrush")
    );
    assert_eq!(
        file_name_for(&"x".repeat(200)),
        format!("{}.cadbrush", "x".repeat(super::MAX_SLUG_CHARS))
    );
}

#[test]
fn the_default_settings_export_under_the_first_factory_name() {
    let library = BrushLibrary::factory_only();
    let settings = BrushSettings::default();
    assert_eq!(
        export_name(&library, None, &settings),
        ("Round · Synthetic".to_string(), Some(0))
    );
}

#[test]
fn tuned_settings_export_as_a_custom_brush() {
    let library = BrushLibrary::factory_only();
    let mut settings = BrushSettings::default();
    settings.hair.clumping = 0.123;
    assert_eq!(
        export_name(&library, None, &settings),
        ("Custom Brush".to_string(), None)
    );
}

#[test]
fn an_imported_brush_exports_under_its_own_name() {
    let mut library = BrushLibrary::factory_only();
    let imported = library
        .import(&brush_to_json(&custom("Soft Dome")))
        .unwrap();
    let mut settings = BrushSettings::default();
    settings.wear(library.get(imported.index).unwrap());
    assert_eq!(
        export_name(&library, None, &settings),
        ("Soft Dome".to_string(), Some(imported.index))
    );
}

#[test]
fn a_re_imported_factory_brush_exports_under_its_new_name() {
    let mut library = BrushLibrary::factory_only();
    let imported = library
        .import(&brush_to_json(&factory_brushes()[0]))
        .unwrap();
    let settings = BrushSettings::default();
    assert_eq!(
        export_name(&library, Some(imported.index), &settings),
        ("Round · Synthetic (2)".to_string(), Some(imported.index))
    );
}
