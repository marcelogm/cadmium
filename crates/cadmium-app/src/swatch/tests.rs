use super::*;

fn style_manager_has_accent_colour_handler() -> bool {
    let style_manager = adw::StyleManager::default();
    let notify =
        glib::subclass::SignalId::lookup("notify", style_manager.type_()).expect("notify signal");
    glib::signal::signal_has_handler_pending(
        &style_manager,
        notify,
        Some(glib::Quark::from_str("accent-color")),
        true,
    )
}

#[test]
fn dropped_swatches_leave_no_accent_colour_handler_behind() {
    adw::init().expect("libadwaita initialises");
    let before = style_manager_has_accent_colour_handler();
    let swatches: Vec<_> = (0..64)
        .map(|_| Swatch::new([0, 0, 0], SwatchShape::Circle, 16, 16))
        .collect();
    assert!(style_manager_has_accent_colour_handler());
    let finalized: Vec<_> = swatches.iter().map(|swatch| swatch.downgrade()).collect();
    drop(swatches);
    assert!(finalized.iter().all(|swatch| swatch.upgrade().is_none()));
    assert_eq!(style_manager_has_accent_colour_handler(), before);
}
