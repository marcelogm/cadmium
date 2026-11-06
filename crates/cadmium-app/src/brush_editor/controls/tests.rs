use cadmium_engine::brush::{HeadSpec, default_brush};
use cadmium_studio::{HairPreset, HeadPreset};

use super::{Control, Part, position_of, preset_at};

fn every_control() -> Vec<Control> {
    Control::head()
        .chain(Control::hair())
        .chain(Control::hair_advanced())
        .collect()
}

/// A value a few steps above the control's low end, as the slider would land on it.
fn on_the_grid(control: Control) -> f32 {
    let [low, _] = control.range();
    let per_unit = control.steps_per_unit();
    let steps = (f64::from(low) * per_unit).round() + 7.0;
    (steps / per_unit) as f32
}

fn labels(controls: impl Iterator<Item = Control>) -> Vec<&'static str> {
    controls.map(Control::label).collect()
}

#[test]
fn the_editor_lists_eight_head_six_hair_and_six_advanced_controls() {
    assert_eq!(
        labels(Control::head()),
        [
            "Thickness",
            "Length",
            "Dome",
            "Slant",
            "Point",
            "Belly",
            "Fan",
            "Density"
        ]
    );
    assert_eq!(
        labels(Control::hair()),
        [
            "Stiffness",
            "Static Friction",
            "Kinetic Friction",
            "Clumping",
            "Wet Point",
            "Load Capacity"
        ]
    );
    assert_eq!(
        labels(Control::hair_advanced()),
        [
            "Damping",
            "Side Drag",
            "Bend Memory",
            "Point Release",
            "Point Film",
            "Wicking"
        ]
    );
}

#[test]
fn every_control_reads_back_what_it_sets_and_moves_nothing_else() {
    for control in every_control() {
        let mut brush = default_brush();
        let on_grid = on_the_grid(control);
        control.set(&mut brush, on_grid);
        assert_eq!(control.get(&brush), on_grid, "{control:?}");
        for other in every_control()
            .into_iter()
            .filter(|&other| other != control)
        {
            assert_eq!(
                other.get(&brush),
                other.get(&default_brush()),
                "{control:?} moved {other:?}"
            );
        }
    }
}

#[test]
fn values_are_kept_in_range() {
    let mut brush = default_brush();
    Control::Thickness.set(&mut brush, 5.0);
    assert_eq!(brush.head.thickness, HeadSpec::THICKNESS_RANGE[1]);
    Control::Wicking.set(&mut brush, -1.0);
    assert_eq!(brush.hair.wicking, 0.0);
}

#[test]
fn each_control_belongs_to_its_group() {
    assert!(Control::head().all(|control| control.part() == Part::Head));
    assert!(
        Control::hair()
            .chain(Control::hair_advanced())
            .all(|control| control.part() == Part::Hair)
    );
}

#[test]
fn values_read_with_their_steps_decimals() {
    assert_eq!(Control::Fan.format(50.0), "50°");
    assert_eq!(Control::Thickness.format(0.25), "0.25");
    assert_eq!(Control::Density.format(1.6), "1.6");
    assert_eq!(Control::Damping.format(0.05), "0.050");
}

#[test]
fn custom_comes_after_the_presets() {
    assert_eq!(position_of(&HeadPreset::ALL, None), 9);
    assert_eq!(position_of(&HeadPreset::ALL, Some(HeadPreset::Flat)), 1);
    assert_eq!(preset_at(&HeadPreset::ALL, 1), Some(HeadPreset::Flat));
    assert_eq!(preset_at(&HeadPreset::ALL, 9), None);
}

#[test]
fn a_value_lands_on_its_slider_step() {
    let mut brush = default_brush();
    Control::Thickness.set(&mut brush, 0.4372);
    assert_eq!(brush.head.thickness, 0.44);
    Control::Length.set(&mut brush, 2.53);
    assert_eq!(brush.head.length, 2.55);
    Control::Fan.set(&mut brush, 49.6);
    assert_eq!(brush.head.fan_deg, 50.0);
}

#[test]
fn every_preset_value_is_already_on_its_step() {
    for preset in HeadPreset::ALL {
        let mut brush = default_brush();
        brush.head = preset.spec();
        for control in Control::head() {
            let value = control.get(&brush);
            control.set(&mut brush, value);
            assert_eq!(control.get(&brush), value, "{preset:?} {control:?}");
        }
    }
    for preset in HairPreset::ALL {
        let mut brush = default_brush();
        brush.hair = preset.spec();
        for control in Control::hair().chain(Control::hair_advanced()) {
            let value = control.get(&brush);
            control.set(&mut brush, value);
            assert_eq!(control.get(&brush), value, "{preset:?} {control:?}");
        }
    }
}

#[test]
fn every_step_is_a_whole_fraction_of_one() {
    for control in every_control() {
        let per_unit = control.steps_per_unit();
        assert!(per_unit >= 1.0, "{control:?}");
        assert!(
            (per_unit * f64::from(control.step()) - 1.0).abs() < 1e-6,
            "{control:?}"
        );
    }
}
