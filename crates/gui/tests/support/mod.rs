#![allow(dead_code)]
//! A folder like a small mod: a diffuse with varying alpha (needs an alpha decision), a normal with no stated green
//! convention, a 3 px diffuse (blocked by DIM-SMALL), a plain diffuse, and roughness + AO that pack `_rmaos`.
use engine::codes::Code;
use engine::config::{AlphaPolicy, GreenConvention, Subtype};
use image::{ImageBuffer, Luma, Rgb, Rgba};
use sdc_gui::model::{Answer, AppModel};
use sdc_gui::settings::GuiSettings;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

pub fn mod_folder(dir: &Path) -> PathBuf {
    let r = dir.join("textures/landscape");
    std::fs::create_dir_all(&r).unwrap();
    let grad = |x: u32, y: u32| [(x * 4) as u8, (y * 4) as u8, 90u8];
    ImageBuffer::from_fn(64, 64, |x, y| Rgb(grad(x, y)))
        .save(r.join("rock01.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| {
        let g = grad(x, y);
        Rgba([g[0], g[1], g[2], if x < 32 { 255 } else { 0 }])
    })
    .save(r.join("leaf.png"))
    .unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([128u8, 128, 255]))
        .save(r.join("rock01_normal.png"))
        .unwrap();
    ImageBuffer::from_fn(3, 3, |x, y| Rgb(grad(x, y)))
        .save(r.join("odd.png"))
        .unwrap();
    for (n, v) in [("rock01_roughness.png", 120u8), ("rock01_ao.png", 200)] {
        ImageBuffer::from_fn(64, 64, |x, _| Luma([v.wrapping_add(x as u8)]))
            .save(r.join(n))
            .unwrap();
    }
    dir.join("textures")
}

/// Rewrites the opened fixture's `rock01_normal.png` with a blue of 0 and plans again, so the folder has one check
/// result (CHK-Z-SIGN on that normal) once it is a tangent-space normal; the fixture's own flat normal has none.
pub fn give_a_check(m: &mut AppModel, dir: &Path) {
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([128u8, 128, 0]))
        .save(dir.join("textures/landscape/rock01_normal.png"))
        .unwrap();
    m.replan();
    assert!(m.wait_idle(Duration::from_secs(60)));
}

/// A session of its own per call: a shared one would drop another test's cache entries on every plan (audit 8).
pub fn session() -> Arc<engine::session::Session> {
    Arc::new(engine::session::Session::new().unwrap())
}

pub fn opened(dir: &Path) -> AppModel {
    let mut m = opened_unchosen(dir);
    choose_export(&mut m, dir.join("export"));
    m
}

/// `opened` before the export folder is chosen: `gui.toml` names one, the project has none (Note FB1), and nothing
/// is written to the project folder.
pub fn opened_unchosen(dir: &Path) -> AppModel {
    let root = mod_folder(dir);
    let settings = GuiSettings {
        export_folder: Some(dir.join("export")),
        ..GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, session());
    // M3-era fixture: High members stay undecided, as before M6 Track E (DECISIONS Note U3).
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(root);
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
    m
}

/// Note FB1: a project's export folder is the one chosen for it (the `gui.toml` value is no longer inherited), so a
/// fixture that exports picks its folder as the user does and waits for the re-plan.
pub fn choose_export(m: &mut AppModel, folder: PathBuf) {
    m.set_export_folder(folder);
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
}

/// Task 5: `opened` with `m.sets` replaced by `n` clones of the first set, each under its own key, members unchanged.
/// The synthetic sets last only until the next plan: any re-plan (a reorder does one) replaces them with the fixture's
/// own sets. After an edit, check `sets_doc.order()`, not `views`.
pub fn many_sets(dir: &Path, n: usize) -> AppModel {
    let mut m = opened(dir);
    let first = m.sets[0].clone();
    m.sets = (0..n)
        .map(|i| {
            let mut s = first.clone();
            s.key = format!("{}{i:04}", first.key);
            s
        })
        .collect();
    m.invalidate_views();
    // A10: a doc-created set or a dedupe must not change N.
    assert_eq!(sdc_gui::model::sets::views(&m).len(), n);
    m
}

/// Note HJ2: `set_output_name` refuses a name on a non-PBR set, but an old project can still hold one. This seeds
/// `output_name` in `sdc-project.toml` as such a project would, and plans again.
pub fn seed_output_name(m: &mut AppModel, id: &str, name: &str) {
    let id = id.to_string();
    let name = name.to_string();
    m.edit_docs("Seed output name", None, true, move |_, d| {
        d.set_output_name(&id, Some(&name))
    });
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
}

