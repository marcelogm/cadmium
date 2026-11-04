use std::collections::{HashSet, VecDeque};

use crate::canvas::{Canvas, TileCoord};
use crate::gpu::{GpuContext, new_encoder};
use crate::surface::Surface;

mod atlas;
mod slots;
mod tiles;

pub use atlas::bytes_per_tile_all_textures;

use atlas::{Atlas, slots_for_budget};
use tiles::{forget_wet, mark_restored, mark_wet, tile_extent};

pub const ATLAS_TILES_PER_SIDE: u32 = 48;
pub const ATLAS_SLOTS: u32 = ATLAS_TILES_PER_SIDE * ATLAS_TILES_PER_SIDE;
pub const MIN_SLOTS_AFTER_OUT_OF_MEMORY: u32 = 64;

type Slot = u32;

struct SavedTile {
    tile: TileCoord,
    before: Slot,
    after: Option<Slot>,
}

struct Step {
    tiles: Vec<SavedTile>,
    saved: HashSet<TileCoord>,
    complete: bool,
    removes_wet_paint: bool,
    surface: Surface,
}

impl Step {
    fn new(removes_wet_paint: bool, surface: Surface) -> Step {
        Step {
            tiles: Vec::new(),
            saved: HashSet::new(),
            complete: true,
            removes_wet_paint,
            surface,
        }
    }

    fn slots(&self) -> impl Iterator<Item = Slot> + '_ {
        self.tiles
            .iter()
            .flat_map(|saved| std::iter::once(saved.before).chain(saved.after))
    }

    fn unsaved(&self, canvas: &Canvas, tiles: &[TileCoord]) -> Vec<TileCoord> {
        let mut seen = HashSet::new();
        tiles
            .iter()
            .copied()
            .filter(|tile| {
                !self.saved.contains(tile)
                    && tile_extent(canvas, *tile).is_some()
                    && seen.insert(*tile)
            })
            .collect()
    }
}

pub struct History {
    wanted_slots: u32,
    slot_capacity: u32,
    max_steps: usize,
    atlas: Option<Atlas>,
    free_slots: Vec<Slot>,
    undo_steps: VecDeque<Step>,
    redo_steps: Vec<Step>,
    current: Option<Step>,
}

impl History {
    pub fn new(gpu: &GpuContext, budget_bytes: u64, max_steps: usize) -> Self {
        let mut history = History {
            wanted_slots: 0,
            slot_capacity: 0,
            max_steps,
            atlas: None,
            free_slots: Vec::new(),
            undo_steps: VecDeque::new(),
            redo_steps: Vec::new(),
            current: None,
        };
        history.allocate_atlas(gpu, slots_for_budget(budget_bytes));
        history
    }

    pub fn refit(&mut self, gpu: &GpuContext, budget_bytes: u64) {
        self.clear();
        let wanted_slots = slots_for_budget(budget_bytes);
        if wanted_slots != self.wanted_slots {
            self.atlas = None;
            gpu.queue.submit([]);
            let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
            self.allocate_atlas(gpu, wanted_slots);
        }
    }

    fn allocate_atlas(&mut self, gpu: &GpuContext, wanted_slots: u32) {
        let (atlas, slot_capacity) = match Atlas::allocate_largest(gpu, wanted_slots) {
            Some((atlas, slot_capacity)) => (Some(atlas), slot_capacity),
            None => (None, 0),
        };
        self.wanted_slots = wanted_slots;
        self.slot_capacity = slot_capacity;
        self.atlas = atlas;
        self.free_slots = (0..slot_capacity).rev().collect();
    }

    pub fn slot_capacity(&self) -> u32 {
        self.slot_capacity
    }

    pub fn is_enabled(&self) -> bool {
        self.slot_capacity > 0
    }

    pub fn begin_step(&mut self, surface: Surface) {
        self.open_step(false, surface);
    }

    pub fn begin_removal_step(&mut self, surface: Surface, wet_tile_count: usize) {
        self.open_step(true, surface);
        if wet_tile_count > self.slot_capacity as usize
            && let Some(mut step) = self.current.take()
        {
            self.abandon(&mut step);
            self.current = Some(step);
        }
    }

