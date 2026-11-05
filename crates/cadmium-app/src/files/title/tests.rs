use super::*;

#[test]
fn untitled_painting_is_named_sem_titulo() {
    assert_eq!(document_name(None), "Untitled");
    assert_eq!(window_title(None), "Cadmium");
}

#[test]
fn saved_painting_titles_the_window_with_its_name() {
    let file = Path::new("/home/painter/Portrait of Ana.cadmium");
    assert_eq!(document_name(Some(file)), "Portrait of Ana");
    assert_eq!(window_title(Some(file)), "Portrait of Ana — Cadmium");
}
