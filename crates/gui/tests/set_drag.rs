//! Notes JW1 to JW7: set cards and dragging (owner additions A4 to A9).
mod support;
use eframe::egui;
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use image::{ImageBuffer, Luma, Rgb};
use sdc_gui::model::AppModel;
use sdc_gui::ui::sets_column::Pending;
use sdc_gui::ui::State;
use std::path::Path;
use std::time::Duration;
use support::*;

const AO: &str = "a/rock_ao.png";
const A: &str = "a/rock";
const B: &str = "b/stone";

/// `textures/<f>` for each of `files` (a colour ramp for a name with no `_`, a grey ramp otherwise), `toml` as the
/// `sdc.toml`, opened as the app opens a folder.
fn fixture(dir: &Path, files: &[&str], toml: &str) -> AppModel {
    let root = dir.join("textures");
    for f in files {
        let p = root.join(f);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        if p.file_stem().unwrap().to_string_lossy().contains('_') {
            ImageBuffer::from_fn(64, 64, |x, y| Luma([(x * 3 + y) as u8]))
                .save(p)
                .unwrap();
        } else {
            ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
                .save(p)
                .unwrap();
        }
    }
    if !toml.is_empty() {
        std::fs::write(root.join("sdc.toml"), toml).unwrap();
    }
    let mut m = opened_as_the_app(root, dir.join("export"));
    idle(&mut m);
    m
}

/// Task 1's fixture: set `a` (rock, rock_ao) and set `b` (stone, stone_roughness).
fn ab(dir: &Path) -> AppModel {
    fixture(
        dir,
        &["a/rock.png", AO, "b/stone.png", "b/stone_roughness.png"],
        "",
    )
}

/// `ab` with rock_ao moved out to no set.
fn no_set_ao(dir: &Path) -> AppModel {
    let mut m = ab(dir);
    m.remove_member(A, AO).expect("removed");
    idle(&mut m);
    assert!(m.is_no_set(AO));
    m
}

type H = Harness<'static, (AppModel, State)>;

fn harness(m: AppModel) -> H {
    let h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1600.0, 1000.0))
        .with_step_dt(0.02)
        .build_ui_state(
            |ui, (m, s): &mut (AppModel, State)| sdc_gui::ui::draw_with(ui, m, s),
            (m, State::default()),
        );
    h.ctx.global_style_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
    h
}

fn idle(m: &mut AppModel) {
    assert!(m.wait_idle(Duration::from_secs(60)), "model did not settle");
}

fn choice(item: &str, set: &str, from: &str) -> Pending {
    Pending::Choice {
        item: item.into(),
        set: set.into(),
        from: from.into(),
    }
}

/// Pinned before Step 3 replaces the hand-built button list: a no-set helper's dialog offers Move and Share today,
/// and still does after (A7-1 adds no button). Passes on the stub and after.
#[test]
fn a_no_set_helper_dialog_keeps_move_and_share() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(no_set_ao(d.path()));
    h.state_mut().1.drops.pending = Some(choice(AO, B, "no set"));
    h.run();
    h.run();
    let all = [
        "Move it here",
        "Share (use this file here too, no copy)",
        "Duplicate the source for this set",
    ];
    let offered: Vec<&str> = all
        .into_iter()
        .filter(|l| h.query_by_role_and_label(Role::Button, l).is_some())
        .collect();
    assert_eq!(
        offered,
        ["Move it here", "Share (use this file here too, no copy)"]
    );
}

/// Note JW5 (A7-1): the dialog for a helper from a real set offers Move and Share only.
#[test]
fn the_drop_dialog_offers_a_helper_no_duplicate() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(ab(d.path()));
    h.state_mut().1.drops.pending = Some(choice(AO, B, A));
    h.run();
    h.run();
    h.get_by_role_and_label(Role::Button, "Move it here");
    h.get_by_role_and_label(Role::Button, "Share (use this file here too, no copy)");
    assert!(h
        .query_by_label("Duplicate the source for this set")
        .is_none());
}
