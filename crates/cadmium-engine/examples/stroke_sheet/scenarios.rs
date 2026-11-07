use cadmium_engine::stroke::Tool;

pub(crate) const SPEEDS_PX_PER_SECOND: [f64; 4] = [800.0, 2000.0, 4000.0, 8000.0];

const BRUSH_SIZES_PX: [f32; 4] = [40.0, 120.0, 200.0, 300.0];
const BRUSH_LOADS: [f32; 3] = [1.0, 0.5, 0.15];
const KNIFE_SIZES_PX: [f32; 2] = [40.0, 120.0];
const KNIFE_DIPS: [f32; 2] = [1.0, 0.3];
const BLENDER_SIZES_PX: [f32; 2] = [40.0, 120.0];
const BLENDER_LOAD: f32 = 0.0;

#[derive(Clone, Copy)]
pub(crate) struct Cell {
    pub(crate) tool: Tool,
    pub(crate) diameter_px: f32,
    pub(crate) load: f32,
    pub(crate) speed_px_per_second: f64,
}

impl Cell {
    pub(crate) fn row_name(&self) -> String {
        let tool = match self.tool {
            Tool::Knife => "knife",
            Tool::Blender => "blender",
            _ => "brush",
        };
        if self.tool == Tool::Blender {
            return format!("{tool} {:.0} px", self.diameter_px);
        }
        format!(
            "{tool} {:.0} px, load {:.0}%",
            self.diameter_px,
            self.load * 100.0
        )
    }

    pub(crate) fn file_stem(&self) -> String {
        format!(
            "{}_{:.0}",
            self.row_name()
                .replace([' ', ',', '%'], "_")
                .replace("__", "_"),
            self.speed_px_per_second
        )
    }
}

fn rows() -> Vec<(Tool, f32, f32)> {
    let mut rows = Vec::new();
    for diameter in BRUSH_SIZES_PX {
        rows.extend(BRUSH_LOADS.map(|load| (Tool::Brush, diameter, load)));
    }
    for diameter in KNIFE_SIZES_PX {
        rows.extend(KNIFE_DIPS.map(|dip| (Tool::Knife, diameter, dip)));
    }
    rows.extend(BLENDER_SIZES_PX.map(|diameter| (Tool::Blender, diameter, BLENDER_LOAD)));
    rows
}

pub(crate) fn cells() -> Vec<Cell> {
    let filter = std::env::var("SHEET_ROWS").ok();
    rows()
        .into_iter()
        .flat_map(|(tool, diameter_px, load)| {
            SPEEDS_PX_PER_SECOND.map(|speed_px_per_second| Cell {
                tool,
                diameter_px,
                load,
                speed_px_per_second,
            })
        })
        .filter(|cell| {
            filter
                .as_ref()
                .is_none_or(|text| cell.row_name().contains(text.as_str()))
        })
        .collect()
}
