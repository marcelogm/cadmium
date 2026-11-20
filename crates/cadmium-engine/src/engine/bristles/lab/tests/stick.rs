use super::friction::{DRAG_STEPS, SETTLE_STEPS, STATIC_MU, TIP, sideways, slanted, tip_ratio};
use super::*;
use crate::mechanics::{ContactRow, contact_compliance, linear_solution};

/// `TANGENT_RIDGE_SHARE` of `bristle_contacts.wgsl`. Its contact ridge, 1e-5 of the largest
/// normal compliance, is far below the tolerance and left out.
const TANGENT_RIDGE_SHARE: f64 = 1e-3;
/// G is the first-order compliance: the load's own softening of the strand, which it leaves
/// out, grows with the normal force, 5.5% under the 1 px press of the friction tests and
/// 0.7% under this one (measured).
const LIGHT_PRESS_PX: f32 = 0.1;
const LIGHT_PRESS_STEPS: u32 = 10;
const STUCK_DRAGS_PX: [f32; 4] = [0.01, 0.02, 0.03, 0.04];
const ORACLE_TOLERANCE: f64 = 0.02;
const FLOOR_NORMAL: [f64; 3] = [0.0, 0.0, 1.0];

fn metres(px: [f32; 3]) -> [f64; 3] {
    px.map(|axis| metres_from_px(f64::from(axis)))
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn unit(v: [f64; 3]) -> [f64; 3] {
    let size = dot(v, v).sqrt();
    v.map(|axis| axis / size)
}

fn drag_direction() -> [f64; 3] {
    let [x, y] = sideways(1.0).map(f64::from);
    unit([x, y, 0.0])
}

/// The rows the solve holds `run`'s contacts with on a flat floor: every pressed vertex's
/// normal and, for a stuck one, the tangent toward the root (unless the vertex above is
/// stuck too) and n × that.
fn held_rows(run: &LabRun) -> Vec<ContactRow> {
    let stuck = |v: usize| run.anchor_modes[v] > 0.0;
    let mut rows = Vec::new();
    for v in (1..BRISTLE_VERTICES).filter(|&v| run.vertex_forces_n[v] > 0.0) {
        rows.push((v, FLOOR_NORMAL));
        if !stuck(v) {
            continue;
        }
        let toward_root = toward_root(&run.chain_px, v);
        if !stuck(v - 1) {
            rows.push((v, toward_root));
        }
        rows.push((v, [-toward_root[1], toward_root[0], 0.0]));
    }
    rows
}

/// The direction from vertex `v` toward the one above it, on the flat floor.
fn toward_root(chain_px: &[[f32; 3]], v: usize) -> [f64; 3] {
    let [x, y] = [0, 1].map(|axis| f64::from(chain_px[v - 1][axis] - chain_px[v][axis]));
    unit([x, y, 0.0])
}

/// The friction along the drag that G f = the gaps adds to the tip once the root has moved
/// `drag_px` from where it held: the drag opens the tangent gaps, not the normal ones.
fn friction_from_the_matrix(run: &LabRun, drag_px: f32) -> f64 {
    let chain_m: Vec<[f64; 3]> = run.chain_px.iter().copied().map(metres).collect();
    let compliances = uniform_compliances().map(f64::from);
    let rows = held_rows(run);
    let mut matrix = contact_compliance(&chain_m, &compliances, &rows);
    for (i, &(vertex, direction)) in rows.iter().enumerate() {
        if direction != FLOOR_NORMAL {
            let normal = contact_compliance(&chain_m, &compliances, &[(vertex, FLOOR_NORMAL)]);
            matrix[i][i] += TANGENT_RIDGE_SHARE * normal[0][0];
        }
    }
    let drag_m = metres_from_px(f64::from(drag_px));
    let gaps: Vec<f64> = rows
        .iter()
        .map(|&(_, d)| -drag_m * dot(d, drag_direction()))
        .collect();
    rows.iter()
        .zip(linear_solution(&matrix, &gaps))
        .filter(|&(&(vertex, direction), _)| vertex == TIP && direction != FLOOR_NORMAL)
        .map(|(&(_, direction), force)| force * dot(direction, drag_direction()))
        .sum()
}

fn tip_friction_along_the_drag(run: &LabRun) -> f64 {
    dot(run.friction_n[TIP].map(f64::from), drag_direction())
}

/// Only the friction along the drag is first order in it: across it the strand is
/// inextensible, and the tip's pull that way changes with the square of the drag.
#[test]
fn a_stuck_tip_holds_the_friction_the_contact_matrix_gives_for_its_drag() {
    let gpu = GpuContext::new().unwrap();
    let held = |px: f32| {
        slanted(LIGHT_PRESS_PX, LIGHT_PRESS_STEPS)
            .with_drag(sideways(px), DRAG_STEPS)
            .with_settle(0.0, SETTLE_STEPS)
    };
    let strands: Vec<LabStrand> = [0.0]
        .iter()
        .chain(&STUCK_DRAGS_PX)
        .map(|&px| held(px))
        .collect();
    let runs = run(&gpu, &strands);
    let before = tip_friction_along_the_drag(&runs[0]);
    for (&px, run) in STUCK_DRAGS_PX.iter().zip(&runs[1..]) {
        assert!(
            run.anchor_modes[TIP] > 0.0 && tip_ratio(run) < STATIC_MU,
            "the tip slid at {px} px"
        );
        let held = tip_friction_along_the_drag(run) - before;
        let expected = friction_from_the_matrix(run, px);
        assert!(
            (held - expected).abs() <= ORACLE_TOLERANCE * expected.abs(),
            "at {px} px the tip holds {held:e} N along the drag, the contact matrix gives {expected:e}"
        );
    }
}
