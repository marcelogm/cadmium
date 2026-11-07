use super::*;

fn texel(bands: [f32; 4]) -> Vec<u8> {
    bands
        .iter()
        .flat_map(|band| f32_to_half(*band).to_le_bytes())
        .collect()
}

#[test]
fn bare_texels_of_an_old_tray_move_onto_todays_ground() {
    let mut raw = [texel([0.75; 4]), texel([0.75; 4])].concat();
    rebase_tray_ground(StateTexture::DryRefl0, &mut raw, 0.4);
    assert_eq!(raw, [texel([0.4; 4]), texel([0.4; 4])].concat());
}

#[test]
fn dried_paint_and_other_states_keep_what_they_hold() {
    let paint = texel([0.75, 0.2, 0.75, 0.75]);
    let mut raw = paint.clone();
    rebase_tray_ground(StateTexture::DryRefl1, &mut raw, 0.4);
    assert_eq!(raw, paint);
    let mut volume = texel([0.75; 4]);
    rebase_tray_ground(StateTexture::WetPig0, &mut volume, 0.4);
    assert_eq!(volume, texel([0.75; 4]));
}

#[test]
fn a_tray_on_todays_ground_is_left_alone() {
    let mut raw = texel([0.4; 4]);
    rebase_tray_ground(StateTexture::DryRefl0, &mut raw, 0.4);
    assert_eq!(raw, texel([0.4; 4]));
}
