use super::*;

fn texel(bands: [f32; 4]) -> Vec<u8> {
    bands
        .iter()
        .flat_map(|band| f32_to_half(*band).to_le_bytes())
        .collect()
}

#[test]
fn bare_texels_of_an_old_tray_move_onto_todays_ground() {
    let old = [texel([0.75; 4]), texel([0.75; 4])].concat();
    let (mut low, mut high) = (old.clone(), old);
    rebase_tray_ground(&mut low, &mut high, 0.4);
    let today = [texel([0.4; 4]), texel([0.4; 4])].concat();
    assert_eq!((low, high), (today.clone(), today));
}

#[test]
fn a_texel_is_ground_only_when_all_eight_bands_are() {
    // Dried paint that happens to show 0.75 in its four lower bands keeps them.
    let mut low = texel([0.75; 4]);
    let mut high = texel([0.75, 0.2, 0.75, 0.75]);
    let (low_before, high_before) = (low.clone(), high.clone());
    rebase_tray_ground(&mut low, &mut high, 0.4);
    assert_eq!((low, high), (low_before, high_before));
}

#[test]
fn a_tray_on_todays_ground_is_left_alone() {
    let (mut low, mut high) = (texel([0.4; 4]), texel([0.4; 4]));
    rebase_tray_ground(&mut low, &mut high, 0.4);
    assert_eq!((low, high), (texel([0.4; 4]), texel([0.4; 4])));
}
