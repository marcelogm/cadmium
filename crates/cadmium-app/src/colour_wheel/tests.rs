use super::geometry::turn_and_radius_at;
use super::*;

const SIDE_PX: i32 = 192;
const PAINT: [u8; 3] = [200, 100, 50];
const COVERED_TOLERANCE_LEVELS: u8 = 2;
/// A pixel the edge of the disc crosses keeps a trace of the disc under the cells.
const EDGE_TOLERANCE_LEVELS: u8 = 8;
const DOT: [u8; 3] = [10, 20, 30];

fn disc_of(colour: [u8; 3]) -> Vec<Option<[u8; 3]>> {
    (0..WHEEL_CELLS * WHEEL_CELLS)
        .map(|index| {
            let (x, y) = ((index % WHEEL_CELLS) as f64, (index / WHEEL_CELLS) as f64);
            turn_and_radius_at(x + 0.5, y + 0.5, WHEEL_CELLS as f64).map(|_| colour)
        })
        .collect()
}

fn drawn(shown: &Shown) -> impl Fn(i32, i32) -> u32 + use<> {
    drawn_as(shown, false)
}

fn drawn_as(shown: &Shown, editable: bool) -> impl Fn(i32, i32) -> u32 + use<> {
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, SIDE_PX, SIDE_PX)
        .expect("a cairo surface");
    {
        let context = cairo::Context::new(&surface).expect("a cairo context");
        draw(shown, &context, f64::from(SIDE_PX), editable);
    }
    let stride = surface.stride();
    let bytes = surface.data().expect("the surface's pixels").to_vec();
    move |x, y| {
        if !(0..SIDE_PX).contains(&x) || !(0..SIDE_PX).contains(&y) {
            return 0;
        }
        let at = (y * stride + x * 4) as usize;
        u32::from_ne_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
    }
}

#[test]
fn the_wheel_paints_its_cells_in_a_disc_and_a_dot_for_each_rim_colour() {
    let pixel = drawn(&Shown::of(WheelImage {
        cells: disc_of(PAINT),
        rim: vec![DOT],
    }));
    assert_eq!(pixel(SIDE_PX / 2, SIDE_PX / 2), 0xFFC8_6432);
    assert_eq!(pixel(SIDE_PX / 4, SIDE_PX / 2), 0xFFC8_6432);
    assert_eq!(pixel(0, 0), 0);
    assert_eq!(pixel(SIDE_PX - 1, SIDE_PX - 1), 0);
    let (x, y) = rim_centre(0, 1, f64::from(SIDE_PX));
    assert_eq!(pixel(x as i32, y as i32), 0xFF0A_141E);
}

#[test]
fn a_wheel_with_no_cell_is_a_neutral_disc() {
    let pixel = drawn(&Shown::of(WheelImage {
        cells: vec![None; WHEEL_CELLS * WHEEL_CELLS],
        rim: vec![],
    }));
    assert_eq!(pixel(SIDE_PX / 2, SIDE_PX / 2), 0xFFE1_E1E1);
    assert_eq!(pixel(SIDE_PX / 2, 2), 0xFFE1_E1E1);
    assert_eq!(pixel(0, 0), 0);
}

#[test]
fn the_disc_is_opaque_up_to_its_edge_and_nothing_is_painted_past_it() {
    let pixel = drawn(&Shown::of(WheelImage {
        cells: disc_of(PAINT),
        rim: vec![],
    }));
    let half = f64::from(SIDE_PX) / 2.0;
    let alpha_at = |angle: f64, reach: f64| {
        let (x, y) = (half + reach * angle.sin(), half - reach * angle.cos());
        pixel(x.floor() as i32, y.floor() as i32) >> 24
    };
    for step in 0..360 {
        let angle = f64::from(step).to_radians();
        assert_eq!(
            alpha_at(angle, half - 1.5),
            0xFF,
            "inside at {step} degrees"
        );
        assert_eq!(alpha_at(angle, half + 1.5), 0, "outside at {step} degrees");
    }
}

#[test]
fn neighbouring_cells_blend_into_each_other() {
    let cells = disc_of(PAINT)
        .iter()
        .enumerate()
        .map(|(index, cell)| {
            cell.map(|_| [200 * u8::from(index % WHEEL_CELLS >= WHEEL_CELLS / 2); 3])
        })
        .collect();
    let pixel = drawn(&Shown::of(WheelImage { cells, rim: vec![] }));
    let red_at = |x: i32| pixel(x, SIDE_PX / 2) >> 16 & 0xFF;
    assert_eq!(red_at(SIDE_PX / 2 - 4), 0);
    assert!(red_at(SIDE_PX / 2 - 1) > 0 && red_at(SIDE_PX / 2 - 1) < red_at(SIDE_PX / 2));
    assert!(red_at(SIDE_PX / 2) < 200);
    assert_eq!(red_at(SIDE_PX / 2 + 4), 200);
}

fn straight_colour(pixel: u32) -> [u8; 3] {
    let alpha = (pixel >> 24).max(1);
    [16, 8, 0].map(|shift| ((pixel >> shift & 0xFF) * 0xFF / alpha).min(0xFF) as u8)
}

fn furthest_from_the_paint(pixel: u32) -> u8 {
    let colour = straight_colour(pixel);
    (0..3)
        .map(|channel| colour[channel].abs_diff(PAINT[channel]))
        .max()
        .unwrap_or(0)
}

#[test]
fn the_rim_is_the_colour_of_its_cells_and_not_a_blend_with_the_disc_under_them() {
    let pixel = drawn(&Shown::of(WheelImage {
        cells: disc_of(PAINT),
        rim: vec![],
    }));
    let half = f64::from(SIDE_PX) / 2.0;
    for step in 0..360 {
        let angle = f64::from(step).to_radians();
        let (x, y) = (
            half + (half - 1.5) * angle.sin(),
            half - (half - 1.5) * angle.cos(),
        );
        let covered = pixel(x.floor() as i32, y.floor() as i32);
        assert!(
            furthest_from_the_paint(covered) <= COVERED_TOLERANCE_LEVELS,
            "{covered:#x} at {step} degrees"
        );
    }
    let on_the_edge = [
        (163, 28),
        (SIDE_PX / 2, 0),
        (SIDE_PX - 1, SIDE_PX / 2),
        (0, SIDE_PX / 2),
    ];
    for (x, y) in on_the_edge {
        let edge = pixel(x, y);
        assert!(
            furthest_from_the_paint(edge) <= EDGE_TOLERANCE_LEVELS,
            "{edge:#x} at ({x}, {y})"
        );
    }
}

#[test]
fn an_editable_wheel_draws_its_rim_dots_bigger() {
    let shown = Shown::of(WheelImage {
        cells: vec![None; WHEEL_CELLS * WHEEL_CELLS],
        rim: vec![DOT],
    });
    let (x, y) = rim_centre(0, 1, f64::from(SIDE_PX));
    let beside_the_dot = (x as i32 + 6, y as i32);
    let plain = drawn_as(&shown, false);
    let editable = drawn_as(&shown, true);
    assert_ne!(plain(beside_the_dot.0, beside_the_dot.1), 0xFF0A_141E);
    assert_eq!(editable(beside_the_dot.0, beside_the_dot.1), 0xFF0A_141E);
}
