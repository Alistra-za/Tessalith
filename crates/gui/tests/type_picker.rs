//! Owner amendment J (Note FB9): Set type and the texture panel's Sub-type share one picker: a popup anchored to its
//! button, the suggestion pinned on top, a search box focused on open, the grouped list below; Esc closes it.
mod support;
use eframe::egui::{self, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use engine::config::{Subtype, Target};
use sdc_gui::model::{AppModel, Selection};
use sdc_gui::ui::settings_panel::subtype_label;
use sdc_gui::ui::type_picker::{self, TypeKind};
use support::*;

const PICKER: &str = "test-picker";

struct State {
    top: f32,
    picked: Option<Subtype>,
}

/// A "Set type…" button `top` px down a 600×700 window, with the picker on it (subsurface tint suggested).
fn bare(top: f32) -> Harness<'static, State> {
    Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(600.0, 700.0))
        .build_ui_state(
            |ui, s: &mut State| {
                ui.add_space(s.top);
                let b = ui.button("Set type…");
                let id = egui::Id::unique(PICKER);
                let suggested = Some(Subtype::SubsurfaceTint);
                let kind = TypeKind::SetType;
                if let Some(st) =
                    type_picker::show(ui, id, &b, None, suggested, kind, Target::Cs, false)
                {
                    s.picked = Some(st);
                }
            },
            State { top, picked: None },
        )
}

fn popup_rect(h: &Harness<'static, State>) -> egui::Rect {
    h.ctx
        .memory(|m| m.area_rect(egui::Id::unique(PICKER)))
        .expect("the picker is drawn")
}

fn open(h: &mut Harness<'static, State>) -> egui::Rect {
    h.run();
    let b = h.get_by_label("Set type…").rect();
    h.get_by_label("Set type…").click();
    h.run();
    b
}

#[test]
fn the_type_menu_shows_descriptions_and_finds_by_them() {
    let mut h = bare(40.0);
    support::instant_tips(&h.ctx); // `bare` keeps egui's tooltip delay; the per-file harness keeps the default tooltip delay
    open(&mut h);
    let search = h.get_by_role(egui::accesskit::Role::TextInput); // the box keeps focus on open
    search.type_text("bumps");
    h.run();
    assert!(h.query_by_label("Normal map").is_some());
    assert!(h.query_by_label("Diffuse").is_none());
    h.get_by_label("Normal map").hover();
    h.run();
    assert!(h.query_by_label_contains("File name: _n").is_some());
    // the id still finds its type (a fresh harness, so the box starts empty)
    let mut h = bare(40.0);
    open(&mut h);
    h.get_by_role(egui::accesskit::Role::TextInput)
        .type_text("envmask");
    h.run();
    assert!(h.query_by_label("Environment mask").is_some());
}

/// Every type the cs target offers.
fn offered_types() -> impl Iterator<Item = Subtype> {
    Subtype::ALL
        .into_iter()
        .filter(|s| engine::policy::offered(Target::Cs, *s))
}

#[test]
fn type_picker_opens_at_its_anchor() {
    // Room below: the popup's top is just under the button.
    let mut h = bare(10.0);
    let b = open(&mut h);
    let p = popup_rect(&h);
    assert!(
        p.top() >= b.bottom() - 1.0 && p.top() - b.bottom() <= 40.0,
        "popup {p:?} under button {b:?}"
    );
    let search = h.get_by_role(Role::TextInput);
    assert!(search.is_focused(), "the search box has focus on open");
    // The suggestion is the first entry, above every listed type.
    let first = h.get_by_label("Subsurface tint (suggested)").rect();
    assert!(first.top() > search.rect().top());
    assert!(support::painted_in(&h, first)
        .contains(sdc_gui::model::labels::type_info(Subtype::SubsurfaceTint).description));
    for l in offered_types().map(subtype_label) {
        if let Some(n) = h.query_by_label(&l) {
            assert!(n.rect().top() > first.top(), "{l} above the suggestion");
        }
    }
    // Typing narrows the list to the entries containing the text.
    h.get_by_role(Role::TextInput).type_text("tint");
    h.run();
    for st in offered_types() {
        let info = sdc_gui::model::labels::type_info(st);
        let matches = [info.label, info.description, st.id().as_str()]
            .iter()
            .any(|s| s.to_lowercase().replace('_', " ").contains("tint"));
        assert_eq!(
            h.query_by_label(info.label).is_some(),
            matches,
            "{st:?} after typing tint"
        );
    }
    h.get_by_label("Subsurface tint (suggested)");
    // Esc closes it, and a pick returns the type and closes it.
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(h.query_by_role(Role::TextInput).is_none(), "Esc closes it");
    open(&mut h);
    h.get_by_label("Diffuse");
    // The list scrolls: narrow it so the entry is on screen, as a user would.
    h.get_by_role(Role::TextInput).type_text("back");
    h.run();
    h.get_by_label("Backlight tint").click();
    h.run();
    assert_eq!(h.state().picked, Some(Subtype::BacklightTint));
    assert!(
        h.query_by_role(Role::TextInput).is_none(),
        "a pick closes it"
    );

    // No room below: it flips above, just over the button.
    let mut h = bare(650.0);
    let b = open(&mut h);
    let p = popup_rect(&h);
    assert!(
        p.bottom() <= b.top() + 1.0 && b.top() - p.bottom() <= 40.0,
        "popup {p:?} over button {b:?}"
    );
}

