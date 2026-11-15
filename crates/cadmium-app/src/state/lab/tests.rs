use super::*;
use crate::state::test_support::{painting_state, present_one_frame};

#[test]
fn pigment_view_beyond_the_palette_shows_the_last_slot() {
    assert_eq!(
        clamp_view_mode(ViewMode::Pigment(6), 4),
        ViewMode::Pigment(3)
    );
    assert_eq!(
        clamp_view_mode(ViewMode::Pigment(2), 4),
        ViewMode::Pigment(2)
    );
    assert_eq!(clamp_view_mode(ViewMode::Pigment(0), 0), ViewMode::Color);
    assert_eq!(clamp_view_mode(ViewMode::Height, 0), ViewMode::Height);
}

#[test]
fn only_render_params_repaint_the_whole_canvas() {
    let render = SimParams::specs()
        .iter()
        .find(|spec| spec.group == ParamGroup::Render)
        .unwrap();
    let transfer = SimParams::specs()
        .iter()
        .find(|spec| spec.group == ParamGroup::Transfer)
        .unwrap();
    assert!(is_render_param(render.key));
    assert!(!is_render_param(transfer.key));
    assert!(!is_render_param("no_such_param"));
}

#[test]
fn a_render_param_reshades_the_painting() {
    let mut state = painting_state();
    present_one_frame(&mut state);
    assert!(!state.session.has_pending_work());
    let render = SimParams::specs()
        .iter()
        .find(|spec| spec.group == ParamGroup::Render)
        .unwrap();
    state.set_param(render.key, render.default);
    assert!(state.session.has_pending_work());
    present_one_frame(&mut state);
    assert!(!state.session.has_pending_work());
    state.reset_params();
    assert!(state.session.has_pending_work());
}
