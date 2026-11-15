use super::*;

const SIDE: i32 = 192;
const PAINT: [u8; 3] = [200, 100, 50];
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
    let mut surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, SIDE, SIDE).expect("a cairo surface");
    {
        let context = cairo::Context::new(&surface).expect("a cairo context");
        draw(shown, &context, f64::from(SIDE));
    }
    let stride = surface.stride();
    let bytes = surface.data().expect("the surface's pixels").to_vec();
    move |x, y| {
        if !(0..SIDE).contains(&x) || !(0..SIDE).contains(&y) {
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
    assert_eq!(pixel(SIDE / 2, SIDE / 2), 0xFFC8_6432);
    assert_eq!(pixel(SIDE / 4, SIDE / 2), 0xFFC8_6432);
    assert_eq!(pixel(0, 0), 0);
    assert_eq!(pixel(SIDE - 1, SIDE - 1), 0);
    let (x, y) = rim_centre(0, 1, f64::from(SIDE));
    assert_eq!(pixel(x as i32, y as i32), 0xFF0A_141E);
}

#[test]
fn a_wheel_with_no_cell_is_a_neutral_disc() {
    let pixel = drawn(&Shown::of(WheelImage {
        cells: vec![None; WHEEL_CELLS * WHEEL_CELLS],
        rim: vec![],
    }));
    assert_eq!(pixel(SIDE / 2, SIDE / 2), 0xFFE1_E1E1);
    assert_eq!(pixel(SIDE / 2, 2), 0xFFE1_E1E1);
    assert_eq!(pixel(0, 0), 0);
}

#[test]
fn the_disc_is_opaque_up_to_its_edge_and_nothing_is_painted_past_it() {
    let pixel = drawn(&Shown::of(WheelImage {
        cells: disc_of(PAINT),
        rim: vec![],
    }));
    let half = f64::from(SIDE) / 2.0;
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
    let red_at = |x: i32| pixel(x, SIDE / 2) >> 16 & 0xFF;
    assert_eq!(red_at(SIDE / 2 - 4), 0);
    assert!(red_at(SIDE / 2 - 1) > 0 && red_at(SIDE / 2 - 1) < red_at(SIDE / 2));
    assert!(red_at(SIDE / 2) < 200);
    assert_eq!(red_at(SIDE / 2 + 4), 200);
}