/// Note FM37: Gray is picked by hand from the Grayscale group, under both targets.
#[test]
fn gray_is_in_the_grayscale_group() {
    for target in [Target::Cs, Target::Vanilla] {
        let groups = sdc_gui::ui::settings_panel::offered_groups(target, false);
        let (name, members) = groups
            .iter()
            .find(|(_, v)| v.contains(&Subtype::Gray))
            .unwrap_or_else(|| panic!("{target:?}: Gray is in no group"));
        assert_eq!(*name, "Grayscale maps", "{target:?}");
        assert!(members.contains(&Subtype::Height), "{target:?}");
    }
    assert_eq!(subtype_label(Subtype::Gray), "Grey map");
    // The picker lists it and a pick returns it.
    let mut h = bare(10.0);
    open(&mut h);
    h.get_by_role(Role::TextInput).type_text("gray");
    h.run();
    h.get_by_label("Grey map").click();
    h.run();
    assert_eq!(h.state().picked, Some(Subtype::Gray));
}

/// Review C7 round 1: the Sub-type button keeps the old combo's gating. A read-only sdc.toml disables it (the
/// panel's `add_enabled_ui(!read_only)`); `busy` never disabled the combo (only sliders defer their commit), so it
/// stays enabled while a plan runs, like the other combos in the panel.
#[test]
fn subtype_button_is_disabled_on_a_read_only_sdc_toml() {
    let d = tempfile::tempdir().unwrap();
    let root = mod_folder(d.path());
    std::fs::write(
        root.join("sdc.toml"),
        "[files.\"landscape/rock01.png\"]\nbogus_field = 1\n",
    )
    .unwrap();
    let settings = sdc_gui::settings::GuiSettings {
        export_folder: Some(d.path().join("export")),
        ..Default::default()
    };
    let mut m = AppModel::new(settings, None, session());
    m.open_folder(root);
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    assert!(m.project.as_ref().unwrap().read_only());
    m.select_item("landscape/rock01_normal.png");
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1400.0, 900.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    h.run();
    let b = h.get_by_role_and_label(Role::ComboBox, "Texture type");
    assert!(b.accesskit_node().is_disabled(), "Type is enabled");
    b.click();
    h.run();
    h.run();
    assert!(
        h.query_all_by_role(Role::TextInput)
            .all(|n| !n.is_focused()),
        "a disabled button opened the picker"
    );
}

#[test]
fn subtype_combo_uses_the_same_picker() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    decide_all(&mut m);
    let key = "landscape/rock01.png";
    m.select_item(key);
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1400.0, 900.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    h.run();
    let inputs = |h: &Harness<'static, AppModel>| h.query_all_by_role(Role::TextInput).count();
    let before = inputs(&h);
    let combo = || {
        h.query_all_by_role(Role::ComboBox)
            .find(|n| n.value().as_deref() == Some("Diffuse"))
            .expect("the type control shows the current type")
            .rect()
    };
    let at = combo();
    h.query_all_by_role(Role::ComboBox)
        .find(|n| n.value().as_deref() == Some("Diffuse"))
        .unwrap()
        .click();
    h.run();
    h.run();
    assert_eq!(inputs(&h), before + 1, "the picker's search box");
    let search = h
        .query_all_by_role(Role::TextInput)
        .find(|n| n.is_focused())
        .expect("the search box has focus");
    let gap = (search.rect().top() - at.bottom()).min((at.top() - search.rect().bottom()).abs());
    assert!(gap <= 40.0, "search {:?} far from {at:?}", search.rect());
    // Esc closes only the picker: the texture stays selected (owner amendment G).
    h.key_press(egui::Key::Escape);
    h.run();
    assert_eq!(inputs(&h), before);
    assert!(matches!(&h.state().selection, Selection::Item(k) if k == key));
    // A pick edits the field.
    h.query_all_by_role(Role::ComboBox)
        .find(|n| n.value().as_deref() == Some("Diffuse"))
        .unwrap()
        .click();
    h.run();
    h.run();
    h.query_all_by_role(Role::TextInput)
        .find(|n| n.is_focused())
        .unwrap()
        .type_text("tint");
    h.run();
    h.get_by_label("Subsurface tint").click();
    h.run();
    assert!(h.state_mut().wait_idle(std::time::Duration::from_secs(60)));
    let v = h
        .state()
        .project
        .as_ref()
        .unwrap()
        .file_value(key, "subtype");
    assert_eq!(v, Some(serde_json::json!("subsurface_tint")));
}
