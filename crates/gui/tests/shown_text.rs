//! `support::shown_containing` against the two cases it must tell apart on egui 0.37: a visible Label whose AccessKit
//! name and hover text are both its full text (Tessalith's rule for a cut-short text), and an unnamed widget whose
//! name egui copied from its hover text.
mod support;
use eframe::egui::{self, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use support::shown_containing;

const FULL: &str = "landscape/rock01_p.png is replaced by landscape/rock01_p_generated.png";
const HOVER: &str = "Only on hover: the reason";

fn harness(app: impl FnMut(&mut egui::Ui) + 'static) -> Harness<'static> {
    Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(400.0, 200.0))
        .build_ui(app)
}

/// A cut-short Label keeps its full text as its name and its hover; it is on screen, so it is counted.
#[test]
fn a_truncated_label_named_by_its_full_text_is_shown() {
    let mut h = harness(|ui| {
        ui.allocate_ui(egui::vec2(160.0, 20.0), |ui| {
            ui.add(egui::Label::new(FULL).truncate())
                .on_hover_text(FULL)
                .accessible_name(FULL);
        });
    });
    h.run();
    // The fixture is the case in question: a Label whose name and description are both the full text.
    let l = h.get_by_role(Role::Label);
    let a = l.accesskit_node();
    assert_eq!(a.label().as_deref(), Some(FULL));
    assert_eq!(a.description().as_deref(), Some(FULL));
    // Cut short on screen (its value stays the full text, which is what a label query reads).
    assert!(l.rect().width() <= 160.5, "{:?}", l.rect());
    assert_eq!(shown_containing(&h, "replaced by").len(), 1);
}

/// An unnamed widget with hover text: egui 0.37 copies the text into its name, which is not on screen until the tooltip
/// shows; the tooltip's own Label is then counted.
#[test]
fn a_name_copied_from_hover_text_is_left_out_until_the_tooltip_shows() {
    let mut h = harness(|ui| {
        ui.allocate_response(egui::vec2(60.0, 20.0), egui::Sense::click())
            .on_hover_text(HOVER);
    });
    h.run();
    let copy = h.get_by_label(HOVER);
    let a = copy.accesskit_node();
    assert_eq!(a.role(), Role::Unknown, "the copy egui 0.37 makes");
    assert_eq!(a.description().as_deref(), Some(HOVER));
    let at = copy.rect().center();
    assert!(shown_containing(&h, "Only on hover").is_empty());
    h.hover_at(at);
    h.run();
    let shown = shown_containing(&h, "Only on hover");
    assert_eq!(shown.len(), 1, "the tooltip shows it");
    assert_eq!(shown[0].accesskit_node().role(), Role::Label);
}
