use std::path::Path;

use super::*;

const PAINTING: &str = "/tmp/study.cadmium";
const IMAGE: &str = "/tmp/study.png";

fn saving_painting() -> Closing {
    let mut closing = Closing::default();
    assert!(closing.begin_write(Path::new(PAINTING), WriteKind::Painting));
    closing
}

#[test]
fn a_saved_painting_closes_at_once() {
    assert_eq!(Closing::default().request_close(|| false), CloseStep::Close);
}

#[test]
fn unsaved_edits_ask_before_closing() {
    assert_eq!(Closing::default().request_close(|| true), CloseStep::Ask);
}

#[test]
fn a_second_close_while_asking_does_not_ask_again() {
    let mut closing = Closing::default();
    closing.request_close(|| true);
    assert_eq!(closing.request_close(|| true), CloseStep::AlreadyAsking);
}

#[test]
fn keeping_the_painting_lets_a_later_close_ask_again() {
    let mut closing = Closing::default();
    closing.request_close(|| true);
    closing.discard_answered(false);
    assert_eq!(closing.request_close(|| true), CloseStep::Ask);
}

#[test]
fn discarding_the_edits_closes() {
    let mut closing = Closing::default();
    closing.request_close(|| true);
    closing.discard_answered(true);
    assert_eq!(closing.request_close(|| true), CloseStep::Close);
}

#[test]
fn a_confirmed_close_skips_the_question() {
    let mut closing = Closing::default();
    closing.confirm();
    assert_eq!(closing.request_close(|| true), CloseStep::Close);
}

#[test]
fn closing_during_a_write_waits_for_it() {
    let mut closing = saving_painting();
    assert_eq!(closing.request_close(|| false), CloseStep::WaitForWrites);
    assert!(closing.still_waiting_for_writes());
}

#[test]
fn the_window_closes_once_the_last_write_finishes() {
    let mut closing = saving_painting();
    assert!(closing.begin_write(Path::new(IMAGE), WriteKind::Image));
    closing.request_close(|| false);
    assert_eq!(
        closing.finish_write(Path::new(PAINTING)),
        AfterWrite::StillWriting
    );
    assert_eq!(closing.finish_write(Path::new(IMAGE)), AfterWrite::CloseNow);
    assert!(!closing.still_waiting_for_writes());
}

#[test]
fn a_write_finishing_without_a_close_request_leaves_the_window_open() {
    let mut closing = saving_painting();
    assert_eq!(closing.finish_write(Path::new(PAINTING)), AfterWrite::Idle);
}

#[test]
fn the_close_after_writes_happens_only_once() {
    let mut closing = saving_painting();
    closing.request_close(|| false);
    closing.finish_write(Path::new(PAINTING));
    assert!(closing.begin_write(Path::new(PAINTING), WriteKind::Painting));
    assert_eq!(closing.finish_write(Path::new(PAINTING)), AfterWrite::Idle);
}

#[test]
fn abandoning_the_writes_closes_even_while_they_run() {
    let mut closing = saving_painting();
    closing.request_close(|| true);
    closing.abandon_writes();
    assert_eq!(closing.request_close(|| true), CloseStep::Close);
}

#[test]
fn abandoning_does_not_look_at_the_edits() {
    let mut closing = Closing::default();
    closing.abandon_writes();
    let step = closing.request_close(|| panic!("the edits need not be read"));
    assert_eq!(step, CloseStep::Close);
}

#[test]
fn the_same_path_cannot_be_written_twice_at_once() {
    let mut closing = saving_painting();
    assert!(!closing.begin_write(Path::new(PAINTING), WriteKind::Painting));
}

#[test]
fn a_running_painting_save_counts_as_work_that_could_be_lost() {
    assert!(saving_painting().work_would_be_lost(false));
    let mut exporting = Closing::default();
    exporting.begin_write(Path::new(IMAGE), WriteKind::Image);
    assert!(!exporting.work_would_be_lost(false));
    assert!(exporting.work_would_be_lost(true));
}

#[test]
fn writes_alone_do_not_hold_a_close() {
    assert!(!saving_painting().still_waiting_for_writes());
}

#[test]
fn a_write_started_while_asking_makes_the_next_close_wait() {
    let mut closing = Closing::default();
    closing.request_close(|| true);
    assert!(closing.begin_write(Path::new(PAINTING), WriteKind::Painting));
    assert_eq!(closing.request_close(|| true), CloseStep::WaitForWrites);
    assert!(closing.still_waiting_for_writes());
}

#[test]
fn a_confirmed_close_still_waits_for_running_writes() {
    let mut closing = saving_painting();
    closing.confirm();
    assert_eq!(closing.request_close(|| true), CloseStep::WaitForWrites);
}
