use std::collections::{HashSet, VecDeque};

use crate::canvas::{Canvas, StateTexture, TILE_SIZE, TileCoord};
use crate::gpu::GpuContext;

pub const ATLAS_TILES_PER_SIDE: u32 = 48;
pub const ATLAS_SLOTS: u32 = ATLAS_TILES_PER_SIDE * ATLAS_TILES_PER_SIDE;

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
}

impl Step {
    fn new() -> Step {
        Step {
            tiles: Vec::new(),
            saved: HashSet::new(),
            complete: true,
        }
    }

    fn slots(&self) -> impl Iterator<Item = Slot> + '_ {
        self.tiles
            .iter()
            .flat_map(|saved| std::iter::once(saved.before).chain(saved.after))
    }
}

struct Atlas {
    textures: Vec<wgpu::Texture>,
}

impl Atlas {
    fn new(gpu: &GpuContext, slot_capacity: u32) -> Atlas {
        let columns = slot_capacity.min(ATLAS_TILES_PER_SIDE);
        let rows = slot_capacity.div_ceil(ATLAS_TILES_PER_SIDE);
        let textures = StateTexture::PAINT_STATE
            .iter()
            .map(|which| {
                gpu.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("undo_atlas"),
                    size: wgpu::Extent3d {
                        width: columns * TILE_SIZE,
                        height: rows * TILE_SIZE,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: which.format(),
                    usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                })
            })
            .collect();
        Atlas { textures }
    }
}

#[derive(Clone, Copy)]
enum CopyDirection {
    CanvasToSlot,
    SlotToCanvas,
}

pub fn bytes_per_tile_all_textures() -> u64 {
    StateTexture::PAINT_STATE
        .iter()
        .map(|which| u64::from(TILE_SIZE * TILE_SIZE * which.bytes_per_texel()))
        .sum()
}

pub struct History {
    slot_capacity: u32,
    max_steps: usize,
    atlas: Option<Atlas>,
    free_slots: Vec<Slot>,
    undo_steps: VecDeque<Step>,
    redo_steps: Vec<Step>,
    current: Option<Step>,
}

impl History {
    pub fn new(_gpu: &GpuContext, budget_bytes: u64, max_steps: usize) -> Self {
        let affordable = budget_bytes / bytes_per_tile_all_textures();
        let slot_capacity = affordable.min(u64::from(ATLAS_SLOTS)) as u32;
        History {
            slot_capacity,
            max_steps,
            atlas: None,
            free_slots: (0..slot_capacity).rev().collect(),
            undo_steps: VecDeque::new(),
            redo_steps: Vec::new(),
            current: None,
        }
    }

    pub fn slot_capacity(&self) -> u32 {
        self.slot_capacity
    }

    pub fn begin_step(&mut self) {
        self.commit_step();
        self.drop_redo_steps();
        self.current = Some(Step::new());
    }

    pub fn encode_save_tiles(
        &mut self,
        gpu: &GpuContext,
        canvas: &Canvas,
        tiles: &[TileCoord],
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let Some(mut step) = self.current.take() else {
            return;
        };
        for tile in tiles {
            if !step.complete {
                break;
            }
            if step.saved.contains(tile) || tile_extent(canvas, *tile).is_none() {
                continue;
            }
            let Some(slot) = self.allocate() else {
                step.complete = false;
                break;
            };
            self.copy_tile(
                gpu,
                canvas,
                *tile,
                slot,
                CopyDirection::CanvasToSlot,
                encoder,
            );
            step.saved.insert(*tile);
            step.tiles.push(SavedTile {
                tile: *tile,
                before: slot,
                after: None,
            });
        }
        self.current = Some(step);
    }

