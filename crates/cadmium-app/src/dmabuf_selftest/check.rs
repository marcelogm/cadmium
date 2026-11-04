use gtk::prelude::*;
use gtk::{gdk, glib, graphene, gsk};

use super::pattern::FramePattern;
use super::{HEIGHT, WIDTH};

const SYNC_FILE_LINK: &str = "anon_inode:sync_file";
const DMABUF_LINK_PREFIX: &str = "/dmabuf:";
const CHANNEL_TOLERANCE: u8 = 1;

pub(super) struct Pixels {
    bytes: glib::Bytes,
    stride: usize,
}

pub(super) fn download(texture: &gdk::Texture) -> Pixels {
    let mut downloader = gdk::TextureDownloader::new(texture);
    downloader.set_format(gdk::MemoryFormat::R8g8b8a8);
    let (bytes, stride) = downloader.download_bytes();
    Pixels { bytes, stride }
}

pub(super) fn render_through_gsk(
    native: &gtk::Native,
    texture: &gdk::Texture,
) -> Result<Pixels, String> {
    let renderer = native
        .renderer()
        .ok_or_else(|| "window has no renderer".to_string())?;
    let bounds = graphene::Rect::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32);
    let node = gsk::TextureNode::new(texture, &bounds);
    let rendered = renderer.render_texture(&node, Some(&bounds));
    if (rendered.width(), rendered.height()) != (WIDTH as i32, HEIGHT as i32) {
        return Err(format!(
            "rendered {}x{} instead of {WIDTH}x{HEIGHT}",
            rendered.width(),
            rendered.height()
        ));
    }
    Ok(download(&rendered))
}

pub(super) fn check_pixels(pixels: &Pixels, pattern: FramePattern) -> Result<u8, String> {
    let data: &[u8] = &pixels.bytes;
    let mut worst = 0u8;
    for y in 0..HEIGHT {
        let row = y as usize * pixels.stride;
        for x in 0..WIDTH {
            let error = pixel_error(data, row, x, y, pattern)?;
            worst = worst.max(error);
        }
    }
    Ok(worst)
}

fn pixel_error(
    data: &[u8],
    row: usize,
    x: u32,
    y: u32,
    pattern: FramePattern,
) -> Result<u8, String> {
    let start = row + x as usize * 4;
    let actual = data
        .get(start..start + 4)
        .ok_or_else(|| format!("download too short at ({x}, {y})"))?;
    let expected = pattern.expected(x, y);
    let error = actual
        .iter()
        .zip(expected)
        .map(|(&got, want)| got.abs_diff(want))
        .max()
        .unwrap_or(0);
    if error > CHANNEL_TOLERANCE {
        return Err(format!(
            "pixel ({x}, {y}) is {actual:?}, expected {expected:?}"
        ));
    }
    Ok(error)
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct FdCensus {
    pub(super) total: usize,
    sync_files: usize,
    pub(super) dmabufs: usize,
}

impl FdCensus {
    pub(super) fn take() -> FdCensus {
        let mut census = FdCensus::default();
        let Ok(entries) = std::fs::read_dir("/proc/self/fd") else {
            return census;
        };
        for entry in entries.flatten() {
            census.count_entry(&entry);
        }
        census
    }

    fn count_entry(&mut self, entry: &std::fs::DirEntry) {
        self.total += 1;
        let Ok(target) = std::fs::read_link(entry.path()) else {
            return;
        };
        let target = target.to_string_lossy();
        if target == SYNC_FILE_LINK {
            self.sync_files += 1;
        } else if target.starts_with(DMABUF_LINK_PREFIX) {
            self.dmabufs += 1;
        }
    }
}

impl std::fmt::Display for FdCensus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} total/{} sync_file/{} dmabuf",
            self.total, self.sync_files, self.dmabufs
        )
    }
}
