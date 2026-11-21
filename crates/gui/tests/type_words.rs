#[test]
fn every_type_has_a_label_a_description_and_a_suffix() {
    for st in engine::config::Subtype::ALL {
        // config.rs:360, [Subtype; 34]
        let i = sdc_gui::model::labels::type_info(st);
        assert!(
            !i.label.is_empty() && !i.description.is_empty() && !i.suffix.is_empty(),
            "{st:?}"
        );
        assert!(
            !i.label.contains('_'),
            "{st:?} label has an underscore: {}",
            i.label
        );
        assert!(i.description.ends_with('.'), "{st:?}");
        assert!(
            !i.description.contains('`') && !i.suffix.contains('`'),
            "{st:?} ships Markdown backticks"
        ); // round 3
    }
}

#[test]
fn the_table_reads_like_the_wiki() {
    use engine::config::Subtype::*;
    use sdc_gui::model::labels::{subtype_name, type_info};
    assert_eq!(subtype_name(SkinSk), "Skin subsurface mask");
    assert_eq!(subtype_name(Msn), "Model-space normal");
    assert_eq!(subtype_name(Envmask), "Environment mask");
    assert_eq!(subtype_name(GenericTwoChannel), "Two-channel data");
    assert_eq!(subtype_name(Rmaos), "PBR RMAOS");
    assert_eq!(type_info(Rmaos).suffix, "_rmaos");
    assert_eq!(
        type_info(Normal).description,
        "The surface bumps of an ordinary material (tangent space)."
    );
    assert_eq!(
        sdc_gui::model::labels::type_hover(Normal),
        "The surface bumps of an ordinary material (tangent space).\nFile name: _n\nA file alone under its base name becomes a set of its own once it has a type."
    );
    let gray = sdc_gui::model::labels::type_hover(Gray);
    assert!(
        gray.contains("No file ending gives this type (picked by hand)")
            && !gray.contains("File name: none ("),
        "{gray}"
    );
}
