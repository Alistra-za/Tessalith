//! Backlog batch B4 (Note FT): input-name edits (T34, T35).
mod support;
use sdc_gui::model::{Answer, AppModel};
use sdc_gui::settings::GuiSettings;
use std::time::Duration;
use support::*;

const ROCK_SET: &str = "landscape/rock01";

fn idle(m: &mut AppModel) {
    assert!(
        m.wait_idle(Duration::from_secs(120)),
        "planning did not finish"
    );
}

/// T34: an input rename moves the set's `[export] skip` entry to the new automatic id.
#[test]
fn an_input_rename_moves_the_skip_entry() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.set_skip_also(ROCK_SET, true);
    idle(&mut m);
    assert!(m.also_skipped(ROCK_SET));
    m.set_input_name(ROCK_SET, "textures/landscape/rock02")
        .unwrap();
    idle(&mut m);
    assert!(m.also_skipped("landscape/rock02"), "the skip follows");
    assert!(!m.also_skipped(ROCK_SET), "no skip left on the old id");
}

/// Note HC3: the box shows the name without the diffuse's own suffix, as every member keeps its suffix under any
/// name. The owner's `body_albedo` showed whole, so typing `body_albedo-` wrote `body_albedo-_albedo`.
#[test]
fn the_box_shows_and_takes_the_name_without_the_diffuses_suffix() {
    use engine::config::{GreenConvention, Subtype};
    let d = tempfile::tempdir().unwrap();
    let root = mod_folder(d.path());
    let r = root.join("landscape");
    std::fs::rename(r.join("rock01.png"), r.join("rock01_albedo.png")).unwrap();
    let settings = GuiSettings {
        export_folder: Some(d.path().join("export")),
        ..GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, session());
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(root);
    idle(&mut m);
    choose_export(&mut m, d.path().join("export"));
    m.decide(
        "landscape/rock01_albedo.png",
        Answer::Subtype(Subtype::PbrAlbedo),
    );
    m.decide(
        "landscape/rock01_normal.png",
        Answer::Subtype(Subtype::PbrNormal),
    );
    m.decide(
        "landscape/rock01_normal.png",
        Answer::Green(GreenConvention::Directx),
    );
    idle(&mut m);
    assert_eq!(
        m.input_name_of(ROCK_SET).as_deref(),
        Some("textures/landscape/rock01")
    );
    assert_eq!(m.input_name_tail(ROCK_SET), "_albedo");
    // The set's own name, with or without the suffix, changes nothing.
    for typed in [
        "textures/landscape/rock01",
        "textures/landscape/rock01_albedo",
    ] {
        let e = m.set_input_name(ROCK_SET, typed).unwrap_err();
        assert!(e.contains("already has this name"), "{typed}: {e}");
    }
    // A new name typed with the suffix does not double it.
    m.set_input_name(ROCK_SET, "textures/landscape/rock02_albedo")
        .unwrap();
    idle(&mut m);
    let new = "landscape/rock02";
    assert_eq!(
        m.input_name_of(new).as_deref(),
        Some("textures/landscape/rock02")
    );
    assert_eq!(m.input_name_tail(new), "_albedo");
    // And one typed without it follows.
    m.set_input_name(new, "textures/landscape/rock03").unwrap();
    idle(&mut m);
    assert_eq!(
        m.input_name_of("landscape/rock03").as_deref(),
        Some("textures/landscape/rock03")
    );
}

/// T35: the set's own name, in another case, is refused: no alias, no undo step.
#[test]
fn the_sets_current_name_in_another_case_is_refused() {
    use engine::config::{GreenConvention, Subtype};
    let d = tempfile::tempdir().unwrap();
    let root = mod_folder(d.path());
    let r = root.join("landscape");
    for (from, to) in [
        ("rock01.png", "Rock01.png"),
        ("rock01_normal.png", "Rock01_normal.png"),
    ] {
        std::fs::rename(r.join(from), r.join(to)).unwrap();
    }
    let settings = GuiSettings {
        export_folder: Some(d.path().join("export")),
        ..GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, session());
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(root);
    idle(&mut m);
    choose_export(&mut m, d.path().join("export"));
    m.decide("landscape/Rock01.png", Answer::Subtype(Subtype::PbrAlbedo));
    m.decide(
        "landscape/Rock01_normal.png",
        Answer::Subtype(Subtype::PbrNormal),
    );
    m.decide(
        "landscape/Rock01_normal.png",
        Answer::Green(GreenConvention::Directx),
    );
    idle(&mut m);
    let id = "landscape/rock01";
    let steps = m.history.undo.len();
    for typed in [
        "textures/landscape/rock01",
        "textures/landscape/rock01.dds",
        "textures\\Landscape\\ROCK01",
        "TEXTURES/landscape/rock01.DDS",
    ] {
        let e = m.set_input_name(id, typed).unwrap_err();
        assert!(e.contains("already has this name"), "{typed}: {e}");
    }
    assert_eq!(m.history.undo.len(), steps, "no undo step");
}