/// The M3a fixture with rock01 decided as a TruePBR set: `pbr_albedo` diffuse, `pbr_normal` normal (DirectX green).
/// leaf.png and odd.png stay undecided, as in M3a.
pub fn pbr_opened(dir: &Path) -> AppModel {
    use engine::config::{GreenConvention, Subtype};
    use sdc_gui::model::Answer;
    let mut m = opened(dir);
    m.decide("landscape/rock01.png", Answer::Subtype(Subtype::PbrAlbedo));
    m.decide(
        "landscape/rock01_normal.png",
        Answer::Subtype(Subtype::PbrNormal),
    );
    m.decide(
        "landscape/rock01_normal.png",
        Answer::Green(GreenConvention::Directx),
    );
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
    m
}

/// `pbr_opened` with rock01 picked as plain `pbr`: the fixture lives under `textures/landscape/`, so it is auto PBR
/// Landscape (Note BM); tests of plain-PBR cards, slots and the sphere/mesh preview use this.
pub fn pbr_opened_plain(dir: &Path) -> AppModel {
    let mut m = pbr_opened(dir);
    m.set_shader("landscape/rock01", Some(engine::shader::Shader::Pbr));
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
    m
}

/// A model built as the app builds it (no test overrides), opened on `root` with exports under `export`.
pub fn opened_as_the_app(root: PathBuf, export: PathBuf) -> AppModel {
    let settings = GuiSettings {
        export_folder: Some(export.clone()),
        ..GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, session());
    m.open_folder(root);
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
    choose_export(&mut m, export);
    m
}

/// Note AZ: give the set whose key ends with `key_end` the complex-material shader (under vanilla it then packs the
/// complex `_m`, the old cpm target) and wait for the re-plan.
pub fn complex_material(m: &mut AppModel, key_end: &str) {
    let views = sdc_gui::model::sets::views(m);
    let id = views
        .iter()
        .find(|v| v.key.ends_with(key_end))
        .unwrap_or_else(|| panic!("no set {key_end}"))
        .id
        .clone();
    m.set_shader(&id, Some(engine::shader::Shader::ComplexMaterial));
    assert!(m.wait_idle(Duration::from_secs(60)));
}

pub fn decide_all(m: &mut AppModel) {
    m.decide("landscape/rock01.png", Answer::Subtype(Subtype::Diffuse));
    m.decide("landscape/odd.png", Answer::Subtype(Subtype::Diffuse));
    m.decide("landscape/odd.png", Answer::Acknowledge(Code::DimSmall));
    m.decide("landscape/leaf.png", Answer::Subtype(Subtype::Diffuse));
    m.decide("landscape/leaf.png", Answer::Alpha(AlphaPolicy::Cutout));
    m.decide(
        "landscape/rock01_normal.png",
        Answer::Subtype(Subtype::Normal),
    );
    m.decide(
        "landscape/rock01_normal.png",
        Answer::Green(GreenConvention::Directx),
    );
    assert!(m.wait_idle(Duration::from_secs(60)));
}

/// `pbr_opened`, planned and idle (was pbr_export.rs::ready).
pub fn pbr_ready(dir: &Path) -> AppModel {
    let mut m = pbr_opened(dir);
    assert!(m.wait_idle(Duration::from_secs(60)));
    m
}

/// Note FB1: the header's Export button reads `Export {n} files` or `Review {n} files`; its label as drawn now (the
/// Review panel's own `Export {n} files` button is below the header, so the header's rect picks the right one).
pub fn header_export_label<S>(h: &egui_kittest::Harness<'_, S>) -> String {
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::{NodeT, Queryable};
    let header =
        sdc_gui::ui::panel_ids::panel_rect(&h.ctx, "header").expect("the header was drawn");
    h.query_all_by_role(Role::Button)
        .filter(|n| header.contains_rect(n.rect()))
        .filter_map(|n| n.accesskit_node().label())
        .find(|l| {
            *l == "Export"
                || ((l.starts_with("Export ") || l.starts_with("Review "))
                    && (l.ends_with(" file") || l.ends_with(" files")))
        })
        .expect("the header Export button")
}

/// Panel `name`'s outer rect as egui stored it last frame (`ui::panel_ids`); panics when the panel was never built.
pub fn panel(ctx: &eframe::egui::Context, name: &str) -> eframe::egui::Rect {
    sdc_gui::ui::panel_ids::panel_rect(ctx, name)
        .unwrap_or_else(|| panic!("panel {name} was never built"))
}

/// Whether panel `name` was never built in `ctx`. A test that checks a panel is absent pairs it with a panel the same
/// frame does build, so a renamed or unrecorded panel cannot make the check pass.
pub fn panel_never_built(ctx: &eframe::egui::Context, name: &str) -> bool {
    sdc_gui::ui::panel_ids::panel_id(ctx, name).is_none()
}

