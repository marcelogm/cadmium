use super::Listening;

#[test]
fn an_open_editor_heeds_its_widgets() {
    assert!(Listening::default().heeds());
}

#[test]
fn filling_the_widgets_is_not_heeded() {
    let listening = Listening::default();
    listening.set_syncing(true);
    assert!(!listening.heeds());
    listening.set_syncing(false);
    assert!(listening.heeds());
}

#[test]
fn a_closing_editor_heeds_nothing_more() {
    let listening = Listening::default();
    listening.stop();
    assert!(!listening.heeds());
    listening.set_syncing(true);
    listening.set_syncing(false);
    assert!(!listening.heeds());
}

#[test]
fn only_a_stopped_editor_is_closing() {
    let listening = Listening::default();
    listening.set_syncing(true);
    assert!(!listening.is_closing());
    listening.stop();
    assert!(listening.is_closing());
}
