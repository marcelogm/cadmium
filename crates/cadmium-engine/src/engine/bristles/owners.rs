use super::roots::Root;
use crate::engine::{TIP_HALF, TIP_SIZE};

pub(crate) const OUTSIDE_TIP: u32 = u32::MAX;
pub(crate) const RANKS_PER_TEXEL: usize = 1;
const MIN_GRID_CELL_PX: f32 = 2.0;
const CLAIM_OFFSETS: [[f32; 2]; 9] = [
    [0.0, 0.0],
    [-1.0, 0.0],
    [1.0, 0.0],
    [0.0, -1.0],
    [0.0, 1.0],
    [-1.0, -1.0],
    [1.0, -1.0],
    [-1.0, 1.0],
    [1.0, 1.0],
];
const LANE_STEP_PX: f32 = 0.25;
const LANE_SAMPLES: usize = 8;
const LANE_SHRINK: f32 = 0.9;
const LANE_MIN_LENGTH_PX: f32 = 0.01;

pub(super) struct OwnerMap {
    pub(super) owners: Vec<u32>,
    pub(super) lanes: Vec<([f32; 2], [f32; 2])>,
}

pub(super) fn build_owner_map(tip_roots: &[[f32; 2]], roots: &[Root], radius_px: f32) -> OwnerMap {
    let (owners, anchors) = assign_owners(tip_roots, radius_px);
    let lanes = roots
        .iter()
        .enumerate()
        .map(|(index, root)| lane_of(&owners, index as u32, anchors[index], root.lane_direction))
        .collect();
    OwnerMap { owners, lanes }
}

pub(super) fn owner_ranks(owners: &[u32]) -> Vec<u32> {
    let mut cell_counts = vec![0u32; owners.len()];
    let mut ranks = vec![0u32; RANKS_PER_TEXEL * owners.len()];
    for (slot, &owner) in owners.iter().enumerate() {
        if owner == OUTSIDE_TIP {
            continue;
        }
        ranks[RANKS_PER_TEXEL * slot] = cell_counts[owner as usize];
        cell_counts[owner as usize] += 1;
    }
    ranks
}

fn assign_owners(tip_roots: &[[f32; 2]], radius_px: f32) -> (Vec<u32>, Vec<[f32; 2]>) {
    let size = TIP_SIZE as usize;
    let half = TIP_HALF as i32;
    let radius_squared = radius_px * radius_px;
    let mut owners = vec![OUTSIDE_TIP; size * size];
    let grid = RootGrid::new(tip_roots, radius_px);
    for (slot, owner) in owners.iter_mut().enumerate() {
        let (x, y) = (slot % size, slot / size);
        let (offset_x, offset_y) = ((x as i32 - half) as f32, (y as i32 - half) as f32);
        if offset_x * offset_x + offset_y * offset_y <= radius_squared {
            *owner = grid.nearest([x as f32 + 0.5, y as f32 + 0.5]).0 as u32;
        }
    }
    claim_unowned_roots(&mut owners, tip_roots);
    let anchors = anchor_centres(&owners, tip_roots);
    (owners, anchors)
}

fn anchor_centres(owners: &[u32], tip_roots: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let size = TIP_SIZE as usize;
    let mut closest = vec![f32::INFINITY; tip_roots.len()];
    let mut centres = tip_roots.to_vec();
    for (slot, &owner) in owners
        .iter()
        .enumerate()
        .filter(|&(_, &owner)| owner != OUTSIDE_TIP)
    {
        let centre = [(slot % size) as f32 + 0.5, (slot / size) as f32 + 0.5];
        let root = tip_roots[owner as usize];
        let distance = (root[0] - centre[0]).powi(2) + (root[1] - centre[1]).powi(2);
        if distance < closest[owner as usize] {
            closest[owner as usize] = distance;
            centres[owner as usize] = centre;
        }
    }
    centres
}

struct RootGrid<'a> {
    roots: &'a [[f32; 2]],
    cell_px: f32,
    columns: usize,
    cells: Vec<Vec<usize>>,
}

