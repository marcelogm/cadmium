use super::owners::OUTSIDE_TIP;
use crate::engine::TIP_SIZE;

pub(crate) const CELL_GRID_SIDE: usize = 16;
pub(crate) const CELL_GRID_BINS: usize = CELL_GRID_SIDE * CELL_GRID_SIDE;

pub(super) struct CellFrame {
    pub(super) direction: [f32; 2],
    pub(super) fallback_slot: u32,
}

struct CellPoint {
    slot: u32,
    along: f32,
    across: f32,
}

pub(super) fn cell_grids(owners: &[u32], frames: &[CellFrame]) -> Vec<u32> {
    let mut cells: Vec<Vec<CellPoint>> = (0..frames.len()).map(|_| Vec::new()).collect();
    for (slot, &owner) in owners.iter().enumerate() {
        let Some(frame) = frames.get(owner as usize).filter(|_| owner != OUTSIDE_TIP) else {
            continue;
        };
        cells[owner as usize].push(cell_point(slot, frame.direction));
    }
    cells
        .iter()
        .zip(frames)
        .flat_map(|(cell, frame)| grid_of(cell, frame.fallback_slot))
        .collect()
}

fn cell_point(slot: usize, direction: [f32; 2]) -> CellPoint {
    let size = TIP_SIZE as usize;
    let centre = [(slot % size) as f32 + 0.5, (slot / size) as f32 + 0.5];
    CellPoint {
        slot: slot as u32,
        along: centre[0] * direction[0] + centre[1] * direction[1],
        across: centre[1] * direction[0] - centre[0] * direction[1],
    }
}

fn grid_of(cell: &[CellPoint], fallback_slot: u32) -> Vec<u32> {
    if cell.is_empty() {
        return vec![fallback_slot; CELL_GRID_BINS];
    }
    let span = |value: fn(&CellPoint) -> f32| {
        cell.iter()
            .map(value)
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(low, high), v| {
                (low.min(v), high.max(v))
            })
    };
    let along = span(|point| point.along);
    let across = span(|point| point.across);
    (0..CELL_GRID_BINS)
        .map(|bin| {
            let target = [
                bin_centre(along, bin / CELL_GRID_SIDE),
                bin_centre(across, bin % CELL_GRID_SIDE),
            ];
            nearest_slot(cell, target)
        })
        .collect()
}

fn bin_centre((low, high): (f32, f32), bin: usize) -> f32 {
    low + (high - low) * (bin as f32 + 0.5) / CELL_GRID_SIDE as f32
}

fn nearest_slot(cell: &[CellPoint], target: [f32; 2]) -> u32 {
    let distance =
        |point: &CellPoint| (point.along - target[0]).powi(2) + (point.across - target[1]).powi(2);
    cell.iter()
        .min_by(|a, b| distance(a).total_cmp(&distance(b)))
        .map_or(OUTSIDE_TIP, |point| point.slot)
}

#[cfg(test)]
mod tests;