/// E28: the header's Undo or Redo (`verb`), found by role and exact name. Its name is `{verb}` or `{verb}: {step}` for
/// the Textures history, `{verb} (NIF edit history)` or `{verb} (NIF edit history): {step}` for the NIF one. On the
/// main screen no other button carries one of these names: the toast's and the Problems row's Undo say more.
/// It uses no rect on purpose: while the command palette is open, its Undo / Redo rows match as well, and the finder
/// then panics with every matching label, so a test can never click a header button hidden under the palette.
pub fn header_undo<'h, S>(
    h: &'h egui_kittest::Harness<'_, S>,
    verb: &str,
) -> egui_kittest::Node<'h> {
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::{NodeT, Queryable};
    let nif = format!("{verb} (NIF edit history)");
    let is = |l: &str| {
        l == verb
            || l.starts_with(&format!("{verb}: "))
            || l == nif
            || l.starts_with(&format!("{nif}: "))
    };
    let mut hits: Vec<_> = h
        .query_all_by_role(Role::Button)
        .filter(|n| n.accesskit_node().label().is_some_and(|l| is(&l)))
        .collect();
    match hits.len() {
        1 => hits.pop().unwrap(),
        0 => {
            let all: Vec<String> = h
                .query_all_by_role(Role::Button)
                .filter_map(|n| n.accesskit_node().label())
                .filter(|l| l.starts_with("Undo") || l.starts_with("Redo"))
                .collect();
            panic!("no header {verb} button; Undo / Redo buttons on screen: {all:?}")
        }
        _ => {
            let labels: Vec<_> = hits
                .iter()
                .filter_map(|n| n.accesskit_node().label())
                .collect();
            panic!("several header {verb} buttons: {labels:?}")
        }
    }
}

/// egui 0.37: the one node of `role` named `name`. A bare label query now also finds the Label beside a named control,
/// or a panel, popup or tooltip of that name; this panics with every node of that name when there is not exactly one.
pub fn widget<'t, 'n, N: egui_kittest::kittest::NodeT<'t> + 't>(
    h: &'n impl egui_kittest::kittest::Queryable<'t, 'n, N>,
    role: eframe::egui::accesskit::Role,
    name: &'t str,
) -> N {
    let mut hits: Vec<N> = h.query_all_by_role_and_label(role, name).collect();
    if hits.len() == 1 {
        return hits.pop().unwrap();
    }
    let all: Vec<String> = h
        .query_all_by_label(name)
        .map(|n| format!("{:?}", n.accesskit_node().role()))
        .collect();
    panic!(
        "{} {role:?} nodes named {name:?}; the roles of every node of that name: {all:?}",
        hits.len()
    )
}

/// egui 0.37 copies an `on_hover_text` into the name (and the description) of a widget that has none, so a name can
/// hold words that are on screen only while the tooltip shows. The nodes whose name contains `text`, leaving out those
/// copies: a shown tooltip's text is a Label of its own and is kept.
pub fn shown_containing<'t, 'n, N: egui_kittest::kittest::NodeT<'t> + 't>(
    h: &'n impl egui_kittest::kittest::Queryable<'t, 'n, N>,
    text: &'t str,
) -> Vec<N> {
    h.query_all(
        egui_kittest::kittest::by()
            .label_contains(text)
            .predicate(|n| n.description().is_none() || n.label() != n.description()),
    )
    .collect()
}

/// Note FM35: open the alpha dropdown that shows `shown` and pick `entry` in it. `place` names the dropdown as
/// `alpha_dropdown` does: `""` (texture panel), `"Problems"` or `"Review"`.
pub fn pick_alpha<S>(
    h: &mut egui_kittest::Harness<'static, S>,
    place: &str,
    shown: &str,
    entry: &str,
) {
    use egui_kittest::kittest::Queryable;
    let name = match place {
        "" => format!("Alpha type: {shown}"),
        p => format!("{p} alpha type: {shown}"),
    };
    h.get_by_label(&name).click();
    h.run();
    h.get_by_label(entry).click();
    h.run();
}

/// Export as a user would: Export, then, if Review opened, press its Export.
pub fn export_through(m: &mut AppModel) {
    m.export_clicked();
    if m.review.is_some() {
        m.export_from_review();
    }
}

/// The PGPatcher JSON an export of `m` writes (was pbr_export.rs::json_path).
pub fn pgp_json_path(m: &AppModel) -> PathBuf {
    m.export_root()
        .join("PBRNIFPatcher")
        .join(format!("{}.json", sdc_gui::model::pbr_export::json_name(m)))
}

