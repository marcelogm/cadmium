use super::{DemoStroke, StrokeBox, ZIGZAG_TEETH, samples};

const BOX: StrokeBox = StrokeBox {
    x: 10.0,
    y: 20.0,
    width: 300.0,
    height: 100.0,
};

#[test]
fn every_demo_stroke_stays_inside_its_box() {
    for stroke in DemoStroke::ALL {
        for s in samples(stroke, BOX) {
            assert!(
                s.x >= BOX.x && s.x <= BOX.x + BOX.width,
                "{stroke:?} x {}",
                s.x
            );
            assert!(
                s.y >= BOX.y && s.y <= BOX.y + BOX.height,
                "{stroke:?} y {}",
                s.y
            );
        }
    }
}

#[test]
fn every_moving_stroke_crosses_its_box() {
    for stroke in DemoStroke::ALL
        .into_iter()
        .filter(|s| *s != DemoStroke::TapAndLift)
    {
        let xs: Vec<f64> = samples(stroke, BOX).iter().map(|s| s.x).collect();
        assert!(spread(&xs) >= BOX.width * 0.9, "{stroke:?}");
    }
}

#[test]
fn time_moves_forward_and_pressure_stays_in_range() {
    for stroke in DemoStroke::ALL {
        let all = samples(stroke, BOX);
        assert!(all.len() >= 2, "{stroke:?}");
        assert!(
            all.windows(2).all(|w| w[1].time_ms > w[0].time_ms),
            "{stroke:?}"
        );
        assert!(
            all.iter().all(|s| (0.0..=1.0).contains(&s.pressure)),
            "{stroke:?}"
        );
    }
}

#[test]
fn the_s_curve_presses_hardest_in_the_middle() {
    let all = samples(DemoStroke::SCurve, BOX);
    let middle = all[all.len() / 2].pressure;
    assert!(middle > all[0].pressure && middle > all[all.len() - 1].pressure);
}

#[test]
fn the_rising_line_only_presses_harder() {
    let all = samples(DemoStroke::RisingLine, BOX);
    assert!(all.windows(2).all(|w| w[1].pressure >= w[0].pressure));
    assert!(all.iter().all(|s| s.y == all[0].y));
}

#[test]
fn a_tap_barely_moves() {
    let all = samples(DemoStroke::TapAndLift, BOX);
    let xs: Vec<f64> = all.iter().map(|s| s.x).collect();
    assert!(spread(&xs) < BOX.width * 0.05);
}

#[test]
fn the_zigzag_turns_back_once_per_tooth_edge_without_jumping() {
    let ys: Vec<f64> = samples(DemoStroke::Zigzag, BOX)
        .iter()
        .map(|s| s.y)
        .collect();
    let steps: Vec<f64> = ys
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|d| *d != 0.0)
        .collect();
    let turns = steps
        .windows(2)
        .filter(|w| w[0].signum() != w[1].signum())
        .count();
    assert!(turns as f64 >= 2.0 * ZIGZAG_TEETH - 1.0, "{turns} turns");
    assert!(spread(&ys) >= BOX.height * 0.75);
    let mut sizes: Vec<f64> = steps.iter().map(|d| d.abs()).collect();
    sizes.sort_by(f64::total_cmp);
    let typical = sizes[sizes.len() / 2];
    assert!(
        sizes[sizes.len() - 1] <= typical * 2.0,
        "the pen jumps: {sizes:?}"
    );
}

#[test]
fn every_demo_stroke_is_listed_once_with_its_own_label() {
    let all = DemoStroke::ALL;
    for (i, a) in all.iter().enumerate() {
        for b in &all[i + 1..] {
            assert_ne!(a, b);
            assert_ne!(a.label(), b.label());
        }
    }
    assert_eq!(all.len(), 4);
}

fn spread(values: &[f64]) -> f64 {
    values.iter().copied().fold(f64::MIN, f64::max)
        - values.iter().copied().fold(f64::MAX, f64::min)
}