    pub fn commit_step(&mut self) {
        let Some(step) = self.current.take() else {
            return;
        };
        if !step.complete || step.tiles.is_empty() {
            self.release(step);
            return;
        }
        self.undo_steps.push_back(step);
        self.trim_to_max_steps();
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
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("undo"),
            });
        for saved in &step.tiles {
            if let Some(after) = saved.after {
                self.copy_tile(
                    gpu,
                    canvas,
                    saved.tile,
                    after,
                    CopyDirection::CanvasToSlot,
                    &mut encoder,
                );
            }
            self.copy_tile(
                gpu,
                canvas,
                saved.tile,
                saved.before,
                CopyDirection::SlotToCanvas,
                &mut encoder,
            );
            mark_restored(canvas, saved.tile);
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
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("redo"),
            });
        for saved in &step.tiles {
            if let Some(after) = saved.after {
                self.copy_tile(
                    gpu,
                    canvas,
                    saved.tile,
                    after,
                    CopyDirection::SlotToCanvas,
                    &mut encoder,
                );
                mark_restored(canvas, saved.tile);
            }
        }
        gpu.queue.submit([encoder.finish()]);
        self.undo_steps.push_back(step);
        self.trim_to_max_steps();
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_steps.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_steps.is_empty()
    }

    fn allocate(&mut self) -> Option<Slot> {
        loop {
            if let Some(slot) = self.free_slots.pop() {
                return Some(slot);
            }
            let oldest = self.undo_steps.pop_front()?;
            self.release(oldest);
        }
    }

    fn reserve_after_slots(&mut self, step: &mut Step) -> bool {
        for saved in step.tiles.iter_mut().filter(|saved| saved.after.is_none()) {
            let Some(slot) = self.allocate() else {
                return false;
            };
            saved.after = Some(slot);
        }
        true
    }

    fn release(&mut self, step: Step) {
        self.free_slots.extend(step.slots());
    }

    fn drop_redo_steps(&mut self) {
        while let Some(step) = self.redo_steps.pop() {
            self.release(step);
        }
    }

    fn trim_to_max_steps(&mut self) {
        while self.undo_steps.len() > self.max_steps {
            if let Some(oldest) = self.undo_steps.pop_front() {
                self.release(oldest);
            }
        }
    }

    fn copy_tile(
        &mut self,
        gpu: &GpuContext,
        canvas: &Canvas,
        tile: TileCoord,
        slot: Slot,
        direction: CopyDirection,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let Some(extent) = tile_extent(canvas, tile) else {
            return;
        };
        let slot_capacity = self.slot_capacity;
        let atlas = self
            .atlas
            .get_or_insert_with(|| Atlas::new(gpu, slot_capacity));
        let canvas_origin = wgpu::Origin3d {
            x: tile.x * TILE_SIZE,
            y: tile.y * TILE_SIZE,
            z: 0,
        };
        let slot_origin = wgpu::Origin3d {
            x: (slot % ATLAS_TILES_PER_SIDE) * TILE_SIZE,
            y: (slot / ATLAS_TILES_PER_SIDE) * TILE_SIZE,
            z: 0,
        };
        for (which, atlas_texture) in StateTexture::PAINT_STATE.iter().zip(&atlas.textures) {
            let canvas_copy = wgpu::TexelCopyTextureInfo {
                texture: canvas.texture(*which),
                mip_level: 0,
                origin: canvas_origin,
                aspect: wgpu::TextureAspect::All,
            };
            let slot_copy = wgpu::TexelCopyTextureInfo {
                texture: atlas_texture,
                mip_level: 0,
                origin: slot_origin,
                aspect: wgpu::TextureAspect::All,
            };
            match direction {
                CopyDirection::CanvasToSlot => {
                    encoder.copy_texture_to_texture(canvas_copy, slot_copy, extent)
                }
                CopyDirection::SlotToCanvas => {
                    encoder.copy_texture_to_texture(slot_copy, canvas_copy, extent)
                }
            }
        }
    }
}

fn tile_extent(canvas: &Canvas, tile: TileCoord) -> Option<wgpu::Extent3d> {
    let width = canvas
        .spec
        .width
        .saturating_sub(tile.x * TILE_SIZE)
        .min(TILE_SIZE);
    let height = canvas
        .spec
        .height
        .saturating_sub(tile.y * TILE_SIZE)
        .min(TILE_SIZE);
    (width > 0 && height > 0).then_some(wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    })
}

fn mark_restored(canvas: &mut Canvas, tile: TileCoord) {
    let Some(extent) = tile_extent(canvas, tile) else {
        return;
    };
    let x0 = (tile.x * TILE_SIZE) as f32;
    let y0 = (tile.y * TILE_SIZE) as f32;
    let x1 = x0 + (extent.width - 1) as f32;
    let y1 = y0 + (extent.height - 1) as f32;
    canvas.dirty.mark_rect(x0, y0, x1, y1);
    canvas.wet_tiles.mark_rect(x0, y0, x1, y1);
}
