use super::view::markup;

#[test]
fn a_labels_arrow_is_set_in_sans() {
    let label = markup("LIGHT → HEAVY");
    assert!(
        label.contains("LIGHT <span font_family=\"sans\">→</span> HEAVY"),
        "{label}"
    );
}

#[test]
fn a_labels_text_is_escaped() {
    let label = markup("A & <B>");
    assert!(label.contains(">A &amp; &lt;B&gt;</span>"), "{label}");
}
