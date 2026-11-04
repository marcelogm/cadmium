use super::*;

#[test]
fn untitled_painting_is_named_sem_titulo() {
    assert_eq!(document_name(None), "Sem título");
    assert_eq!(window_title(None), "Cadmium");
}

#[test]
fn saved_painting_titles_the_window_with_its_name() {
    let file = Path::new("/home/pintor/Retrato de Ana.cadmium");
    assert_eq!(document_name(Some(file)), "Retrato de Ana");
    assert_eq!(window_title(Some(file)), "Retrato de Ana — Cadmium");
}