/// Layout v2 K5: a slot row's label, the leftmost match (the Matching header repeats "Diffuse" and "Normal _n").
pub fn slot_rect<S>(h: &egui_kittest::Harness<'_, S>, label: &str) -> eframe::egui::Rect {
    use egui_kittest::kittest::Queryable;
    h.query_all_by_label(label)
        .map(|n| n.rect())
        .min_by(|a, b| a.min.x.total_cmp(&b.min.x))
        .unwrap_or_else(|| panic!("no slot row {label:?}"))
}

/// Layout v2 K11: whether `r` is inside the Sets column. Its cards' chips are named by their letter, and "RMAOS" is
/// also the set editor's slot row.
pub fn in_sets_column<S>(h: &egui_kittest::Harness<'_, S>, r: eframe::egui::Rect) -> bool {
    sdc_gui::ui::panel_ids::panel_rect(&h.ctx, "sets").is_some_and(|p| p.contains_rect(r))
}

/// Floating windows on screen: an `egui::Window` is a Middle-order area, and so is a viewport embedded as one (Note
/// FB7: a docked centre view opens none).
pub fn windows<S>(h: &egui_kittest::Harness<'_, S>) -> usize {
    h.ctx.memory(|m| {
        m.areas()
            .visible_layer_ids()
            .iter()
            .filter(|l| l.order == eframe::egui::Order::Middle)
            .count()
    })
}

/// The one node labelled `label` lies inside the centre column (Note FB7: a docked view's controls and body).
pub fn assert_in_centre<S>(h: &egui_kittest::Harness<'_, S>, label: &str) {
    use egui_kittest::kittest::Queryable;
    let r = h.get_by_label(label).rect();
    let c = sdc_gui::ui::centre_rect(&h.ctx).expect("the centre is drawn");
    assert!(
        c.expand(0.5).contains_rect(r),
        "{label} {r:?} is outside the centre {c:?}"
    );
}

/// The one node labelled `label` outside the Sets column (K11).
pub fn outside_sets<'a, S>(
    h: &'a egui_kittest::Harness<'_, S>,
    label: &'a str,
) -> egui_kittest::Node<'a> {
    use egui_kittest::kittest::Queryable;
    let mut n: Vec<_> = h
        .query_all_by_label(label)
        .filter(|n| !in_sets_column(h, n.rect()))
        .collect();
    assert_eq!(n.len(), 1, "one {label:?} outside the Sets column");
    n.pop().unwrap()
}

/// Task 6: a PBR set under `textures/test` (`wall.png` at grey `albedo`, `wall_rmaos.png` filled with `rmaos`),
/// opened as the app opens a folder, shader Pbr.
pub fn pbr_wall(dir: &Path, albedo: u8, rmaos: [u8; 3]) -> AppModel {
    let r = dir.join("textures/test");
    std::fs::create_dir_all(&r).unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([albedo, albedo, albedo]))
        .save(r.join("wall.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb(rmaos))
        .save(r.join("wall_rmaos.png"))
        .unwrap();
    let mut m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    let id = sdc_gui::model::sets::views(&m)
        .iter()
        .find(|v| v.key.ends_with("wall"))
        .expect("the set")
        .id
        .clone();
    m.set_shader(&id, Some(engine::shader::Shader::Pbr));
    assert!(m.wait_idle(Duration::from_secs(60)));
    m
}

/// A 64 px BC1 DDS at `path` (parent folders made), written by the engine's writer, never copied.
/// How many `.dds` files are under `dir`, at any depth (0 for a folder that does not exist): what an export wrote.
pub fn dds_under(dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| e.path())
        .map(|p| {
            if p.is_dir() {
                dds_under(&p)
            } else {
                usize::from(p.extension().is_some_and(|x| x.eq_ignore_ascii_case("dds")))
            }
        })
        .sum()
}

pub fn bc1_dds(path: &Path) {
    use engine::dds::{dds_bytes, DdsDesc, ResolvedHeader};
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let desc = DdsDesc {
        width: 64,
        height: 64,
        format: engine::config::Format::Bc1,
        srgb: false,
        header: ResolvedHeader::Legacy,
    };
    let data = vec![[0x00, 0xF8, 0x00, 0xF8, 0, 0, 0, 0].repeat(256)];
    std::fs::write(path, dds_bytes(&desc, &data).unwrap()).unwrap();
}

/// Note FI2: a released mod's all-DDS PBR sets: `textures/pbr/<s>/<s>.dds`, `<s>_n.dds` and `<s>_rmaos.dds` for each
/// of `sets` under `dir`; returns the `textures` folder.
pub fn dds_pbr_sets(dir: &Path, sets: &[&str]) -> PathBuf {
    for s in sets {
        for suffix in ["", "_n", "_rmaos"] {
            bc1_dds(&dir.join(format!("textures/pbr/{s}/{s}{suffix}.dds")));
        }
    }
    dir.join("textures")
}
