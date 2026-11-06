//! The `.cadbrush` file: one brush as versioned JSON.

use serde::{Deserialize, Serialize};

use super::{BrushSpec, HairSpec, HeadSpec};

pub const BRUSH_EXTENSION: &str = "cadbrush";
const FORMAT: &str = "cadmium-brush";
const VERSION: u32 = 1;
pub(crate) const UNNAMED_BRUSH: &str = "Imported Brush";
/// Longer names are cut to this many characters on reading.
pub const MAX_NAME_CHARS: usize = 80;

#[derive(Serialize, Deserialize)]
struct BrushFile {
    format: String,
    version: u32,
    name: String,
    size_px: f32,
    #[serde(default)]
    head: HeadSpec,
    #[serde(default)]
    hair: HairSpec,
}

#[derive(Debug)]
pub enum BrushFileError {
    Malformed(String),
    NotABrush,
    NewerVersion(u32),
}

impl std::fmt::Display for BrushFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BrushFileError::Malformed(reason) => write!(f, "not a readable brush file ({reason})"),
            BrushFileError::NotABrush => write!(f, "not a Cadmium brush"),
            BrushFileError::NewerVersion(version) => {
                write!(f, "made by a newer Cadmium (brush format {version})")
            }
        }
    }
}

impl std::error::Error for BrushFileError {}

/// A brush read from a file, and whether any value had to be pulled into range.
#[derive(Debug)]
pub struct ReadBrush {
    pub brush: BrushSpec,
    pub adjusted: bool,
}

pub fn brush_to_json(brush: &BrushSpec) -> String {
    let file = BrushFile {
        format: FORMAT.to_string(),
        version: VERSION,
        name: brush.name.clone(),
        size_px: brush.size_px,
        head: brush.head,
        hair: brush.hair,
    };
    serde_json::to_string_pretty(&file).expect("a brush always serializes")
}

pub fn brush_from_json(text: &str) -> Result<ReadBrush, BrushFileError> {
    let file: BrushFile =
        serde_json::from_str(text).map_err(|error| BrushFileError::Malformed(error.to_string()))?;
    if file.format != FORMAT {
        return Err(BrushFileError::NotABrush);
    }
    if file.version > VERSION {
        return Err(BrushFileError::NewerVersion(file.version));
    }
    let (name, cut) = brush_name(&file.name);
    let raw = BrushSpec {
        name,
        size_px: file.size_px,
        head: file.head,
        hair: file.hair,
    };
    let brush = raw.clamped();
    Ok(ReadBrush {
        adjusted: cut || brush != raw,
        brush,
    })
}

/// The trimmed name, or a placeholder when blank, and whether it had to be cut short.
fn brush_name(name: &str) -> (String, bool) {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return (UNNAMED_BRUSH.to_string(), false);
    }
    if trimmed.chars().count() <= MAX_NAME_CHARS {
        return (trimmed.to_string(), false);
    }
    let cut: String = trimmed.chars().take(MAX_NAME_CHARS).collect();
    (cut.trim_end().to_string(), true)
}

#[cfg(test)]
mod tests;
