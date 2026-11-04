use cadmium_engine::canvas::MAX_CANVAS_SIZE;

pub(super) const MIN_SIDE: u32 = 64;
pub(super) const SIDE_STEP: u32 = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintingFormat {
    Landscape4x3,
    Portrait3x4,
    Square,
    Landscape16x9,
    Custom,
}

impl PaintingFormat {
    pub const ALL: [PaintingFormat; 5] = [
        PaintingFormat::Landscape4x3,
        PaintingFormat::Portrait3x4,
        PaintingFormat::Square,
        PaintingFormat::Landscape16x9,
        PaintingFormat::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PaintingFormat::Landscape4x3 => "Paisagem 4:3",
            PaintingFormat::Portrait3x4 => "Retrato 3:4",
            PaintingFormat::Square => "Quadrado",
            PaintingFormat::Landscape16x9 => "Paisagem 16:9",
            PaintingFormat::Custom => "Personalizado",
        }
    }

    fn aspect(self) -> Option<(u32, u32)> {
        match self {
            PaintingFormat::Landscape4x3 => Some((4, 3)),
            PaintingFormat::Portrait3x4 => Some((3, 4)),
            PaintingFormat::Square => Some((1, 1)),
            PaintingFormat::Landscape16x9 => Some((16, 9)),
            PaintingFormat::Custom => None,
        }
    }

    pub fn size_keeping_long_side(self, width: u32, height: u32) -> Option<(u32, u32)> {
        let (aspect_width, aspect_height) = self.aspect()?;
        let long = width.max(height).clamp(MIN_SIDE, MAX_CANVAS_SIZE);
        let short = |long_part: u32, short_part: u32| {
            ((long as f64 * short_part as f64 / long_part as f64).round() as u32)
                .clamp(MIN_SIDE, MAX_CANVAS_SIZE)
        };
        Some(if aspect_width >= aspect_height {
            (long, short(aspect_width, aspect_height))
        } else {
            (short(aspect_height, aspect_width), long)
        })
    }

    pub fn matching(width: u32, height: u32) -> PaintingFormat {
        PaintingFormat::ALL
            .into_iter()
            .find(|format| format.size_keeping_long_side(width, height) == Some((width, height)))
            .unwrap_or(PaintingFormat::Custom)
    }

    pub(super) fn index(self) -> u32 {
        PaintingFormat::ALL
            .iter()
            .position(|format| *format == self)
            .unwrap_or_default() as u32
    }
}

#[cfg(test)]
mod tests;
