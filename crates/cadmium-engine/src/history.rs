use std::collections::{HashSet, VecDeque};

use crate::canvas::{Canvas, TileCoord};
use crate::gpu::GpuContext;
use crate::surface::Surface;

mod atlas;
mod slots;
mod tiles;

pub use atlas::bytes_per_tile_all_textures;

use atlas::{Atlas, CopyDirection, slots_for_budget};
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

    #[expect(clippy::excessive_nesting)]
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
        let mut unsaved = Vec::new();
        let mut seen = HashSet::new();
        for tile in tiles {
            if !step.saved.contains(tile)
                && tile_extent(canvas, *tile).is_some()
                && seen.insert(*tile)
            {
                unsaved.push(*tile);
            }
        }
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
        for tile in &unsaved {
            let Some(slot) = self.allocate() else {
                self.abandon(&mut step);
                break;
            };
            self.copy_tile(canvas, *tile, slot, CopyDirection::CanvasToSlot, encoder);
            step.saved.insert(*tile);
            step.tiles.push(SavedTile {
                tile: *tile,
                before: slot,
                after: None,
            });
        }
        self.current = Some(step);
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

    #[expect(clippy::excessive_nesting, clippy::too_many_lines)]
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
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("undo"),
            });
        for saved in &step.tiles {
            if let Some(after) = saved.after {
                self.copy_tile(
                    canvas,
                    saved.tile,
                    after,
                    CopyDirection::CanvasToSlot,
                    &mut encoder,
                );
            }
            self.copy_tile(
                canvas,
                saved.tile,
                saved.before,
                CopyDirection::SlotToCanvas,
                &mut encoder,
            );
            mark_restored(canvas, saved.tile);
            mark_wet(canvas, saved.tile);
        }
        gpu.queue.submit([encoder.finish()]);
        if redoable {
            self.redo_steps.push(step);
        } else {
            self.release(step);
        }
        true
    }

    #[expect(clippy::excessive_nesting)]
    pub fn redo(&mut self, gpu: &GpuContext, canvas: &mut Canvas) -> bool {
        let Some(step) = self.redo_steps.pop() else {
            return false;
        };
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("redo"),
            });
        for saved in &step.tiles {
            if let Some(after) = saved.after {
                self.copy_tile(
                    canvas,
                    saved.tile,
                    after,
                    CopyDirection::SlotToCanvas,
                    &mut encoder,
                );
                mark_restored(canvas, saved.tile);
                if !step.removes_wet_paint {
                    mark_wet(canvas, saved.tile);
                }
            }
        }
        gpu.queue.submit([encoder.finish()]);
        if step.removes_wet_paint {
            forget_wet(canvas, &step.saved);
        }
        self.undo_steps.push_back(step);
        self.trim_to_max_steps();
        true
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
