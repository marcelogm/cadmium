use super::{BRISTLE_SETTLE_MS, BristleRig, LayoutKey, LayoutRequest};
use crate::brush::HeadSpec;
use crate::engine::dab_record::DabRecord;
use crate::params::SimParams;
use crate::stroke::{Dab, Tool};

const REACH_PX: f32 = 30.0;

fn pressed_dab() -> Dab {
    Dab {
        x: 100.0,
        y: 80.0,
        radius: 20.0,
        pressure: 1.0,
        altitude: std::f32::consts::FRAC_PI_2,
        azimuth: 0.0,
        direction: 0.0,
        speed: 0.0,
        time_ms: 0.0,
    }
}

fn rig_after_one_pressed_dab() -> (BristleRig, DabRecord) {
    let mut rig = BristleRig::new();
    rig.take_up(Tool::Brush);
    rig.reach_px = REACH_PX;
    rig.begin_stroke();
    let mut record: DabRecord = bytemuck::Zeroable::zeroed();
    rig.advance(&mut record, &pressed_dab(), &SimParams::default());
    (rig, record)
}

#[test]
fn a_reset_forgets_the_last_dab_so_settling_waits_for_a_fresh_one() {
    let (mut rig, _) = rig_after_one_pressed_dab();
    assert!(rig.settle(16.0).is_some());
    rig.reset();
    assert!(rig.settle(32.0).is_none());
}

#[test]
fn while_the_stroke_is_open_a_settle_holds_the_handle_where_it_pressed() {
    let (mut rig, pressed) = rig_after_one_pressed_dab();
    let settled = rig.settle(16.0).unwrap();
    assert_eq!(settled.handle_height_px, pressed.handle_height_px);
    assert_eq!(settled.dt_seconds, 0.016);
}

#[test]
fn once_lifted_a_settle_raises_the_handle_by_one_bristle_length() {
    let (mut rig, pressed) = rig_after_one_pressed_dab();
    rig.lift();
    let settled = rig.settle(16.0).unwrap();
    assert_eq!(
        settled.handle_height_px,
        pressed.handle_height_px + REACH_PX
    );
}

#[test]
fn a_settle_takes_no_step_until_time_has_passed_since_the_last_one() {
    let (mut rig, _) = rig_after_one_pressed_dab();
    assert!(rig.settle(0.0).is_none());
    assert!(rig.settle(16.0).is_some());
    assert!(rig.settle(16.0).is_none());
    assert!(rig.settle(8.0).is_none());
}

#[test]
fn a_held_brush_settles_until_a_settle_period_after_its_last_dab() {
    let (rig, _) = rig_after_one_pressed_dab();
    assert!(rig.settling(BRISTLE_SETTLE_MS - 1.0));
    assert!(!rig.settling(BRISTLE_SETTLE_MS));
}

#[test]
fn a_lifted_brush_settles_until_a_settle_period_after_its_first_lifted_step() {
    let (mut rig, _) = rig_after_one_pressed_dab();
    rig.lift();
    let lifted_ms = 5000.0;
    assert!(rig.settling(lifted_ms));
    rig.settle(lifted_ms).unwrap();
    assert!(rig.settling(lifted_ms + BRISTLE_SETTLE_MS - 1.0));
    assert!(!rig.settling(lifted_ms + BRISTLE_SETTLE_MS));
}

#[test]
fn a_brush_put_down_for_another_tool_does_not_settle() {
    let (mut rig, _) = rig_after_one_pressed_dab();
    rig.put_down();
    assert!(!rig.settling(16.0));
    assert!(rig.settle(16.0).is_none());
}

fn key_of(head: HeadSpec) -> LayoutKey {
    LayoutKey::of(&LayoutRequest {
        head,
        ferrule_radius_px: 20.0,
        seed: 7,
    })
}

#[test]
fn a_change_of_point_alone_keeps_the_layout() {
    let pointed = HeadSpec {
        point: 0.1,
        ..HeadSpec::ROUND
    };
    assert_eq!(key_of(pointed), key_of(HeadSpec::ROUND));
    let domed = HeadSpec {
        dome: 0.5,
        ..HeadSpec::ROUND
    };
    assert_ne!(key_of(domed), key_of(HeadSpec::ROUND));
}
