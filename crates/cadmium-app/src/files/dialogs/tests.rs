use super::*;

#[test]
fn chosen_name_without_the_extension_gets_it() {
    assert_eq!(
        with_extension(PathBuf::from("/tmp/study"), PAINTING_EXTENSION),
        PathBuf::from("/tmp/study.cadmium")
    );
    assert_eq!(
        with_extension(PathBuf::from("/tmp/study.v2"), IMAGE_EXTENSION),
        PathBuf::from("/tmp/study.v2.png")
    );
}

#[test]
fn appended_extension_landing_on_an_existing_file_asks_before_replacing_it() {
    let chosen = Path::new("/tmp/study");
    let target = with_extension(chosen.to_path_buf(), PAINTING_EXTENSION);
    assert!(overwrite_needs_confirmation(chosen, &target, true));
}

#[test]
fn appended_extension_on_a_new_name_writes_without_asking() {
    let chosen = Path::new("/tmp/study");
    let target = with_extension(chosen.to_path_buf(), PAINTING_EXTENSION);
    assert!(!overwrite_needs_confirmation(chosen, &target, false));
}

#[test]
fn name_already_confirmed_by_the_file_chooser_is_not_asked_again() {
    let chosen = Path::new("/tmp/study.cadmium");
    let target = with_extension(chosen.to_path_buf(), PAINTING_EXTENSION);
    assert!(!overwrite_needs_confirmation(chosen, &target, true));
}

#[test]
fn chosen_name_with_the_extension_in_any_case_is_kept() {
    assert_eq!(
        with_extension(PathBuf::from("/tmp/study.CADMIUM"), PAINTING_EXTENSION),
        PathBuf::from("/tmp/study.CADMIUM")
    );
}
