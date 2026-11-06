use super::{DemoStroke, StrokeBox, samples};

const BOX: StrokeBox = StrokeBox {
    x: 10.0,
    y: 20.0,
    width: 300.0,
    height: 100.0,
};

const STROKES: [DemoStroke; 2] = [DemoStroke::SCurve, DemoStroke::RisingLine];

#[test]
fn every_demo_stroke_stays_inside_its_box() {
    for stroke in STROKES {
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
fn every_demo_stroke_crosses_its_box() {
    for stroke in STROKES {
        let xs: Vec<f64> = samples(stroke, BOX).iter().map(|s| s.x).collect();
        assert!(spread(&xs) >= BOX.width * 0.9, "{stroke:?}");
    }
}

#[test]
fn time_moves_forward_and_pressure_stays_in_range() {
    for stroke in STROKES {
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

fn spread(values: &[f64]) -> f64 {
    values.iter().copied().fold(f64::MIN, f64::max)
        - values.iter().copied().fold(f64::MAX, f64::min)
}
