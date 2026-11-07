use super::knife_top_up;

#[test]
fn knife_tops_up_only_the_share_it_is_missing() {
    assert_eq!(knife_top_up(0.0, 400.0), Some(1.0));
    assert_eq!(knife_top_up(300.0, 400.0), Some(0.25));
    assert_eq!(knife_top_up(400.0, 400.0), None);
    assert_eq!(knife_top_up(500.0, 400.0), None);
    assert_eq!(knife_top_up(-5.0, 400.0), Some(1.0));
    assert_eq!(knife_top_up(0.0, 0.0), None);
    assert_eq!(knife_top_up(f32::NAN, 400.0), None);
}
