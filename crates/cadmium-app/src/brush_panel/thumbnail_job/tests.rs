use super::{ThumbnailJob, TickOutcome};

#[test]
fn an_idle_job_starts_only_when_a_thumbnail_is_missing() {
    assert!(ThumbnailJob::Idle.starts(true, false));
    assert!(!ThumbnailJob::Idle.starts(false, false));
}

#[test]
fn a_pending_job_is_not_started_twice() {
    assert!(!ThumbnailJob::Pending.starts(true, false));
}

#[test]
fn painting_keeps_the_job_running_until_nothing_is_missing() {
    let job = TickOutcome::Painted.job_after();
    assert_eq!(job, ThumbnailJob::Pending);
    assert!(job.keeps_running());
    let job = TickOutcome::NothingMissing.job_after();
    assert_eq!(job, ThumbnailJob::Idle);
    assert!(!job.keeps_running());
    assert!(job.starts(true, false));
}

#[test]
fn a_failed_job_stops_and_never_restarts() {
    let job = TickOutcome::Failed.job_after();
    assert_eq!(job, ThumbnailJob::Failed);
    assert!(!job.keeps_running());
    assert!(!job.starts(true, false));
}

#[test]
fn an_open_editor_pauses_the_job_until_it_closes() {
    let job = TickOutcome::Paused.job_after();
    assert_eq!(job, ThumbnailJob::Idle);
    assert!(!job.keeps_running());
    assert!(!job.starts(true, true));
    assert!(job.starts(true, false));
}