impl<'a> RootGrid<'a> {
    fn new(roots: &'a [[f32; 2]], radius_px: f32) -> RootGrid<'a> {
        let cell_px = (2.0 * radius_px / (roots.len() as f32).sqrt()).max(MIN_GRID_CELL_PX);
        let columns = (TIP_SIZE as f32 / cell_px).ceil() as usize + 1;
        let mut grid = RootGrid {
            roots,
            cell_px,
            columns,
            cells: vec![Vec::new(); columns * columns],
        };
        for (index, root) in roots.iter().enumerate() {
            let [column, row] = grid.cell_of(*root);
            grid.cells[row * columns + column].push(index);
        }
        grid
    }

    fn cell_of(&self, point: [f32; 2]) -> [usize; 2] {
        let last = self.columns - 1;
        let clamp = |value: f32| ((value / self.cell_px).max(0.0) as usize).min(last);
        [clamp(point[0]), clamp(point[1])]
    }

    fn nearest(&self, point: [f32; 2]) -> (usize, f32) {
        let [column, row] = self.cell_of(point);
        let mut best = (0, f32::INFINITY);
        let mut ring = 0;
        while ring < self.columns && best.1 > self.ring_reach_squared(ring) {
            self.search_ring([column, row], ring, point, &mut best);
            ring += 1;
        }
        best
    }

    fn ring_reach_squared(&self, ring: usize) -> f32 {
        let reach_px = (ring as f32 - 1.0) * self.cell_px;
        reach_px.max(0.0).powi(2)
    }

    fn search_ring(
        &self,
        centre: [usize; 2],
        ring: usize,
        point: [f32; 2],
        best: &mut (usize, f32),
    ) {
        for delta in ring_cells(ring as isize) {
            self.search_cell(centre, delta, point, best);
        }
    }

    fn search_cell(
        &self,
        centre: [usize; 2],
        delta: [isize; 2],
        point: [f32; 2],
        best: &mut (usize, f32),
    ) {
        let column = centre[0] as isize + delta[0];
        let row = centre[1] as isize + delta[1];
        let limit = self.columns as isize;
        if column < 0 || row < 0 || column >= limit || row >= limit {
            return;
        }
        *best = self.cells[row as usize * self.columns + column as usize]
            .iter()
            .fold(*best, |nearest, &index| {
                closer(nearest, (index, squared_distance(self.roots[index], point)))
            });
    }
}

fn ring_cells(ring: isize) -> impl Iterator<Item = [isize; 2]> {
    (-ring..=ring).flat_map(move |delta_row| {
        (-ring..=ring)
            .step_by(ring_column_step(delta_row, ring))
            .map(move |delta_column| [delta_column, delta_row])
    })
}

fn ring_column_step(delta_row: isize, ring: isize) -> usize {
    if delta_row.abs() == ring {
        1
    } else {
        2 * ring.max(1) as usize
    }
}

fn squared_distance(from: [f32; 2], to: [f32; 2]) -> f32 {
    (from[0] - to[0]).powi(2) + (from[1] - to[1]).powi(2)
}

fn closer(nearest: (usize, f32), candidate: (usize, f32)) -> (usize, f32) {
    if candidate.1 < nearest.1 {
        candidate
    } else {
        nearest
    }
}

fn owner_at(owners: &[u32], point: [f32; 2]) -> u32 {
    let size = TIP_SIZE as f32;
    if point[0] < 0.0 || point[1] < 0.0 || point[0] >= size || point[1] >= size {
        return OUTSIDE_TIP;
    }
    owners[point[1] as usize * TIP_SIZE as usize + point[0] as usize]
}

fn lane_of(
    owners: &[u32],
    index: u32,
    start: [f32; 2],
    direction: [f32; 2],
) -> ([f32; 2], [f32; 2]) {
    let point_at = |distance_px: f32| {
        [
            start[0] + direction[0] * distance_px,
            start[1] + direction[1] * distance_px,
        ]
    };
    let mut reach_px = 0.0;
    while owner_at(owners, point_at(reach_px + LANE_STEP_PX)) == index {
        reach_px += LANE_STEP_PX;
    }
    let mut end = point_at(reach_px);
    while !lane_is_inside(owners, index, start, end) {
        end = [
            start[0] + (end[0] - start[0]) * LANE_SHRINK,
            start[1] + (end[1] - start[1]) * LANE_SHRINK,
        ];
        if (end[0] - start[0]).hypot(end[1] - start[1]) < LANE_MIN_LENGTH_PX {
            return (start, start);
        }
    }
    (start, end)
}

fn lane_is_inside(owners: &[u32], index: u32, start: [f32; 2], end: [f32; 2]) -> bool {
    (0..LANE_SAMPLES).all(|sample| {
        let along = sample as f32 / (LANE_SAMPLES - 1) as f32;
        let point = [
            start[0] + along * (end[0] - start[0]),
            start[1] + along * (end[1] - start[1]),
        ];
        owner_at(owners, point) == index
    })
}

fn claim_unowned_roots(owners: &mut [u32], tip_roots: &[[f32; 2]]) {
    let mut texels = vec![0usize; tip_roots.len()];
    for &owner in owners.iter().filter(|&&owner| owner != OUTSIDE_TIP) {
        texels[owner as usize] += 1;
    }
    let unowned: Vec<usize> = (0..tip_roots.len())
        .filter(|&index| texels[index] == 0)
        .collect();
    for index in unowned {
        let root = tip_roots[index];
        let claimed = CLAIM_OFFSETS.iter().find_map(|offset| {
            let point = [root[0].floor() + offset[0], root[1].floor() + offset[1]];
            let slot = owner_slot(point)?;
            let previous = owners[slot];
            let stealable = previous != OUTSIDE_TIP && texels[previous as usize] > 1;
            stealable.then_some((slot, previous as usize))
        });
        if let Some((slot, previous)) = claimed {
            owners[slot] = index as u32;
            texels[previous] -= 1;
            texels[index] = 1;
        }
    }
}

fn owner_slot(point: [f32; 2]) -> Option<usize> {
    let size = TIP_SIZE as f32;
    let inside = (0.0..size).contains(&point[0]) && (0.0..size).contains(&point[1]);
    inside.then(|| point[1] as usize * TIP_SIZE as usize + point[0] as usize)
}
