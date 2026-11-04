use cadmium_engine::engine::PaintEngine;
use cadmium_engine::pigments::PigmentDb;
use cadmium_engine::render::ViewMode;

use super::ParamControl;
use crate::state::AppState;

const TINT_WHITE_PARTS: f32 = 1.0;

pub(super) struct SlotInfo {
    pub(super) name: &'static str,
    pub(super) subtitle: String,
    pub(super) masstone: [u8; 3],
    pub(super) tint: [u8; 3],
    pub(super) glaze_over_white: [u8; 3],
    pub(super) glaze_over_black: [u8; 3],
}

pub(super) struct Snapshot {
    pub(super) slots: Vec<String>,
    pub(super) infos: Vec<SlotInfo>,
    pub(super) mode: ViewMode,
    pub(super) values: Vec<f32>,
}

impl Snapshot {
    pub(super) fn of(state: &AppState, controls: &[ParamControl]) -> Snapshot {
        let session = &state.session;
        let slots = session.canvas.spec.pigment_slots.clone();
        let infos = slot_infos(&session.db, &slots);
        let params = session.engine.params();
        let values: Vec<f32> = controls
            .iter()
            .map(|control| params.get(control.spec.key))
            .collect();
        Snapshot {
            slots,
            infos,
            mode: session.view_mode,
            values,
        }
    }
}

fn slot_infos(db: &PigmentDb, slots: &[String]) -> Vec<SlotInfo> {
    slots
        .iter()
        .filter_map(|id| db.get(id))
        .map(|pigment| SlotInfo {
            name: pigment.name,
            subtitle: format!("{} · {}", pigment.colour_index, pigment.provenance),
            masstone: db.masstone_srgb8(pigment.id),
            tint: db.tint_srgb8(pigment.id, TINT_WHITE_PARTS),
            glaze_over_white: db.glaze_srgb8(pigment.id, false),
            glaze_over_black: db.glaze_srgb8(pigment.id, true),
        })
        .collect()
}