    fn open_step(&mut self, removes_wet_paint: bool, surface: Surface) {
        self.commit_step();
        self.current = Some(Step::new(removes_wet_paint, surface));
    }

    pub fn encode_save_tiles(
        &mut self,
        _gpu: &GpuContext,
        canvas: &Canvas,
        tiles: &[TileCoord],
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let Some(mut step) = self.current.take() else {
            return;
        };
        let unsaved = step.unsaved(canvas, tiles);
        if unsaved.is_empty() || !step.complete {
            self.current = Some(step);
            return;
        }
        self.drop_redo_steps();
        if step.tiles.len() + unsaved.len() > self.slot_capacity as usize {
            self.abandon(&mut step);
            self.current = Some(step);
            return;
        }
        let saved_every_tile = unsaved
            .iter()
            .all(|tile| self.save_into_step(canvas, &mut step, *tile, encoder));
        if !saved_every_tile {
            self.abandon(&mut step);
        }
        self.current = Some(step);
    }

    fn save_into_step(
        &mut self,
        canvas: &Canvas,
        step: &mut Step,
        tile: TileCoord,
        encoder: &mut wgpu::CommandEncoder,
    ) -> bool {
        let Some(slot) = self.allocate() else {
            return false;
        };
        self.copy_canvas_to_slot(canvas, tile, slot, encoder);
        step.saved.insert(tile);
        step.tiles.push(SavedTile {
            tile,
            before: slot,
            after: None,
        });
        true
    }

    pub fn commit_step(&mut self) -> bool {
        let Some(step) = self.current.take() else {
            return true;
        };
        let complete = step.complete;
        if !complete || step.tiles.is_empty() {
            self.release(step);
            return complete;
        }
        self.undo_steps.push_back(step);
        self.trim_to_max_steps();
        true
    }

    pub fn undo(&mut self, gpu: &GpuContext, canvas: &mut Canvas) -> bool {
        self.commit_step();
        let Some(mut step) = self.undo_steps.pop_back() else {
            return false;
        };
        let mut redoable = self.reserve_after_slots(&mut step);
        if !redoable {
            self.drop_redo_steps();
            redoable = self.reserve_after_slots(&mut step);
        }
        let mut encoder = new_encoder(gpu, "undo");
        for saved in &step.tiles {
            self.undo_tile(canvas, saved, &mut encoder);
        }
        gpu.queue.submit([encoder.finish()]);
        if redoable {
            self.redo_steps.push(step);
        } else {
            self.release(step);
        }
        true
    }

    pub fn redo(&mut self, gpu: &GpuContext, canvas: &mut Canvas) -> bool {
        let Some(step) = self.redo_steps.pop() else {
            return false;
        };
        let mut encoder = new_encoder(gpu, "redo");
        for saved in &step.tiles {
            self.redo_tile(canvas, saved, step.removes_wet_paint, &mut encoder);
        }
        gpu.queue.submit([encoder.finish()]);
        if step.removes_wet_paint {
            forget_wet(canvas, &step.saved);
        }
        self.undo_steps.push_back(step);
        self.trim_to_max_steps();
        true
    }

    fn undo_tile(
        &self,
        canvas: &mut Canvas,
        saved: &SavedTile,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if let Some(after) = saved.after {
            self.copy_canvas_to_slot(canvas, saved.tile, after, encoder);
        }
        self.copy_slot_to_canvas(canvas, saved.tile, saved.before, encoder);
        mark_restored(canvas, saved.tile);
        mark_wet(canvas, saved.tile);
    }

    fn redo_tile(
        &self,
        canvas: &mut Canvas,
        saved: &SavedTile,
        removes_wet_paint: bool,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let Some(after) = saved.after else {
            return;
        };
        self.copy_slot_to_canvas(canvas, saved.tile, after, encoder);
        mark_restored(canvas, saved.tile);
        if !removes_wet_paint {
            mark_wet(canvas, saved.tile);
        }
    }

    pub fn undo_surface(&self) -> Option<Surface> {
        self.current
            .as_ref()
            .filter(|step| step.complete && !step.tiles.is_empty())
            .or(self.undo_steps.back())
            .map(|step| step.surface)
    }

    pub fn redo_surface(&self) -> Option<Surface> {
        self.redo_steps.last().map(|step| step.surface)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_steps.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_steps.is_empty()
    }
}
