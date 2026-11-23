//! Note JS (N-17, N-18, N-19, N-10): a normal's alpha follows where it is used.
mod support;
use engine::config::{AlphaPolicy, Subtype};
use engine::shader::Shader;
use image::{ImageBuffer, Luma, Rgb, Rgba};
use sdc_gui::model::{Answer, AppModel};
use serde_json::json;
use std::fmt::Write as _;
use std::path::Path;
use std::time::Duration;
use support::*;

const WALL_N: &str = "test/wall_n.png";
const ROCK_N: &str = "landscape/rock_n.png";
const OTHER_WALL_N: &str = "test/wall_n.png.tga";

/// A second input in a separate regular set. Its physical stem is the first normal's full table name.
fn add_wall_collision(dir: &Path, m: &mut AppModel) -> AppModel {
    let root = dir.join("textures");
    m.save().unwrap();
    bumpy_n(&root.join(OTHER_WALL_N));
    std::fs::copy(root.join("test/wall.png"), root.join("test/steel.png")).unwrap();
    let path = root.join("sdc.toml");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str("\n[inputs.'test/wall_n.png.tga']\nas = 'test/steel_n.tga'\n");
    std::fs::write(path, text).unwrap();
    let m = opened_as_the_app(root, dir.join("export"));
    assert_eq!(
        normal_owner(&m, "test/steel").as_deref(),
        Some(OTHER_WALL_N)
    );
    m
}

#[test]
fn a_shader_pick_keeps_the_second_normals_written_alpha() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("normal"));
    let mut m = add_wall_collision(d.path(), &mut m);
    let source = image::open(d.path().join("textures").join(OTHER_WALL_N))
        .unwrap()
        .to_rgba8();
    assert_eq!(
        (source.get_pixel(0, 0)[3], source.get_pixel(63, 63)[3]),
        (0, 252)
    );
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(&mut m);
    m.save().unwrap();
    let job = m.item(OTHER_WALL_N).unwrap().job.as_ref().unwrap();
    assert_eq!(
        job.profile.subtype,
        Subtype::Normal,
        "the other set keeps its normal"
    );
    assert_eq!(job.profile.alpha.policy, AlphaPolicy::Data);
    let out = job.output.clone();
    export_through(&mut m);
    settle(&mut m);
    let bytes = std::fs::read(out).unwrap();
    let f = engine::dds::read_dds(&bytes).unwrap();
    let (w, h, r) = f.levels[0].clone();
    let px = engine::decode::decode_level_rgba(f.format, w, h, &f.data[r]).unwrap();
    assert!(px[0][3] < 0.05 && px.last().unwrap()[3] > 0.95);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("normal"));
}

#[test]
fn the_recorder_and_diffuse_switch_leave_a_colliding_normal_alone() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    let mut m = add_wall_collision(d.path(), &mut m);
    m.decide("test/wall.png", Answer::Subtype(Subtype::PbrAlbedo));
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")), "Switch");
    settle(&mut m);
    add_rmaos(&t, &mut m);
    assert_eq!(
        held(&m, WALL_N, "subtype"),
        Some(json!("normal")),
        "recorder"
    );
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("normal"));
}

#[test]
fn a_diffuse_revert_leaves_a_colliding_listed_normal_alone() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.decide("test/wall.png", Answer::Subtype(Subtype::PbrAlbedo));
    settle(&mut m);
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("pbr_normal"));
    let mut m = add_wall_collision(d.path(), &mut m);
    m.decide("test/wall.png", Answer::Subtype(Subtype::Diffuse));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("pbr_normal")));
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("pbr_normal"));
}

#[test]
fn the_diffuse_switch_checks_the_table_created_by_the_pick() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    std::fs::remove_file(t.join("wall_n.png")).unwrap();
    bumpy_n(&t.join("wall.png.tga"));
    let root = d.path().join("textures");
    std::fs::write(
        root.join("sdc.toml"),
        "[inputs.'test/wall.png.tga']\nas = 'test/wall_n.tga'\n",
    )
    .unwrap();
    let mut m = AppModel::new(Default::default(), None, session());
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(root);
    settle(&mut m);
    assert_eq!(held(&m, "test/wall.png", "subtype"), None);
    m.decide("test/wall.png", Answer::Subtype(Subtype::PbrAlbedo));
    assert_eq!(
        held(&m, "test/wall.png", "subtype"),
        Some(json!("pbr_albedo"))
    );
    assert!(!m
        .sets_doc
        .as_ref()
        .unwrap()
        .auto_pbr()
        .contains(&"test/wall.png.tga".into()));
}

#[test]
fn auto_cannot_clear_the_shared_normal_table() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(&mut m);
    let mut m = add_wall_collision(d.path(), &mut m);
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("pbr_normal"));
    m.set_shader("test/wall", None);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("pbr_normal")));
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("pbr_normal"));
}

#[test]
fn the_first_recording_skips_a_full_key_that_would_also_be_a_stem() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    bumpy_n(&t.join("wall_n.png.tga"));
    let root = d.path().join("textures");
    std::fs::write(
        root.join("sdc.toml"),
        "[inputs.'test/wall_n.png.tga']\nas = 'test/steel_n.tga'\n",
    )
    .unwrap();
    let m = opened_as_the_app(root, d.path().join("export"));
    assert_eq!(held(&m, WALL_N, "subtype"), None);
    assert_eq!(listed(&m, WALL_N), None);
    assert_eq!(held(&m, OTHER_WALL_N, "subtype"), Some(json!("normal")));
}

#[test]
fn the_recorder_checks_the_tables_a_six_face_subtype_would_write() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("textures");
    let dir = root.join("cubemaps");
    std::fs::create_dir_all(&dir).unwrap();
    for face in ["px", "nx", "py", "ny", "pz", "nz"] {
        image::RgbImage::from_pixel(16, 16, Rgb([25, 45, 80]))
            .save(dir.join(format!("chrome_e_{face}.png")))
            .unwrap();
    }
    bumpy_n(&dir.join("chrome_e_nx.png.tga"));
    std::fs::write(
        root.join("sdc.toml"),
        "[inputs.'cubemaps/chrome_e_nx.png.tga']\nas = 'test/steel_n.tga'\n",
    )
    .unwrap();
    let m = opened_as_the_app(root, d.path().join("export"));
    let cube = m
        .item("cubemaps/chrome_e_px.png")
        .unwrap()
        .job
        .as_ref()
        .unwrap();
    assert_eq!(cube.cube.as_ref().unwrap().sources.len(), 6);
    assert_eq!(
        held(&m, "cubemaps/chrome_e_nx.png", "subtype"),
        None,
        "a secondary face would retype the TGA"
    );
    assert_eq!(held(&m, "cubemaps/chrome_e_px.png", "subtype"), None);
}

#[test]
fn an_unwritable_listing_records_the_landscape_suggestion() {
    let d = tempfile::tempdir().unwrap();
    rock(d.path());
    let root = d.path().join("textures");
    std::fs::write(root.join("sdc-project.toml"), "project = 'bad'\n").unwrap();
    let mut m = opened_without_export(d.path());
    assert!(!m.sets_doc.as_ref().unwrap().read_only());
    assert_eq!(held(&m, ROCK_N, "subtype"), Some(json!("landscape_normal")));
    assert_eq!(listed(&m, ROCK_N), None);
    m.save().unwrap();
    assert_eq!(
        sdc_gui::project::ProjectDoc::open(&root).file_value(ROCK_N, "subtype"),
        Some(json!("landscape_normal"))
    );
    m.set_shader("landscape/rock", Some(Shader::Default));
    settle(&mut m);
    assert_eq!(
        m.item(ROCK_N)
            .unwrap()
            .job
            .as_ref()
            .unwrap()
            .profile
            .alpha
            .policy,
        AlphaPolicy::Data
    );
}

#[test]
fn a_presets_new_type_wins_over_a_simultaneous_pbr_follow() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    let key = "test/wall_detailx.png";
    std::fs::rename(t.join("wall_n.png"), t.join("wall_detailx.png")).unwrap();
    let presets = d.path().join("presets");
    std::fs::create_dir(&presets).unwrap();
    let preset = presets.join("custom.toml");
    let alias = |suffix| {
        format!("schema_version = 1\nid = 'custom'\nversion = '1.0.0'\nbase = 'cs-quality'\n[suffix_aliases]\ndetailx = '{suffix}'\n")
    };
    std::fs::write(&preset, alias("n")).unwrap();
    let mut m = AppModel::new(Default::default(), None, session());
    m.user_presets = Some(presets);
    m.open_folder(d.path().join("textures"));
    settle(&mut m);
    assert_eq!(listed(&m, key).as_deref(), Some("normal"));
    std::fs::write(preset, alias("p")).unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([100u8, 0, 255]))
        .save(t.join("wall_rmaos.png"))
        .unwrap();
    m.presets_saved();
    settle(&mut m);
    assert_eq!(
        held(&m, key, "subtype"),
        Some(json!("height")),
        "the preset's write wins"
    );
    let row = sdc_gui::model::bottom::problems(&m);
    let row = row
        .iter()
        .find(|r| r.text.contains("set automatically"))
        .unwrap();
    assert_eq!(
        row.detail
            .as_deref()
            .unwrap()
            .matches("wall_detailx.png")
            .count(),
        1
    );
}

#[test]
fn a_follow_only_recording_does_not_claim_high_confidence() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.accept = engine::policy::AcceptSuggestions::None;
    add_rmaos(&t, &mut m);
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("pbr_normal"));
    let rows = sdc_gui::model::bottom::problems(&m);
    let row = rows
        .iter()
        .find(|r| r.text.contains("set automatically"))
        .unwrap();
    assert_eq!(row.text, "1 type set automatically: wall_n.png");
    let detail = row.detail.as_deref().unwrap();
    assert!(
        detail.contains("wall_n.png") && !detail.contains("sets a type on its own"),
        "{detail}"
    );
}

/// A batch with an alpha write and only a guarded normal follow: the hover has both details and no paragraph on
/// when Tessalith decides (the follow retypes a file already typed; it is not a first automatic type).
#[test]
fn an_alpha_write_beside_a_normal_follow_has_no_confidence_paragraph() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.accept = engine::policy::AcceptSuggestions::None;
    ImageBuffer::from_fn(16, 16, |x, _| Rgba([90u8, 60, 30, (x * 16) as u8]))
        .save(t.join("door.png"))
        .unwrap();
    m.mesh_alpha.insert(
        "textures/test/door".to_string(),
        vec![engine::shader::MeshAlpha {
            blend: false,
            test: true,
            threshold: 90,
        }],
    );
    m.scan_store_unsettled = true;
    add_rmaos(&t, &mut m);
    let rows = sdc_gui::model::bottom::problems(&m);
    let row = rows
        .iter()
        .find(|r| r.text.contains("set automatically"))
        .unwrap();
    let detail = row.detail.as_deref().unwrap();
    assert!(
        detail.contains("wall_n.png: ") && detail.contains("door.png: cutout"),
        "{detail}"
    );
    assert!(!detail.contains("sets a type on its own"), "{detail}");
    assert!(
        !detail.contains("\n\n"),
        "no blank line or paragraph: {detail}"
    );
}

fn skin_msn(dir: &Path, files: &str, borrow: &str) -> AppModel {
    open_slot_files(dir);
    let root = dir.join("textures");
    std::fs::rename(root.join(IRON_N), root.join("pbr/iron_msn.png")).unwrap();
    open_saved_share(
        dir,
        files,
        &format!("[[sets]]\nid = 'pbr/iron'\nshader = 'advanced_skin'\n{borrow}"),
    )
}

#[test]
fn sharing_a_rule_typed_msn_never_writes_data() {
    let d = tempfile::tempdir().unwrap();
    let key = "pbr/iron_msn.png";
    let mut m = skin_msn(
        d.path(),
        "[[rules]]\nmatch.glob = '**/iron_msn.png'\nsubtype = 'msn'\n",
        "",
    );
    assert_eq!(held(&m, key, "subtype"), None);
    assert_eq!(
        m.item(key).unwrap().job.as_ref().unwrap().profile.subtype,
        Subtype::Msn
    );
    share(&mut m, key, "armor/steel");
    assert_eq!(held(&m, key, "alpha.policy"), None);
    assert!(auto_data(&m).is_empty());
}

#[test]
fn a_saved_msn_borrow_never_offers_keep_alpha() {
    let d = tempfile::tempdir().unwrap();
    let key = "pbr/iron_msn.png";
    let mut m = skin_msn(
        d.path(),
        "[files.'pbr/iron_msn.png']\nsubtype = 'msn'\n",
        STEEL_BORROWS,
    );
    assert_eq!(
        m.item(key).unwrap().job.as_ref().unwrap().profile.subtype,
        Subtype::Msn
    );
    assert_eq!(
        sdc_gui::model::sets::borrowed_normal(&m)["armor/steel"],
        "pbr/iron"
    );
    assert_eq!(keep_rows(&m, key), 0);
    assert!(m.keep_alpha(key).is_err());
    assert_eq!(held(&m, key, "alpha.policy"), None);
}

#[test]
fn ending_an_old_msn_share_leaves_its_saved_data_alone() {
    use sdc_gui::model::sets::SlotCommand;
    let d = tempfile::tempdir().unwrap();
    let key = "pbr/iron_msn.png";
    let mut m = skin_msn(
        d.path(),
        "[[rules]]\nmatch.glob = '**/iron_msn.png'\nsubtype = 'msn'\n\
         [files.'pbr/iron_msn.png']\nalpha.policy = 'data'\n",
        &format!("[project]\nauto_data = ['{key}']\n{STEEL_BORROWS}"),
    );
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    assert_eq!(held(&m, key, "alpha.policy"), Some(json!("data")));
    assert_eq!(auto_data(&m), [key]);
}

/// `textures/landscape/rock.png`, `rock_n.png` and `rock_rmaos.png`: an automatic PBR Landscape set.
fn rock(dir: &Path) {
    let t = dir.join("textures/landscape");
    std::fs::create_dir_all(&t).unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 60u8]))
        .save(t.join("rock.png"))
        .unwrap();
    bumpy_n(&t.join("rock_n.png"));
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([100u8, 0, 255]))
        .save(t.join("rock_rmaos.png"))
        .unwrap();
}

fn add_rmaos(t: &Path, m: &mut AppModel) {
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([100u8, 0, 255]))
        .save(t.join("wall_rmaos.png"))
        .unwrap();
    // A plan reads the folder again (the engine discovers the inputs each plan).
    m.replan();
    settle(m);
}

/// An `_n` like `normal`, with a specular alpha that changes per texel.
fn bumpy_n(path: &Path) {
    ImageBuffer::from_fn(64, 64, |x, y| {
        Rgba([
            if x % 2 == 0 { 204u8 } else { 51 },
            128,
            230,
            (x * 3 + y) as u8,
        ])
    })
    .save(path)
    .unwrap();
}

/// `textures/test/wall.png` and `wall_n.png`; returns the folder.
fn wall(dir: &Path) -> std::path::PathBuf {
    let t = dir.join("textures/test");
    std::fs::create_dir_all(&t).unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
        .save(t.join("wall.png"))
        .unwrap();
    bumpy_n(&t.join("wall_n.png"));
    t
}

fn listed(m: &AppModel, key: &str) -> Option<String> {
    m.sets_doc.as_ref().unwrap().auto_normal().remove(key)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ClaimStart {
    Recording,
    Saved,
    Pruned,
    FuzzUndo,
    Unreadable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Gesture {
    Pick,
    Switch,
    KeepAlpha,
    Reset,
}

#[track_caller]
fn assert_claim_prompt(m: &mut AppModel, files: bool) {
    let recover_sets = files && m.sets_doc.as_ref().unwrap().is_dirty();
    check_close(m, (files, false), (files, recover_sets));
}

fn exercise_claim(start: ClaimStart, gesture: Gesture) {
    use ClaimStart::*;
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("textures");
    let t = wall(d.path());
    if gesture == Gesture::KeepAlpha {
        std::fs::copy(t.join("wall.png"), t.join("steel.png")).unwrap();
        std::fs::write(
            root.join("sdc-project.toml"),
            "[[sets]]\nid = 'test/steel'\nborrow = {slot2 = 'test/wall'}\n",
        )
        .unwrap();
    }
    let mut sets = sdc_gui::sets_doc::SetsDoc::open(&root);
    sets.set_export_folder(&d.path().join("export"));
    sets.save(false).unwrap();
    if start == Unreadable {
        std::fs::write(root.join("sdc-project.toml"), "[project\n").unwrap();
    } else if start == FuzzUndo {
        std::fs::remove_file(t.join("wall_n.png")).unwrap();
        std::fs::copy(t.join("wall.png"), t.join("rock01.png")).unwrap();
        for name in ["rock01_roughness.png", "rock01_fuzz.png"] {
            ImageBuffer::from_fn(64, 64, |x, _| Luma([60 + x as u8]))
                .save(t.join(name))
                .unwrap();
        }
        std::fs::write(
            root.join("sdc.toml"),
            "[files.'test/rock01.png']\nsubtype = 'skin_albedo'\n",
        )
        .unwrap();
    }
    let mut m = opened_without_export(d.path());
    assert_claim_prompt(&mut m, false);
    if start == FuzzUndo {
        assert!(m.fuzz_note_for_test().is_some());
        m.undo();
        settle(&mut m);
        assert_claim_prompt(&mut m, true);
        bumpy_n(&t.join("wall_n.png"));
        m.replan();
        settle(&mut m);
    }
    let initial = listed(&m, WALL_N);
    assert_eq!(
        initial.as_deref(),
        (start != Unreadable).then_some("normal")
    );
    assert_claim_prompt(&mut m, start == FuzzUndo);
    if gesture == Gesture::KeepAlpha && start != Pruned {
        add_rmaos(&t, &mut m);
        assert_claim_prompt(&mut m, start == FuzzUndo);
    }
    if matches!(start, Saved | Pruned) {
        m.save().unwrap();
        assert_claim_prompt(&mut m, false);
    }
    if start == Pruned {
        let word = if gesture == Gesture::KeepAlpha {
            Subtype::PbrNormal
        } else {
            Subtype::Msn
        };
        m.decide(WALL_N, Answer::Subtype(word));
        settle(&mut m);
        assert_claim_prompt(&mut m, true);
        bumpy_n(&t.join("other_n.png"));
        m.replan();
        settle(&mut m);
        assert_eq!(listed(&m, WALL_N), None);
        assert_eq!(listed(&m, "test/other_n.png").as_deref(), Some("normal"));
        assert_claim_prompt(&mut m, true);
    }
    match gesture {
        Gesture::Pick => m.decide(WALL_N, Answer::Subtype(Subtype::Normal)),
        Gesture::Switch => m.decide("test/wall.png", Answer::Subtype(Subtype::PbrAlbedo)),
        Gesture::Reset => {
            m.reset_all_settings(&[WALL_N.to_owned()]);
            assert_eq!(held(&m, WALL_N, "subtype"), None);
        }
        Gesture::KeepAlpha => {
            if start == Pruned {
                add_rmaos(&t, &mut m);
            }
            assert_claim_prompt(&mut m, matches!(start, Pruned | FuzzUndo));
            assert_eq!(
                keep_rows(&m, WALL_N),
                1,
                "{start:?}: Keep alpha must be available"
            );
            m.keep_alpha(WALL_N).unwrap();
        }
    }
    settle(&mut m);
    let sets_claimed = match gesture {
        Gesture::Pick => start == Saved,
        Gesture::Switch => !matches!(start, Pruned | Unreadable),
        // Reset of the pruned MSN gives A back to the recorder, changing its ownership again.
        Gesture::Reset => start == Pruned,
        Gesture::KeepAlpha => false,
    };
    let recover_sets = m.sets_doc.as_ref().unwrap().is_dirty();
    assert_eq!(
        m.sets_doc.as_ref().unwrap().user_dirty(),
        sets_claimed,
        "{start:?} {gesture:?}"
    );
    check_close(&mut m, (true, sets_claimed), (true, recover_sets));
    m.save().unwrap();
    assert_claim_prompt(&mut m, false);
    let mut reopened = opened_without_export(d.path());
    assert_claim_prompt(&mut reopened, false);
    // A second PBR signal for Keep alpha, whose owner already had the packed map.
    if gesture == Gesture::KeepAlpha {
        std::fs::copy(t.join("wall.png"), t.join("wall_roughness.png")).unwrap();
    }
    add_rmaos(&t, &mut reopened);
    let owner = sdc_gui::model::sets::views(&reopened);
    let shader = owner
        .iter()
        .find(|v| v.id == "test/wall")
        .unwrap()
        .shader
        .as_ref()
        .unwrap();
    assert!(
        shader.shader.is_pbr_or_advanced_skin(),
        "{start:?} {gesture:?}: {shader:?}"
    );
    match gesture {
        Gesture::Pick => {
            assert_eq!(held(&reopened, WALL_N, "subtype"), Some(json!("normal")));
            assert_eq!(listed(&reopened, WALL_N), None);
        }
        Gesture::Switch => {
            assert_eq!(
                held(&reopened, "test/wall.png", "subtype"),
                Some(json!("pbr_albedo"))
            );
            let word = match start {
                Pruned => "msn",
                Unreadable => "normal",
                _ => "pbr_normal",
            };
            assert_eq!(held(&reopened, WALL_N, "subtype"), Some(json!(word)));
            assert_eq!(
                listed(&reopened, WALL_N).as_deref(),
                (!matches!(start, Pruned | Unreadable)).then_some(word)
            );
        }
        Gesture::KeepAlpha => {
            assert_eq!(held(&reopened, WALL_N, "alpha.policy"), Some(json!("data")));
            assert!(!auto_data(&reopened).iter().any(|k| k == WALL_N));
        }
        // Reset returns control to the recorder. New evidence can change its choice.
        Gesture::Reset => {
            let word = if start == Unreadable {
                "normal"
            } else {
                "pbr_normal"
            };
            assert_eq!(held(&reopened, WALL_N, "subtype"), Some(json!(word)));
            assert_eq!(
                listed(&reopened, WALL_N).as_deref(),
                (start != Unreadable).then_some(word)
            );
        }
    }
    assert_claim_prompt(&mut reopened, false);
}

#[test]
fn pick_switch_and_reset_from_each_claim_start() {
    use ClaimStart::*;
    for start in [Recording, Saved, Pruned, FuzzUndo, Unreadable] {
        for gesture in [Gesture::Pick, Gesture::Switch, Gesture::Reset] {
            exercise_claim(start, gesture);
        }
    }
}

#[test]
fn keep_alpha_from_each_readable_claim_start() {
    use ClaimStart::*;
    for start in [Recording, Saved, Pruned, FuzzUndo] {
        exercise_claim(start, Gesture::KeepAlpha);
    }
}

#[test]
fn keep_alpha_with_an_unreadable_sets_document() {
    let d = tempfile::tempdir().unwrap();
    let shared = old_share(d.path());
    assert_eq!(keep_rows(&shared, IRON_N), 1);
    drop(shared);

    std::fs::write(d.path().join("textures/sdc-project.toml"), "[project\n").unwrap();
    let mut m = opened_without_export(d.path());
    let files = m.project.as_ref().unwrap().text();
    let sets = m.sets_doc.as_ref().unwrap().text();
    assert_eq!(keep_rows(&m, IRON_N), 0);
    assert!(m.sets_doc.as_ref().unwrap().read_only());
    assert!(m.keep_alpha(IRON_N).is_err());
    assert_eq!(m.project.as_ref().unwrap().text(), files);
    assert_eq!(m.sets_doc.as_ref().unwrap().text(), sets);
    assert_claim_prompt(&mut m, false);
}

fn held(m: &AppModel, key: &str, field: &str) -> Option<serde_json::Value> {
    m.project.as_ref().unwrap().file_value(key, field)
}

fn settle(m: &mut AppModel) {
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
}

/// The folder opened as the app opens it, with no export folder chosen yet (so nothing is saved for it).
fn opened_without_export(dir: &Path) -> AppModel {
    let mut m = AppModel::new(sdc_gui::settings::GuiSettings::default(), None, session());
    m.open_folder(dir.join("textures"));
    settle(&mut m);
    m
}

/// N-26 (FI4): the listing alone, on a folder opened with no export folder chosen yet, asks nothing and leaves no
/// recovery text; choosing the export folder saves it with `sdc-project.toml`.
#[test]
fn the_listing_alone_is_an_automatic_change() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_without_export(d.path());
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("normal"));
    assert!(
        m.sets_doc.as_ref().unwrap().is_dirty(),
        "the listing is unsaved"
    );
    assert!(m.is_dirty() && !m.unsaved());
    assert!(!m.unsaved());
    assert!(
        m.recovery_snapshot().is_empty(),
        "{:?}",
        m.recovery_snapshot()
    );
    m.set_export_folder(d.path().join("export"));
    settle(&mut m);
    assert!(!m.sets_doc.as_ref().unwrap().is_dirty());
    let saved = std::fs::read_to_string(d.path().join("textures/sdc-project.toml")).unwrap();
    assert!(saved.contains("auto_normal"), "{saved}");
    assert!(m.is_dirty() && !m.unsaved());
}

/// N-26, review 2 Important 1, review 3 Minor 2: the listing snapshot ends at a save. With no export folder chosen
/// (so the Undo returns to the very text the snapshot holds): Save, a user edit to `sdc-project.toml`, Save, Undo. The
/// document is back at the text the recording left, but that is now the user's unsaved change: it is asked about,
/// named in the dialog's file list (`files_hint` reads `SetsDoc::user_dirty`), and no save-if-clean site writes it.
#[test]
fn a_save_ends_the_listing_snapshot() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_without_export(d.path());
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("normal"));
    assert!(
        m.sets_doc
            .as_ref()
            .is_some_and(|s| s.is_dirty() && !s.defer_record_save()),
        "precondition: the snapshot holds the listing's text"
    );
    assert!(m.save().is_ok());
    m.set_shader("test/wall", Some(Shader::Default));
    settle(&mut m);
    assert!(m.save().is_ok());
    let file = d.path().join("textures/sdc-project.toml");
    let on_disk = std::fs::read_to_string(&file).unwrap();
    m.undo();
    settle(&mut m);
    assert!(m.sets_doc.as_ref().unwrap().is_dirty());
    assert!(!m
        .sets_doc
        .as_ref()
        .is_some_and(|s| s.is_dirty() && !s.defer_record_save()));
    assert!(
        m.sets_doc.as_ref().unwrap().user_dirty(),
        "the dialog names sdc-project.toml"
    );
    assert!(m.unsaved());
    assert!(!m.is_dirty() || m.unsaved());
    m.set_export_folder(d.path().join("export"));
    settle(&mut m);
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        on_disk,
        "no save-if-clean write"
    );
}

fn failed_listing_save(which: sdc_gui::model::DocKind) {
    use sdc_gui::model::{Dialog, DocKind};
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_without_export(d.path());
    assert!(
        m.is_dirty()
            && !m.unsaved()
            && m.sets_doc
                .as_ref()
                .is_some_and(|s| s.is_dirty() && !s.defer_record_save())
    );
    let path = d.path().join("textures").join(which.file());
    std::fs::write(&path, "# edited outside Tessalith\n").unwrap();
    assert_eq!(m.save(), Err(sdc_gui::project::SaveError::ChangedOnDisk));
    assert!(matches!(m.dialog, Some(Dialog::ExternalChange(kind)) if kind == which));
    assert_eq!(
        m.project.as_ref().unwrap().is_dirty(),
        which == DocKind::SdcToml
    );
    assert!(
        m.sets_doc
            .as_ref()
            .is_some_and(|s| s.is_dirty() && !s.defer_record_save()),
        "the failed Save still owns the listing"
    );
    assert!(!m.sets_doc.as_ref().unwrap().user_dirty() && !m.unsaved());
    assert!(m.recovery_snapshot().is_empty());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "# edited outside Tessalith\n"
    );
    m.resolve_external_change(which, true);
    if which == DocKind::SdcToml {
        assert!(
            m.sets_doc
                .as_ref()
                .is_some_and(|s| s.is_dirty() && !s.defer_record_save()),
            "only the files document has been saved"
        );
    }
    m.save().unwrap();
    assert!(!m.is_dirty());
    assert_eq!(
        sdc_gui::sets_doc::SetsDoc::open(&d.path().join("textures"))
            .auto_normal()
            .get(WALL_N)
            .map(String::as_str),
        Some("normal")
    );
}

#[test]
fn a_failed_save_keeps_the_listing_when_the_files_document_conflicts() {
    failed_listing_save(sdc_gui::model::DocKind::SdcToml);
}

#[test]
fn a_failed_save_keeps_the_listing_when_the_sets_document_conflicts() {
    failed_listing_save(sdc_gui::model::DocKind::Project);
}

#[test]
fn a_failed_second_write_keeps_the_type_claim_until_save_succeeds() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_without_export(d.path());
    m.save().unwrap();
    m.decide(WALL_N, Answer::Subtype(Subtype::Normal));
    settle(&mut m);
    let path = m.sets_doc.as_ref().unwrap().path().to_owned();
    std::fs::write(&path, "# external edit\n").unwrap();
    assert_eq!(m.save(), Err(sdc_gui::project::SaveError::ChangedOnDisk));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "# external edit\n");
    check_close(&mut m, (true, true), (true, true));
    m.resolve_external_change(sdc_gui::model::DocKind::Project, true);
    m.save().unwrap();
    check_close(&mut m, (false, false), (false, false));
}

#[test]
fn overwriting_a_conflicting_files_document_still_requires_saving_the_pruned_listing() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    let mut m = opened_without_export(d.path());
    m.save().unwrap();
    m.decide(WALL_N, Answer::Subtype(Subtype::Msn));
    settle(&mut m);
    bumpy_n(&t.join("other_n.png"));
    m.replan();
    settle(&mut m);
    m.decide(WALL_N, Answer::Subtype(Subtype::Normal));
    settle(&mut m);
    std::fs::write(m.project.as_ref().unwrap().path(), "# external edit\n").unwrap();
    assert_eq!(m.save(), Err(sdc_gui::project::SaveError::ChangedOnDisk));
    m.resolve_external_change(sdc_gui::model::DocKind::SdcToml, true);
    check_close(&mut m, (true, false), (true, true));
    m.save().unwrap();
    check_close(&mut m, (false, false), (false, false));
    let mut reopened = opened_without_export(d.path());
    add_rmaos(&t, &mut reopened);
    assert_eq!(held(&reopened, WALL_N, "subtype"), Some(json!("normal")));
}

#[test]
fn a_files_only_edit_recovers_the_recorded_type_with_its_listing() {
    use egui_kittest::kittest::Queryable;
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_without_export(d.path());
    assert!(
        m.sets_doc
            .as_ref()
            .is_some_and(|s| s.is_dirty() && !s.defer_record_save())
            && m.recovery_snapshot().is_empty()
    );
    m.set_field("test/wall.png", "alpha.policy", json!("data"));
    settle(&mut m);
    assert!(m.project.as_ref().unwrap().is_dirty() && m.unsaved());
    assert!(
        !m.sets_doc.as_ref().unwrap().user_dirty(),
        "the close dialog names only sdc.toml"
    );
    let files = m.recovery_snapshot();
    let p = m.project.as_ref().unwrap();
    let s = m.sets_doc.as_ref().unwrap();
    assert_eq!(
        files,
        [
            (p.path().to_owned(), p.text()),
            (s.path().to_owned(), s.text())
        ]
    );
    assert!(!m.request_close());
    let dialog = keep_harness(m);
    assert!(dialog.query_by_label("sdc.toml").is_some());
    assert!(dialog
        .query_by_label("sdc.toml, sdc-project.toml")
        .is_none());
    for (path, text) in files {
        std::fs::write(sdc_gui::crash_log::recovery_path(&path), text).unwrap();
    }
    let restored = opened_without_export(d.path());
    assert_eq!(restored.undo_label(), Some("Restore crash edits"));
    assert_eq!(
        held(&restored, "test/wall.png", "alpha.policy"),
        Some(json!("data"))
    );
    assert_eq!(restored.untouched_normal(WALL_N), Some(Subtype::Normal));
}

/// N-27: a user's sub-type edit of a listed normal to another value leaves `sdc-project.toml` as it was (the fast
/// partial plan is kept); the key no longer counts as untouched.
#[test]
fn a_users_edit_of_a_listed_normal_is_files_only() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("normal"));
    assert!(
        !m.sets_doc.as_ref().unwrap().is_dirty(),
        "saved with the export folder"
    );
    let rev = m.sets_doc.as_ref().unwrap().rev();
    m.set_field(WALL_N, "subtype", json!("pbr_normal"));
    settle(&mut m);
    assert_eq!(
        m.sets_doc.as_ref().unwrap().rev(),
        rev,
        "no write to sdc-project.toml"
    );
    assert_eq!(
        listed(&m, WALL_N).as_deref(),
        Some("normal"),
        "stale, not untouched"
    );
    assert_eq!(m.untouched_normal(WALL_N), None);
}

/// N-27: typing the listed word itself makes the value the user's: the key leaves the list.
#[test]
fn typing_the_listed_word_takes_the_key_off() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("normal"));
    assert_eq!(m.untouched_normal(WALL_N), Some(Subtype::Normal));
    m.decide(WALL_N, Answer::Subtype(Subtype::Normal));
    settle(&mut m);
    assert_eq!(listed(&m, WALL_N), None);
    assert_eq!(m.untouched_normal(WALL_N), None);
}

#[test]
fn claiming_a_new_recording_prompts_recovers_and_reopens_user_owned() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    std::fs::remove_file(t.join("wall_n.png")).unwrap();
    let mut m = opened_without_export(d.path());
    m.set_export_folder(d.path().join("export"));
    settle(&mut m);
    m.save().unwrap();
    bumpy_n(&t.join("wall_n.png"));
    m.replan();
    settle(&mut m);
    assert_eq!(m.untouched_normal(WALL_N), Some(Subtype::Normal));
    assert!(!m.can_undo());
    check_close(&mut m, (false, false), (false, false));
    m.decide(WALL_N, Answer::Subtype(Subtype::Normal));
    settle(&mut m);
    assert_eq!(listed(&m, WALL_N), None);
    assert!(
        !m.request_close(),
        "close must offer to save the claimed type"
    );
    let recovery = m.recovery_snapshot();
    assert!(recovery
        .iter()
        .any(|(p, text)| p.ends_with("sdc.toml") && text == &m.project.as_ref().unwrap().text()));
    for (path, text) in recovery {
        std::fs::write(sdc_gui::crash_log::recovery_path(&path), text).unwrap();
    }
    check_close(&mut m, (true, false), (true, true));
    m.undo();
    settle(&mut m);
    assert!(!m.can_undo());
    assert_eq!(m.untouched_normal(WALL_N), Some(Subtype::Normal));
    check_close(&mut m, (true, false), (true, true));
    m.redo();
    settle(&mut m);
    assert!(!m.can_redo());
    assert_eq!(listed(&m, WALL_N), None);
    check_close(&mut m, (true, false), (true, false));
    let mut restored = opened_without_export(d.path());
    assert_eq!(restored.undo_label(), Some("Restore crash edits"));
    assert_eq!(held(&restored, WALL_N, "subtype"), Some(json!("normal")));
    assert_eq!(listed(&restored, WALL_N), None);
    check_close(&mut restored, (true, false), (true, false));
    restored.undo_crash_restore();
    assert!(!restored.can_undo());
    check_close(&mut restored, (true, false), (true, false));
    assert!(!sdc_gui::crash_log::recovery_path(restored.project.as_ref().unwrap().path()).exists());
    settle(&mut restored);
    assert_eq!(restored.untouched_normal(WALL_N), Some(Subtype::Normal));
    check_close(&mut restored, (true, false), (true, true));
    restored.redo();
    settle(&mut restored);
    assert!(!restored.can_redo());
    assert_eq!(held(&restored, WALL_N, "subtype"), Some(json!("normal")));
    assert_eq!(listed(&restored, WALL_N), None);
    check_close(&mut restored, (true, false), (true, false));
    // The folder picker can save the sets document independently. The next equal edit still sees the claim.
    restored.set_export_folder(d.path().join("new-export"));
    settle(&mut restored);
    restored.decide(WALL_N, Answer::Subtype(Subtype::Normal));
    settle(&mut restored);
    check_close(&mut restored, (true, false), (true, false));
    restored.save().unwrap();
    check_close(&mut restored, (false, false), (false, false));
    let mut reopened = opened_without_export(d.path());
    add_rmaos(&t, &mut reopened);
    assert_eq!(held(&reopened, WALL_N, "subtype"), Some(json!("normal")));
    assert_eq!(listed(&reopened, WALL_N), None);
    check_close(&mut reopened, (false, false), (false, false));
}

/// G1: the recorder wrote `normal` while the set was regular, and listed it; an `_rmaos` arrives and the set is PBR by
/// evidence, so the listed normal becomes `pbr_normal` and its alpha is dropped silently, in the recorder's own step.
#[test]
fn a_recorded_normal_follows_its_set_into_pbr() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(
        held(&m, WALL_N, "subtype"),
        Some(json!("normal")),
        "FC1 recorded it"
    );
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("normal"));
    add_rmaos(&t, &mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("pbr_normal")));
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("pbr_normal"));
    let j = m.item(WALL_N).and_then(|i| i.job.clone()).expect("job");
    assert_eq!(j.profile.subtype, Subtype::PbrNormal);
    assert_eq!(j.profile.alpha.policy, AlphaPolicy::Remove);
    assert!(!j
        .warnings
        .iter()
        .any(|w| w.code == engine::codes::Code::LossAlphaData));
    assert!(
        m.undo_label().is_some_and(|l| l.ends_with("automatically")),
        "{:?}",
        m.undo_label()
    );
}

fn unlock_and_follow(dir: &Path) -> AppModel {
    let t = wall(dir);
    let mut m = opened_without_export(dir);
    check_close(&mut m, (false, false), (false, false));
    m.set_slot_lock("test/wall", 2, true).unwrap();
    settle(&mut m);
    check_close(&mut m, (false, true), (true, true));
    add_rmaos(&t, &mut m);
    check_close(&mut m, (false, true), (true, true));
    m.save().unwrap();
    check_close(&mut m, (false, false), (false, false));
    assert!(m.slot_locked("test/wall", 2));
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("normal"));
    m.set_slot_lock("test/wall", 2, false).unwrap();
    settle(&mut m);
    assert_eq!(m.untouched_normal(WALL_N), Some(Subtype::PbrNormal));
    m
}

#[test]
fn an_unlocked_normal_follow_leaves_the_locked_history_intact() {
    let d = tempfile::tempdir().unwrap();
    let mut m = unlock_and_follow(d.path());
    check_close(&mut m, (false, true), (true, true));
    record_another_colour(&mut m);
    check_close(&mut m, (false, true), (true, true));
    m.undo();
    settle(&mut m);
    assert!(!m.slot_locked("test/wall", 2));
    assert_eq!(m.untouched_normal(WALL_N), Some(Subtype::Normal));
    check_close(&mut m, (true, true), (true, true));
    m.undo();
    settle(&mut m);
    assert!(m.slot_locked("test/wall", 2));
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("normal"));
    check_close(&mut m, (true, false), (true, false));
    undo_lock_and_redo_follow(&mut m);
}

#[test]
fn an_unlocked_normal_follows_with_undo_in_problems() {
    use egui_kittest::kittest::Queryable;
    use sdc_gui::model::bottom::{problems, Fix};
    let d = tempfile::tempdir().unwrap();
    let mut m = unlock_and_follow(d.path());
    let rows = problems(&m);
    let follow = rows
        .iter()
        .find(|r| r.text.contains("set automatically"))
        .unwrap();
    assert_eq!(follow.fix, Some(Fix::UndoLast));
    check_close(&mut m, (false, true), (true, true));
    record_another_colour(&mut m);
    check_close(&mut m, (false, true), (true, true));
    let rows = problems(&m);
    let replacement = rows
        .iter()
        .find(|r| r.text.contains("set automatically"))
        .unwrap();
    assert!(replacement.detail.as_ref().unwrap().contains("extra_d.png"));
    assert_eq!(
        replacement.fix,
        Some(Fix::UndoLast),
        "the follow is still on top"
    );
    m.open_problems(None);
    let mut h = keep_harness(m);
    h.get_by_label("Undo this change").click();
    h.step();
    settle(h.state_mut());
    assert_eq!(h.state().untouched_normal(WALL_N), Some(Subtype::Normal));
    assert!(!problems(h.state()).iter().any(|r| r.text == follow.text));
    assert!(!problems(h.state())
        .iter()
        .any(|r| r.text == replacement.text));
    check_close(h.state_mut(), (true, true), (true, true));
    h.state_mut().undo();
    settle(h.state_mut());
    assert!(h.state().slot_locked("test/wall", 2));
    check_close(h.state_mut(), (true, false), (true, false));
    undo_lock_and_redo_follow(h.state_mut());
}

fn undo_lock_and_redo_follow(m: &mut AppModel) {
    m.undo();
    settle(m);
    assert!(!m.can_undo());
    assert!(!m.slot_locked("test/wall", 2));
    check_close(m, (true, true), (true, true));
    m.redo();
    settle(m);
    assert!(m.slot_locked("test/wall", 2));
    check_close(m, (true, false), (true, false));
    m.redo();
    settle(m);
    assert!(!m.slot_locked("test/wall", 2));
    check_close(m, (true, true), (true, true));
    m.redo();
    settle(m);
    assert!(!m.can_redo());
    assert_eq!(m.untouched_normal(WALL_N), Some(Subtype::PbrNormal));
    check_close(m, (true, true), (true, true));
}

#[test]
fn an_automatic_prune_allows_the_export_folder_to_save() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    std::fs::remove_file(t.join("wall_n.png")).unwrap();
    let mut m = opened_without_export(d.path());
    m.save().unwrap();
    bumpy_n(&t.join("wall_n.png"));
    m.replan();
    settle(&mut m);
    assert_eq!(m.untouched_normal(WALL_N), Some(Subtype::Normal));
    check_close(&mut m, (false, false), (false, false));
    m.decide(WALL_N, Answer::Subtype(Subtype::Msn));
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("normal"));
    settle(&mut m);
    check_close(&mut m, (true, false), (true, true));
    bumpy_n(&t.join("other_n.png"));
    m.replan();
    settle(&mut m);
    let b = "test/other_n.png";
    assert_eq!(m.untouched_normal(b), Some(Subtype::Normal));
    assert_eq!(listed(&m, WALL_N), None);
    check_close(&mut m, (true, false), (true, true));
    m.undo();
    settle(&mut m);
    assert!(!m.can_undo());
    assert_eq!(m.untouched_normal(WALL_N), Some(Subtype::Normal));
    assert_eq!(m.untouched_normal(b), Some(Subtype::Normal));
    check_close(&mut m, (true, false), (true, true));
    m.redo();
    settle(&mut m);
    assert!(!m.can_redo());
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("msn")));
    assert_eq!(listed(&m, WALL_N), None);
    assert_eq!(m.untouched_normal(b), Some(Subtype::Normal));
    check_close(&mut m, (true, false), (true, true));
    let folder = d.path().join("new-export");
    m.set_export_folder(folder.clone());
    settle(&mut m);
    let sets = m.sets_doc.as_ref().unwrap();
    assert_eq!(sets.export_folder(), Some(folder));
    assert_eq!(
        sets.auto_normal(),
        [(b.to_owned(), "normal".to_owned())].into()
    );
    assert_eq!(std::fs::read_to_string(sets.path()).unwrap(), sets.text());
    check_close(&mut m, (true, false), (true, false));

    m.decide(WALL_N, Answer::Subtype(Subtype::Normal));
    settle(&mut m);
    assert_eq!(listed(&m, WALL_N), None);
    check_close(&mut m, (true, false), (true, false));
    m.save().unwrap();
    check_close(&mut m, (false, false), (false, false));
    m.undo();
    settle(&mut m);
    check_close(&mut m, (true, false), (true, false));
    m.undo();
    settle(&mut m);
    assert!(!m.can_undo());
    assert_eq!(m.untouched_normal(WALL_N), Some(Subtype::Normal));
    check_close(&mut m, (true, true), (true, true));
    m.redo();
    settle(&mut m);
    check_close(&mut m, (true, false), (true, false));
    m.redo();
    settle(&mut m);
    assert!(!m.can_redo());
    assert_eq!(listed(&m, WALL_N), None);
    check_close(&mut m, (true, false), (true, false));
}

#[test]
fn picking_normal_after_a_recording_prunes_the_saved_listing_prompts() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    let mut m = opened_without_export(d.path());
    m.save().unwrap();
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("normal"));
    m.decide(WALL_N, Answer::Subtype(Subtype::Msn));
    settle(&mut m);
    check_close(&mut m, (true, false), (true, false));
    bumpy_n(&t.join("other_n.png"));
    m.replan();
    settle(&mut m);
    assert_eq!(listed(&m, WALL_N), None);
    assert_eq!(listed(&m, "test/other_n.png").as_deref(), Some("normal"));
    check_close(&mut m, (true, false), (true, true));
    m.decide(WALL_N, Answer::Subtype(Subtype::Normal));
    settle(&mut m);
    check_close(&mut m, (true, false), (true, true));
    m.save().unwrap();
    check_close(&mut m, (false, false), (false, false));
    let mut reopened = opened_without_export(d.path());
    assert_eq!(listed(&reopened, WALL_N), None);
    check_close(&mut reopened, (false, false), (false, false));
    add_rmaos(&t, &mut reopened);
    assert_eq!(held(&reopened, WALL_N, "subtype"), Some(json!("normal")));
    check_close(&mut reopened, (false, false), (false, false));
}

#[test]
fn saving_a_claim_persists_the_automatically_pruned_listing() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    let root = d.path().join("textures");
    let mut m = opened_without_export(d.path());
    m.save().unwrap();
    m.decide(WALL_N, Answer::Subtype(Subtype::Msn));
    settle(&mut m);
    bumpy_n(&t.join("other_n.png"));
    m.replan();
    settle(&mut m);
    m.decide(WALL_N, Answer::Subtype(Subtype::Normal));
    settle(&mut m);
    let disk_listing = || sdc_gui::sets_doc::SetsDoc::open(&root).auto_normal();
    assert_eq!(
        disk_listing().get(WALL_N).map(String::as_str),
        Some("normal")
    );
    let sets = m.sets_doc.as_mut().unwrap();
    assert!(!sets.user_dirty());
    assert!(!sets.auto_normal().contains_key(WALL_N));
    // Save must persist a real text difference even when the cached write flag is clear.
    sets.set_dirty(false);
    m.save().unwrap();
    assert_eq!(
        disk_listing(),
        [("test/other_n.png".into(), "normal".into())].into()
    );
    let mut reopened = opened_without_export(d.path());
    add_rmaos(&t, &mut reopened);
    assert_eq!(held(&reopened, WALL_N, "subtype"), Some(json!("normal")));
}

#[test]
fn listing_b_keeps_as_ownership_in_the_snapshot_where_a_is_normal() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    let mut m = opened_without_export(d.path());
    m.save().unwrap();
    check_close(&mut m, (false, false), (false, false));
    assert_eq!(m.untouched_normal(WALL_N), Some(Subtype::Normal));
    m.decide(WALL_N, Answer::Subtype(Subtype::Msn));
    settle(&mut m);
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("normal"));
    check_close(&mut m, (true, false), (true, false));
    let b = "test/other_n.png";
    bumpy_n(&t.join("other_n.png"));
    m.replan();
    settle(&mut m);
    assert!(
        !m.sets_doc.as_ref().unwrap().user_dirty(),
        "B's automatic prune is clean"
    );
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("msn")));
    assert_eq!(listed(&m, WALL_N), None);
    assert_eq!(m.untouched_normal(b), Some(Subtype::Normal));
    check_close(&mut m, (true, false), (true, true));
    m.undo();
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
    assert_eq!(
        listed(&m, WALL_N).as_deref(),
        Some("normal"),
        "A is still owned here"
    );
    assert_eq!(m.untouched_normal(b), Some(Subtype::Normal));
    assert!(!m.can_undo());
    check_close(&mut m, (true, false), (true, true));
    m.redo();
    settle(&mut m);
    assert!(!m.can_redo());
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("msn")));
    assert_eq!(m.untouched_normal(b), Some(Subtype::Normal));
    check_close(&mut m, (true, false), (true, true));
    m.undo();
    settle(&mut m);
    check_close(&mut m, (true, false), (true, true));
    add_rmaos(&t, &mut m);
    assert_eq!(m.untouched_normal(WALL_N), Some(Subtype::PbrNormal));
    check_close(&mut m, (true, false), (true, true));
}

#[test]
fn a_follow_only_command_owns_its_listing_prune() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    let b = "test/other_n.png";
    bumpy_n(&t.join("other_n.png"));
    let mut m = opened_without_export(d.path());
    check_close(&mut m, (false, false), (false, false));
    m.set_slot_lock("test/wall", 2, true).unwrap();
    settle(&mut m);
    check_close(&mut m, (false, true), (true, true));
    add_rmaos(&t, &mut m);
    check_close(&mut m, (false, true), (true, true));
    m.save().unwrap();
    check_close(&mut m, (false, false), (false, false));
    m.decide(b, Answer::Subtype(Subtype::Msn));
    settle(&mut m);
    assert_eq!(listed(&m, b).as_deref(), Some("normal"));
    check_close(&mut m, (true, false), (true, false));
    m.set_slot_lock("test/wall", 2, false).unwrap();
    settle(&mut m);
    assert_eq!(m.untouched_normal(WALL_N), Some(Subtype::PbrNormal));
    assert_eq!(listed(&m, b), None);
    check_close(&mut m, (true, true), (true, true));
    record_another_colour(&mut m);
    check_close(&mut m, (true, true), (true, true));
    m.undo();
    settle(&mut m);
    assert_eq!(
        listed(&m, b).as_deref(),
        Some("normal"),
        "Undo restores the follow's prune"
    );
    check_close(&mut m, (true, true), (true, true));
    m.undo();
    settle(&mut m);
    assert_eq!(listed(&m, b).as_deref(), Some("normal"));
    check_close(&mut m, (true, false), (true, false));
    m.undo();
    settle(&mut m);
    assert_eq!(m.untouched_normal(b), Some(Subtype::Normal));
    check_close(&mut m, (true, false), (true, false));
    m.undo();
    settle(&mut m);
    assert!(!m.can_undo());
    check_close(&mut m, (true, true), (true, true));
    // Lock, retype B, unlock, then follow A and prune B.
    for sets_dirty in [false, false, true, true] {
        assert!(m.can_redo());
        m.redo();
        settle(&mut m);
        check_close(&mut m, (true, sets_dirty), (true, sets_dirty));
    }
    assert!(!m.can_redo());
    assert_eq!(m.untouched_normal(WALL_N), Some(Subtype::PbrNormal));
    assert_eq!(held(&m, b, "subtype"), Some(json!("msn")));
    assert_eq!(listed(&m, b), None);
}

/// An Undo of the follow step stays undone: the next settled plan does not retype the normal again (`normal-follow`
/// marker in `History::auto_written`).
#[test]
fn an_undone_follow_stays_undone() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    add_rmaos(&t, &mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("pbr_normal")));
    m.undo();
    settle(&mut m);
    m.replan();
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("normal"));
    assert!(m.can_redo(), "the undone step is still offered again");
}

#[test]
fn an_established_follow_discards_an_unrelated_redo() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    let mut m = opened_without_export(d.path());
    m.save().unwrap();
    check_close(&mut m, (false, false), (false, false));
    m.set_alpha("test/wall.png", AlphaPolicy::Blend);
    settle(&mut m);
    check_close(&mut m, (true, false), (true, false));
    m.undo();
    settle(&mut m);
    assert!(!m.can_undo() && m.can_redo());
    check_close(&mut m, (true, false), (true, false));
    record_another_colour(&mut m);
    assert!(m.can_redo(), "an ordinary recording leaves Redo available");
    check_close(&mut m, (true, false), (true, false));
    add_rmaos(&t, &mut m);
    assert_eq!(m.untouched_normal(WALL_N), Some(Subtype::PbrNormal));
    check_close(&mut m, (true, false), (true, true));
    assert!(!m.can_redo(), "the follow replaces the unrelated Redo");
    m.undo();
    settle(&mut m);
    assert!(!m.can_undo());
    assert_eq!(m.untouched_normal(WALL_N), Some(Subtype::Normal));
    check_close(&mut m, (true, false), (true, false));
    m.redo();
    settle(&mut m);
    assert!(!m.can_redo());
    assert_eq!(m.untouched_normal(WALL_N), Some(Subtype::PbrNormal));
    check_close(&mut m, (true, false), (true, true));
}

#[test]
fn a_follow_clears_redo_and_the_user_can_claim_normal_again() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    let mut m = opened_without_export(d.path());
    m.save().unwrap();
    check_close(&mut m, (false, false), (false, false));
    m.decide(WALL_N, Answer::Subtype(Subtype::Normal));
    settle(&mut m);
    assert_eq!(listed(&m, WALL_N), None);
    check_close(&mut m, (true, true), (true, true));
    m.undo();
    settle(&mut m);
    assert!(!m.can_undo() && m.can_redo());
    check_close(&mut m, (true, false), (true, false));
    record_another_colour(&mut m);
    assert!(m.can_redo());
    check_close(&mut m, (true, false), (true, false));
    add_rmaos(&t, &mut m);
    assert_eq!(m.untouched_normal(WALL_N), Some(Subtype::PbrNormal));
    assert!(m.unsaved());
    check_close(&mut m, (true, false), (true, true));
    assert!(!m.can_redo(), "the follow clears the pending claim's Redo");
    m.decide(WALL_N, Answer::Subtype(Subtype::Normal));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
    assert_eq!(m.untouched_normal(WALL_N), None);
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("pbr_normal"));
    assert!(m.unsaved());
    check_close(&mut m, (true, false), (true, true));
    m.undo();
    settle(&mut m);
    assert_eq!(m.untouched_normal(WALL_N), Some(Subtype::PbrNormal));
    check_close(&mut m, (true, false), (true, true));
    m.undo();
    settle(&mut m);
    assert!(!m.can_undo());
    assert_eq!(m.untouched_normal(WALL_N), Some(Subtype::Normal));
    check_close(&mut m, (true, false), (true, false));
    m.redo();
    settle(&mut m);
    assert_eq!(m.untouched_normal(WALL_N), Some(Subtype::PbrNormal));
    check_close(&mut m, (true, false), (true, true));
    m.redo();
    settle(&mut m);
    assert!(!m.can_redo());
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
    assert_eq!(m.untouched_normal(WALL_N), None);
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("pbr_normal"));
    check_close(&mut m, (true, false), (true, true));
}

/// A `normal` the user typed is never retyped (the owner's "data treatment" by type), a pin.
#[test]
fn a_typed_normal_stays_when_its_set_turns_pbr() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.decide(WALL_N, Answer::Subtype(Subtype::Normal));
    settle(&mut m);
    add_rmaos(&t, &mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
}

/// N-19: a never-recorded `_n` in an automatic PBR Landscape set is recorded `pbr_normal` and listed, in the
/// recording's own step; its alpha is dropped silently.
#[test]
fn a_landscape_n_in_a_pbr_landscape_set_is_recorded_pbr_normal() {
    let d = tempfile::tempdir().unwrap();
    rock(d.path());
    let m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(held(&m, ROCK_N, "subtype"), Some(json!("pbr_normal")));
    assert_eq!(listed(&m, ROCK_N).as_deref(), Some("pbr_normal"));
    let j = m.item(ROCK_N).and_then(|i| i.job.clone()).expect("job");
    assert_eq!(j.profile.alpha.policy, AlphaPolicy::Remove);
    assert!(!j
        .warnings
        .iter()
        .any(|w| w.code == engine::codes::Code::LossAlphaData));
}

/// N-19, a pin: a landscape `_n` whose alpha policy the user wrote is recorded as the engine suggests and stays.
#[test]
fn a_landscape_n_with_the_users_alpha_keeps_its_type() {
    let d = tempfile::tempdir().unwrap();
    rock(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"landscape/rock_n.png\"]\nalpha.policy = \"data\"\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(held(&m, ROCK_N, "subtype"), Some(json!("landscape_normal")));
    m.replan();
    settle(&mut m);
    assert_eq!(held(&m, ROCK_N, "subtype"), Some(json!("landscape_normal")));
}

/// A pin: with `sdc-project.toml` read-only the type could not be listed, so the recorder writes the engine's own
/// type, as before.
#[test]
fn a_read_only_sets_document_gets_no_pbr_normal() {
    let d = tempfile::tempdir().unwrap();
    rock(d.path());
    std::fs::write(d.path().join("textures/sdc-project.toml"), "[project\n").unwrap();
    let mut m = opened_without_export(d.path());
    assert!(
        m.sets_doc.as_ref().unwrap().read_only(),
        "precondition: unreadable sdc-project.toml"
    );
    assert_eq!(held(&m, ROCK_N, "subtype"), Some(json!("landscape_normal")));
    check_close(&mut m, (false, false), (false, false));
}

/// G2, G3 (N-17): an explicit PBR pick types the untouched normal `pbr_normal` in the pick's own step, a regular pick
/// types it back to `normal`, and each is one Undo. No fuzz map is involved.
#[test]
fn a_pick_moves_an_untouched_normal_in_and_out_of_pbr() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("pbr_normal")));
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("pbr_normal"));
    assert_eq!(m.undo_label(), Some("Shader"), "the pick's own step");
    m.set_shader("test/wall", Some(Shader::Default));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
    m.undo();
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("pbr_normal")));
}

/// N-17: back to Auto after an explicit PBR pick clears the listed `pbr_normal`; Auto (a regular set) then reads
/// `normal`.
#[test]
fn auto_after_a_pbr_pick_takes_the_normal_back() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(&mut m);
    m.set_shader("test/wall", None);
    settle(&mut m);
    let j = m.item(WALL_N).and_then(|i| i.job.clone()).unwrap();
    assert_eq!(j.profile.subtype, Subtype::Normal);
    assert_ne!(held(&m, WALL_N, "subtype"), Some(json!("pbr_normal")));
}

/// G4 (N-17): an explicit Advanced Skin pick with no skin evidence types the untouched normal `pbr_normal`. A pin: it
/// passes with or without the pick's own write, because the wall's diffuse triggers the diffuse Switch inside the
/// same step; `an_advanced_skin_pick_without_a_diffuse_types_the_normal_in_its_own_step` isolates the pick.
#[test]
fn an_advanced_skin_pick_types_the_untouched_normal() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.set_shader("test/wall", Some(Shader::AdvancedSkin));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("pbr_normal")));
    assert_eq!(m.undo_label(), Some("Shader"), "the pick's own step");
}

/// N-17 (N6-a): the pick's rule owns tangent-space values only. An Eye pick still takes an untouched `normal` to `msn`
/// (FC1 R5), a regular pick takes it back, and the explicit PBR pick then types and lists `pbr_normal`.
#[test]
fn an_eye_pick_then_a_regular_pick_then_pbr_keeps_the_normal_moving() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("normal"));
    m.set_shader("test/wall", Some(Shader::Eye));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("msn")));
    m.set_shader("test/wall", Some(Shader::Default));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("pbr_normal")));
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("pbr_normal"));
}

/// N-19: an explicit regular pick takes an untouched landscape `_n` back to the engine's type, `landscape_normal`.
#[test]
fn a_regular_pick_takes_a_landscape_n_back_to_landscape_normal() {
    let d = tempfile::tempdir().unwrap();
    rock(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(held(&m, ROCK_N, "subtype"), Some(json!("pbr_normal")));
    m.set_shader("landscape/rock", Some(Shader::Landscape));
    settle(&mut m);
    assert_eq!(held(&m, ROCK_N, "subtype"), Some(json!("landscape_normal")));
    let j = m.item(ROCK_N).and_then(|i| i.job.clone()).unwrap();
    assert_eq!(j.profile.alpha.policy, AlphaPolicy::Data);
}

/// N-17, N-19: a pick never retypes a normal the user typed (plain or landscape), nor one whose alpha the user wrote.
#[test]
fn a_pick_leaves_the_users_normal_alone() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.decide(WALL_N, Answer::Subtype(Subtype::Normal));
    settle(&mut m);
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));

    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.set_alpha(WALL_N, AlphaPolicy::Data);
    settle(&mut m);
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));

    let d = tempfile::tempdir().unwrap();
    rock(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.decide(ROCK_N, Answer::Subtype(Subtype::LandscapeNormal));
    settle(&mut m);
    m.set_shader("landscape/rock", Some(Shader::PbrLandscape));
    settle(&mut m);
    assert_eq!(held(&m, ROCK_N, "subtype"), Some(json!("landscape_normal")));
}

/// N-20: a recorded `normal` from before this change (in `sdc.toml`, not listed) reads as typed: a PBR pick keeps it.
#[test]
fn an_unlisted_recorded_normal_survives_a_pbr_pick() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"test/wall_n.png\"]\nsubtype = \"normal\"\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(listed(&m, WALL_N), None);
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
}

/// N-17: in a set holding a fuzz map (read Glow, as Fuzz-27's set is), a `normal` the user typed equal to the
/// recorder's is never retyped by an explicit PBR pick (`untouched_for_shader`'s `pbr_pick` arm) nor back.
#[test]
fn a_typed_normal_in_a_fuzz_map_set_stays() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    ImageBuffer::from_fn(64, 64, |x, _| Rgb([(x * 4) as u8, 200, 40]))
        .save(t.join("wall_g.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |x, _| Luma([(x * 3) as u8]))
        .save(t.join("wall_fuzz.png"))
        .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.decide(WALL_N, Answer::Subtype(Subtype::Normal));
    settle(&mut m);
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
    m.set_shader("test/wall", Some(Shader::Default));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
}

/// N-28: a typed `normal` stays when the diffuse is decided `pbr_albedo` (`pbr_follow`'s Switch), and when the set is
/// picked Advanced Skin (its diffuse step runs the same Switch).
#[test]
fn the_diffuse_switch_leaves_a_typed_normal() {
    for skin in [false, true] {
        let d = tempfile::tempdir().unwrap();
        wall(d.path());
        let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
        m.decide(WALL_N, Answer::Subtype(Subtype::Normal));
        settle(&mut m);
        if skin {
            m.set_shader("test/wall", Some(Shader::AdvancedSkin));
        } else {
            m.decide("test/wall.png", Answer::Subtype(Subtype::PbrAlbedo));
        }
        settle(&mut m);
        assert_eq!(
            held(&m, WALL_N, "subtype"),
            Some(json!("normal")),
            "skin {skin}"
        );
        assert!(!m
            .sets_doc
            .as_ref()
            .unwrap()
            .auto_pbr()
            .iter()
            .any(|k| k == WALL_N));
    }
}

/// N-28: the Switch types an untouched listed normal `pbr_normal`, keeps it in `auto_normal` (never `auto_pbr`), and
/// the diffuse leaving PBR takes it back.
#[test]
fn the_diffuse_switch_moves_an_untouched_normal_and_takes_it_back() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.decide("test/wall.png", Answer::Subtype(Subtype::PbrAlbedo));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("pbr_normal")));
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("pbr_normal"));
    assert!(!m
        .sets_doc
        .as_ref()
        .unwrap()
        .auto_pbr()
        .iter()
        .any(|k| k == WALL_N));
    m.decide("test/wall.png", Answer::Subtype(Subtype::Diffuse));
    settle(&mut m);
    assert_eq!(
        facts(&m, WALL_N),
        (Some(json!("normal")), Some("normal".into()), false)
    );
}

/// N-17: an explicit regular pick takes a followed `pbr_normal` back to `normal` and clears the follow's marker, so
/// the set returning to PBR on Auto in the same session follows again.
#[test]
fn a_regular_pick_clears_the_follow_marker() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    add_rmaos(&t, &mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("pbr_normal")));
    m.set_shader("test/wall", Some(Shader::Default));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
    m.set_shader("test/wall", None);
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("pbr_normal")));
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("pbr_normal"));
}

/// The three facts of a key: the held `[files]` sub-type, its `auto_normal` word, and whether `auto_pbr` lists it.
fn facts(m: &AppModel, key: &str) -> (Option<serde_json::Value>, Option<String>, bool) {
    (
        held(m, key, "subtype"),
        listed(m, key),
        m.sets_doc
            .as_ref()
            .unwrap()
            .auto_pbr()
            .iter()
            .any(|k| k == key),
    )
}

/// The wall with its `_rmaos` added after the open: the recorder followed, so its normal is `pbr_normal` and listed.
fn followed_wall(dir: &Path) -> AppModel {
    let t = wall(dir);
    let mut m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    add_rmaos(&t, &mut m);
    assert_eq!(
        facts(&m, WALL_N),
        (Some(json!("pbr_normal")), Some("pbr_normal".into()), false)
    );
    m
}

/// N-17 (N6-c, N6-d): an Eye pick takes a followed `pbr_normal` to `msn` (Eye accepts nothing else) and leaves its
/// listed word; the way back from `msn` is the normal rule's write, listed, and never in `auto_pbr`.
#[test]
fn eye_from_a_followed_normal_is_msn_and_the_way_back_is_listed() {
    let word = |s: &str| (Some(json!(s)), Some(s.to_string()), false);
    for (leg, want) in [
        (Some(Shader::Default), word("normal")),
        (Some(Shader::Pbr), word("pbr_normal")),
    ] {
        let d = tempfile::tempdir().unwrap();
        let mut m = followed_wall(d.path());
        m.set_shader("test/wall", Some(Shader::Eye));
        settle(&mut m);
        assert_eq!(
            facts(&m, WALL_N),
            (Some(json!("msn")), Some("pbr_normal".into()), false),
            "Eye"
        );
        m.set_shader("test/wall", leg);
        settle(&mut m);
        assert_eq!(facts(&m, WALL_N), want, "{leg:?}");
    }
}

/// N-17 (N6-d): Eye, then Auto: the Eye step leaves `msn` with the parked word; Auto clears once, the set is PBR by
/// its `_rmaos`, so the recorder then follows and the key ends `pbr_normal`, listed, off `auto_pbr`.
#[test]
fn eye_then_auto_leaves_the_lists_agreeing_with_the_key() {
    let d = tempfile::tempdir().unwrap();
    let mut m = followed_wall(d.path());
    m.set_shader("test/wall", Some(Shader::Eye));
    settle(&mut m);
    assert_eq!(
        facts(&m, WALL_N),
        (Some(json!("msn")), Some("pbr_normal".into()), false)
    );
    m.set_shader("test/wall", None);
    assert_eq!(facts(&m, WALL_N), (None, None, false), "Auto clears once");
    settle(&mut m);
    assert_eq!(
        facts(&m, WALL_N),
        (Some(json!("pbr_normal")), Some("pbr_normal".into()), false)
    );
}

#[test]
fn auto_before_the_eye_plan_lands_clears_msn_and_exports_the_followed_normal() {
    let d = tempfile::tempdir().unwrap();
    let mut m = followed_wall(d.path());
    m.set_shader("test/wall", Some(Shader::Eye));
    assert_plan_pending(&m, "test/wall", Shader::Eye);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("msn")));
    m.set_shader("test/wall", None);
    assert_eq!(
        facts(&m, WALL_N),
        (None, None, false),
        "Auto clears the pending Eye type"
    );
    settle(&mut m);
    m.save().unwrap();
    let root = d.path().join("textures");
    let saved = sdc_gui::sets_doc::SetsDoc::open(&root);
    assert!(saved
        .entries()
        .iter()
        .all(|e| e.id != "test/wall" || e.shader.is_none()));
    assert_eq!(
        saved.auto_normal().get(WALL_N).map(String::as_str),
        Some("pbr_normal")
    );
    assert_eq!(
        sdc_gui::project::ProjectDoc::open(&root).file_value(WALL_N, "subtype"),
        Some(json!("pbr_normal"))
    );
    let job = m.item(WALL_N).unwrap().job.as_ref().unwrap();
    assert_eq!(job.profile.subtype, Subtype::PbrNormal);
    let out = job.output.clone();
    export_through(&mut m);
    settle(&mut m);
    assert!(out.is_file(), "the followed normal was exported");
}

/// B1 (N6-i): Eye, then Auto clears the key once: a sub-type the user wrote under the extension-less `[files]` spelling
/// is still there.
#[test]
fn auto_after_eye_keeps_the_users_extensionless_subtype() {
    let d = tempfile::tempdir().unwrap();
    let mut m = followed_wall(d.path());
    m.set_shader("test/wall", Some(Shader::Eye));
    settle(&mut m);
    m.edit_docs("user glow", None, true, |p, _| {
        p.set_file_value("test/wall_n", "subtype", &json!("glow"))
    });
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("msn")));
    m.set_shader("test/wall", None);
    assert_eq!(held(&m, "test/wall_n", "subtype"), Some(json!("glow")));
}

/// The key of the member that owns the set's Normal slot.
fn normal_owner(m: &AppModel, id: &str) -> Option<String> {
    let v = sdc_gui::model::sets::set_views(m)
        .into_iter()
        .find(|v| v.id == id)?;
    match v.slots.get(&sdc_gui::model::sets::SlotKind::Normal) {
        Some(sdc_gui::model::sets::SlotFill::Own { item }) => Some(item.clone()),
        _ => None,
    }
}

/// The sets document's pick differs from the last plan's: the plan has not seen it yet.
fn assert_plan_pending(m: &AppModel, id: &str, choice: Shader) {
    let v = sdc_gui::model::sets::set_views(m)
        .into_iter()
        .find(|v| v.id == id)
        .unwrap();
    assert_eq!(v.shader_choice, Some(choice), "the live choice");
    assert!(
        !v.shader
            .as_ref()
            .is_some_and(|p| p.provenance == engine::shader::ShaderOrigin::Set),
        "the last plan has not seen the pick: {:?}",
        v.shader
    );
}

/// B1 (N6-h): PBR, then Auto before the PBR plan lands: the Auto leg reads the live pick, so the key is cleared and
/// unlisted, never held and listed `pbr_normal`.
#[test]
fn auto_right_after_a_pbr_pick_clears_the_normal() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("normal"));
    m.set_shader("test/wall", Some(Shader::Pbr));
    assert_plan_pending(&m, "test/wall", Shader::Pbr);
    m.set_shader("test/wall", None);
    assert_eq!(facts(&m, WALL_N), (None, None, false));
}

/// B4 (N6-h): an automatically PBR set with a PBR diffuse and a listed `pbr_normal`: PBR is picked, then the diffuse
/// is decided `diffuse` before the plan lands. The Revert reads the live pick and keeps the normal.
#[test]
fn a_diffuse_revert_before_the_plan_lands_reads_the_live_pick() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    ImageBuffer::from_fn(64, 64, |x, _| Rgb([(x * 4) as u8, 200, 40]))
        .save(t.join("wall_g.png"))
        .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    add_rmaos(&t, &mut m);
    m.decide("test/wall.png", Answer::Subtype(Subtype::PbrAlbedo));
    settle(&mut m);
    // The Revert has something to do: a glow map an earlier Switch typed `pbr_emissive`, listed in `auto_pbr`.
    m.edit_docs("switched", None, true, |p, s| {
        p.set_file_value("test/wall_g.png", "subtype", &json!("pbr_emissive"));
        s.set_auto_pbr(&["test/wall_g.png".to_string()]);
    });
    m.set_shader("test/wall", Some(Shader::Pbr));
    assert_plan_pending(&m, "test/wall", Shader::Pbr);
    assert_eq!(
        held(&m, "test/wall_g.png", "subtype"),
        Some(json!("pbr_emissive"))
    );
    m.decide("test/wall.png", Answer::Subtype(Subtype::Diffuse));
    assert_ne!(
        held(&m, "test/wall_g.png", "subtype"),
        Some(json!("pbr_emissive")),
        "the Revert ran"
    );
    assert_eq!(
        facts(&m, WALL_N),
        (Some(json!("pbr_normal")), Some("pbr_normal".into()), false)
    );
}

/// N6-l: a key in both lists is the normal rule's alone in a manual Revert: under a settled explicit PBR pick it stays
/// `pbr_normal`, listed, off `auto_pbr`; without the pick it goes back to `tangent_back`, relisted, off `auto_pbr`.
#[test]
fn a_revert_leaves_a_key_in_both_lists_to_the_normal_rule() {
    for explicit in [true, false] {
        let d = tempfile::tempdir().unwrap();
        let mut m = followed_wall(d.path());
        m.decide("test/wall.png", Answer::Subtype(Subtype::PbrAlbedo));
        settle(&mut m);
        if explicit {
            m.set_shader("test/wall", Some(Shader::Pbr));
            settle(&mut m);
        }
        m.edit_docs("both lists", None, true, |_, s| {
            s.set_auto_pbr(&[WALL_N.to_string()])
        });
        m.decide("test/wall.png", Answer::Subtype(Subtype::Diffuse));
        let want = if explicit { "pbr_normal" } else { "normal" };
        assert_eq!(
            facts(&m, WALL_N),
            (Some(json!(want)), Some(want.into()), false),
            "explicit {explicit}"
        );
    }
}

/// B3 (N6-j): `alpha.policy` under the extension-less spelling `wall_n` and another field under `wall_n.png`: the
/// engine applies both, so the alpha is the user's. The Switch leaves an inferred normal alone (acceptance off), and a
/// listed `normal` is not untouched.
#[test]
fn an_alpha_under_the_extensionless_spelling_is_the_users() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"test/wall_n\"]\nalpha.policy = \"data\"\n\n[files.\"test/wall_n.png\"]\nformat = \"bc7\"\n",
    )
    .unwrap();
    let mut m = AppModel::new(
        sdc_gui::settings::GuiSettings {
            export_folder: Some(d.path().join("export")),
            ..sdc_gui::settings::GuiSettings::default()
        },
        None,
        session(),
    );
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(d.path().join("textures"));
    settle(&mut m);
    m.decide("test/wall.png", Answer::Subtype(Subtype::PbrAlbedo));
    settle(&mut m);
    assert_eq!(facts(&m, WALL_N), (None, None, false));

    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(m.untouched_normal(WALL_N), Some(Subtype::Normal));
    m.edit_docs("alpha", None, true, |p, _| {
        p.set_file_value("test/wall_n", "alpha.policy", &json!("data"))
    });
    assert_eq!(m.untouched_normal(WALL_N), None);
}

/// B2 (N6-e): a fuzz-map set where `wall_msn.png` sorts before `wall_n.png`: `normal` typed by the user on `_n` is
/// unchanged by an explicit PBR pick and by a regular pick. A pin of the guard on a real pick. The positive legacy
/// exception: a `pbr_normal` an older pick listed in `auto_pbr` is cleared when the set leaves PBR.
#[test]
fn a_typed_normal_beside_an_msn_survives_picks_and_an_older_listed_one_is_cleared() {
    let build = |d: &Path| {
        let t = fuzz_wall(d);
        ImageBuffer::from_fn(64, 64, |_, _| Rgb([128u8, 128, 255]))
            .save(t.join("wall_msn.png"))
            .unwrap();
        let mut m = opened_as_the_app(d.join("textures"), d.join("export"));
        m.decide(WALL_N, Answer::Subtype(Subtype::Normal));
        settle(&mut m);
        assert_eq!(
            normal_owner(&m, "test/wall").as_deref(),
            Some("test/wall_msn.png"),
            "the `_msn` owns the Normal slot; `_n` is the secondary member"
        );
        m
    };
    let d = tempfile::tempdir().unwrap();
    let mut m = build(d.path());
    let typed = facts(&m, WALL_N);
    assert_eq!(typed, (Some(json!("normal")), None, false));
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(facts(&m, WALL_N), typed, "explicit PBR pick");
    m.set_shader("test/wall", Some(Shader::Default));
    settle(&mut m);
    assert_eq!(facts(&m, WALL_N), typed, "regular pick");

    let d = tempfile::tempdir().unwrap();
    let mut m = build(d.path());
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(&mut m);
    m.edit_docs("older pick", None, true, |p, s| {
        p.set_file_value(WALL_N, "subtype", &json!("pbr_normal"));
        s.set_auto_pbr(&[WALL_N.to_string()]);
    });
    m.set_shader("test/wall", Some(Shader::Default));
    assert_eq!(facts(&m, WALL_N), (None, None, false), "the leave step");
}

/// N6-k: the diffuse's Switch governs every free member that is inferred or untouched-listed, not only the Normal
/// slot's: in the `_msn`/`_n` set a recorder-listed `normal` on the secondary `_n` switches to `pbr_normal` and stays
/// in `auto_normal` (never `auto_pbr`); with acceptance off the inferred one switches too. The Revert takes both back.
#[test]
fn the_switch_governs_every_free_member_not_only_the_normal_slot() {
    for accept_on in [true, false] {
        let d = tempfile::tempdir().unwrap();
        let t = wall(d.path());
        ImageBuffer::from_fn(64, 64, |_, _| Rgb([128u8, 128, 255]))
            .save(t.join("wall_msn.png"))
            .unwrap();
        let mut m = AppModel::new(
            sdc_gui::settings::GuiSettings {
                export_folder: Some(d.path().join("export")),
                ..sdc_gui::settings::GuiSettings::default()
            },
            None,
            session(),
        );
        if !accept_on {
            m.accept = engine::policy::AcceptSuggestions::None;
        }
        m.open_folder(d.path().join("textures"));
        settle(&mut m);
        assert_eq!(
            normal_owner(&m, "test/wall").as_deref(),
            Some("test/wall_msn.png"),
            "the `_msn` owns the Normal slot; `_n` is the secondary member"
        );
        m.decide("test/wall.png", Answer::Subtype(Subtype::PbrAlbedo));
        let (switched, back) = if accept_on {
            (
                (
                    Some(json!("pbr_normal")),
                    Some("pbr_normal".to_string()),
                    false,
                ),
                (Some(json!("normal")), Some("normal".to_string()), false),
            )
        } else {
            ((Some(json!("pbr_normal")), None, true), (None, None, false))
        };
        assert_eq!(facts(&m, WALL_N), switched, "accept on {accept_on}");
        settle(&mut m);
        m.decide("test/wall.png", Answer::Subtype(Subtype::Diffuse));
        assert_eq!(facts(&m, WALL_N), back, "accept on {accept_on}");
    }
}
/// A fuzz-map wall: `wall_g` and `wall_fuzz` beside the normal, so an explicit PBR pick runs R5's fuzz step.
fn fuzz_wall(dir: &Path) -> std::path::PathBuf {
    let t = wall(dir);
    ImageBuffer::from_fn(64, 64, |x, _| Rgb([(x * 4) as u8, 200, 40]))
        .save(t.join("wall_g.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |x, _| Luma([(x * 3) as u8]))
        .save(t.join("wall_fuzz.png"))
        .unwrap();
    t
}

/// N-17 (N6-e): the typed-normal guard covers every member, not only the Normal slot's. A set whose second `_n`-like
/// file the user typed `normal` (no job: the two compete for the slot, so no real plan reaches R5 with it) keeps it:
/// the pick's resolution drops every R5 entry for it, but a legacy leave clear of a key in `auto_pbr`.
#[test]
fn the_typed_normal_guard_covers_every_member() {
    const SECOND: &str = "test/wall_normal.png";
    let d = tempfile::tempdir().unwrap();
    let t = fuzz_wall(d.path());
    bumpy_n(&t.join("wall_normal.png"));
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.decide(SECOND, Answer::Subtype(Subtype::Normal));
    settle(&mut m);
    let r5 = vec![
        (SECOND.to_string(), Some(json!("pbr_normal")), true),
        (
            "test/wall_fuzz.png".to_string(),
            Some(json!("pbr_fuzz")),
            true,
        ),
        (SECOND.to_string(), None, true),
    ];
    let kept = m.normal_plan_probe("test/wall", Some(Shader::Pbr), r5);
    // The fuzz map's entry stands; the typed member's value entry goes; its clear stands only for a key an older
    // pick listed in `auto_pbr` (this one is not listed).
    assert_eq!(
        kept,
        vec![(
            "test/wall_fuzz.png".to_string(),
            Some(json!("pbr_fuzz")),
            true
        )]
    );
}
/// N-28 (N6-e): the Switch reads the stored value. A `_n` typed `glow`, then typed `normal`, then the diffuse decided
/// before the plan settles: the `_n` stays `normal`, in no list.
#[test]
fn the_switch_reads_the_stored_type_of_a_member_the_plan_has_not_seen_yet() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.decide(WALL_N, Answer::Subtype(Subtype::Glow));
    settle(&mut m);
    m.decide(WALL_N, Answer::Subtype(Subtype::Normal));
    m.decide("test/wall.png", Answer::Subtype(Subtype::PbrAlbedo));
    settle(&mut m);
    assert_eq!(facts(&m, WALL_N), (Some(json!("normal")), None, false));
}

/// N-17, N-19 (N6-f): an inferred normal whose `alpha.policy` the user wrote is never switched or listed by the
/// diffuse's Switch.
#[test]
fn the_switch_leaves_an_inferred_normal_with_the_users_alpha() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"test/wall_n.png\"]\nalpha.policy = \"data\"\n",
    )
    .unwrap();
    let mut m = AppModel::new(
        sdc_gui::settings::GuiSettings {
            export_folder: Some(d.path().join("export")),
            ..sdc_gui::settings::GuiSettings::default()
        },
        None,
        session(),
    );
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(d.path().join("textures"));
    settle(&mut m);
    assert_eq!(facts(&m, WALL_N), (None, None, false));
    m.decide("test/wall.png", Answer::Subtype(Subtype::PbrAlbedo));
    settle(&mut m);
    assert_eq!(facts(&m, WALL_N), (None, None, false));
}

/// N-17 (N6-c): a saved fuzz-map set under an explicit PBR pick whose `pbr_normal` is in both lists (the two lists
/// are read independently): Advanced Skin keeps it `pbr_normal` listed in `auto_normal` only; Default types it back
/// and lists that word; Auto clears it and unlists it.
#[test]
fn a_normal_in_both_lists_resolves_to_one_final_value_per_pick() {
    let word = |s: &str| (Some(json!(s)), Some(s.to_string()), false);
    for (shader, want) in [
        (Some(Shader::AdvancedSkin), word("pbr_normal")),
        (Some(Shader::Default), word("normal")),
        (None, (None, None, false)),
    ] {
        let d = tempfile::tempdir().unwrap();
        fuzz_wall(d.path());
        let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
        m.set_shader("test/wall", Some(Shader::Pbr));
        settle(&mut m);
        m.edit_docs("both lists", None, true, |p, s| {
            p.set_file_value(WALL_N, "subtype", &json!("pbr_normal"));
            s.set_auto_pbr(&[WALL_N.to_string()]);
            s.set_auto_normal(&std::collections::BTreeMap::from([(
                WALL_N.to_string(),
                "pbr_normal".to_string(),
            )]));
        });
        settle(&mut m);
        assert_eq!(m.untouched_normal(WALL_N), Some(Subtype::PbrNormal));
        m.set_shader("test/wall", shader);
        // The pick alone, before the next plan records anything.
        assert_eq!(facts(&m, WALL_N), want, "{shader:?}");
    }
}

/// N-17 (N6-g): the recorder follows, the user undoes it, picks PBR, and undoes the pick: the key is `normal`, the
/// recorder does not follow again, and Redo is still offered.
#[test]
fn undoing_a_pick_after_an_undone_follow_does_not_follow_again() {
    let d = tempfile::tempdir().unwrap();
    let mut m = followed_wall(d.path());
    m.undo();
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(&mut m);
    m.undo();
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
    assert!(m.can_redo(), "Redo is still offered");
}

/// N-28 (N6-g): a followed normal on a wall whose diffuse was recorded `diffuse`: Advanced Skin, then PBR. The pick
/// types the normal `pbr_normal` and the diffuse's Revert leaves it (the explicit pick is a PBR-set shader), in the
/// pick's one step; a settle adds none.
#[test]
fn advanced_skin_then_pbr_keeps_a_followed_normal_in_one_step() {
    let d = tempfile::tempdir().unwrap();
    let mut m = followed_wall(d.path());
    m.set_shader("test/wall", Some(Shader::AdvancedSkin));
    settle(&mut m);
    let before = m.history.undo.len();
    m.set_shader("test/wall", Some(Shader::Pbr));
    let after_pick = m.history.undo.len();
    assert_eq!(after_pick, before + 1, "one Shader step");
    settle(&mut m);
    assert_eq!(m.history.undo.len(), after_pick, "a settle adds none");
    assert_eq!(
        facts(&m, WALL_N),
        (Some(json!("pbr_normal")), Some("pbr_normal".into()), false)
    );
}

/// G4 (N-17): an explicit Advanced Skin pick on a set with no diffuse types the untouched normal `pbr_normal` in
/// the pick's own step (no diffuse Switch, no recorder follow), listed in `auto_normal` only.
#[test]
fn an_advanced_skin_pick_without_a_diffuse_types_the_normal_in_its_own_step() {
    let d = tempfile::tempdir().unwrap();
    let t = d.path().join("textures/test");
    std::fs::create_dir_all(&t).unwrap();
    bumpy_n(&t.join("wall_n.png"));
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(
        facts(&m, WALL_N),
        (Some(json!("normal")), Some("normal".into()), false)
    );
    m.set_shader("test/wall", Some(Shader::AdvancedSkin));
    assert_eq!(m.undo_label(), Some("Shader"));
    assert_eq!(
        facts(&m, WALL_N),
        (Some(json!("pbr_normal")), Some("pbr_normal".into()), false),
        "the pick itself, before any plan"
    );
}

/// B3 (N6-j): the engine lowercases both sides, so `alpha.policy` under `TEST/WALL_N` beside a `test/wall_n.png` table
/// is the user's: the Switch leaves the inferred normal alone (acceptance off) and `auto_pbr` does not gain it.
#[test]
fn an_alpha_under_another_case_of_the_key_is_the_users() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"TEST/WALL_N\"]\nalpha.policy = \"data\"\n\n[files.\"test/wall_n.png\"]\nformat = \"bc7\"\n",
    )
    .unwrap();
    let mut m = AppModel::new(
        sdc_gui::settings::GuiSettings {
            export_folder: Some(d.path().join("export")),
            ..sdc_gui::settings::GuiSettings::default()
        },
        None,
        session(),
    );
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(d.path().join("textures"));
    settle(&mut m);
    m.decide("test/wall.png", Answer::Subtype(Subtype::PbrAlbedo));
    settle(&mut m);
    assert_eq!(facts(&m, WALL_N), (None, None, false));
}

/// B7 (N6-j): the user's `glow` and alpha under `TEST/WALL_N`, only `pbr_normal` under `test/wall_n.png`, the key in
/// both lists, an explicit PBR pick settled, a manual Revert: the key is not untouched, so the legacy loop clears the
/// `pbr_normal` (the user's `glow` shows through) and the normal rule keeps nothing.
#[test]
fn a_revert_clears_a_pbr_normal_whose_alpha_is_the_users_under_another_case() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"TEST/WALL_N\"]\nsubtype = \"glow\"\nalpha.policy = \"data\"\n\n[files.\"test/wall_n.png\"]\nsubtype = \"pbr_normal\"\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.decide("test/wall.png", Answer::Subtype(Subtype::PbrAlbedo));
    settle(&mut m);
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(&mut m);
    m.edit_docs("both lists", None, true, |p, s| {
        assert_eq!(p.file_value(WALL_N, "subtype"), Some(json!("pbr_normal")));
        s.set_auto_pbr(&[WALL_N.to_string()]);
        s.set_auto_normal(&std::collections::BTreeMap::from([(
            WALL_N.to_string(),
            "pbr_normal".to_string(),
        )]));
    });
    assert_eq!(m.untouched_normal(WALL_N), None, "the alpha is the user's");
    m.decide("test/wall.png", Answer::Subtype(Subtype::Diffuse));
    assert_eq!(
        held(&m, WALL_N, "subtype"),
        Some(json!("glow")),
        "the legacy loop cleared the pbr_normal and the user's glow shows through"
    );
}

/// N-28 (the Switch keeps the module's promise): a `normal` persisted in both lists is `pbr_normal` after the Switch,
/// listed with that word, and off `auto_pbr`.
#[test]
fn the_switch_takes_an_untouched_key_off_auto_pbr() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.edit_docs("both lists", None, true, |_, s| {
        s.set_auto_pbr(&[WALL_N.to_string()])
    });
    assert_eq!(
        facts(&m, WALL_N),
        (Some(json!("normal")), Some("normal".into()), true)
    );
    m.decide("test/wall.png", Answer::Subtype(Subtype::PbrAlbedo));
    assert_eq!(
        facts(&m, WALL_N),
        (Some(json!("pbr_normal")), Some("pbr_normal".into()), false)
    );
}

/// N-18: on a `pbr_normal` in a PBR set the texture panel's Alpha row offers Data; picking it writes
/// `alpha.policy = "data"`, raises no error, and survives retypes (the user's own, and shader picks).
#[test]
fn data_is_offered_on_a_pbr_normal_and_sticks() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("pbr_normal")));
    m.select_item(WALL_N);
    let mut h = keep_harness(m);
    pick_data_over_remove(&mut h);
    h.run();
    let m = h.state_mut();
    assert_eq!(held(m, WALL_N, "alpha.policy"), Some(json!("data")));
    let j = m.item(WALL_N).and_then(|i| i.job.clone()).unwrap();
    assert_eq!(j.profile.alpha.policy, AlphaPolicy::Data);
    assert!(
        j.warnings
            .iter()
            .all(|w| w.severity != engine::codes::Severity::Error),
        "{:?}",
        j.warnings
    );
    m.set_field(WALL_N, "subtype", json!("normal"));
    settle(m);
    m.set_field(WALL_N, "subtype", json!("pbr_normal"));
    settle(m);
    assert_eq!(
        held(m, WALL_N, "alpha.policy"),
        Some(json!("data")),
        "the user's retypes"
    );
    m.set_shader("test/wall", Some(Shader::Default));
    settle(m);
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(m);
    assert_eq!(
        held(m, WALL_N, "alpha.policy"),
        Some(json!("data")),
        "the picks"
    );
    let j = m.item(WALL_N).and_then(|i| i.job.clone()).unwrap();
    assert_eq!(j.profile.alpha.policy, AlphaPolicy::Data);
}

/// The files of two sets: `textures/pbr/iron.png`, `iron_n.png` (varying alpha), `iron_rmaos.png`: a PBR set;
/// `textures/armor/steel.png` with its own `steel_n.png`: a regular set.
fn two_sets_files(dir: &Path) {
    let pbr = dir.join("textures/pbr");
    let armor = dir.join("textures/armor");
    for f in [&pbr, &armor] {
        std::fs::create_dir_all(f).unwrap();
    }
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([120u8, 90, 60]))
        .save(pbr.join("iron.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([90u8, 90, 100]))
        .save(armor.join("steel.png"))
        .unwrap();
    bumpy_n(&pbr.join("iron_n.png"));
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([128u8, 128, 255]))
        .save(armor.join("steel_n.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([100u8, 0, 255]))
        .save(pbr.join("iron_rmaos.png"))
        .unwrap();
}

/// `two_sets_files` without `steel_n.png`: `armor/steel`'s Normal slot is empty, so a Share into it changes only its
/// borrow line. (N8-r: a Share that replaces `steel_n.png`, an automatic member, puts that file in no set, which
/// changes the borrower's membership, so that step writes nothing.)
fn open_slot_files(dir: &Path) {
    two_sets_files(dir);
    std::fs::remove_file(dir.join("textures/armor/steel_n.png")).unwrap();
}

/// `open_slot_files`, opened as the app opens them.
pub(super) fn two_sets(dir: &Path) -> AppModel {
    open_slot_files(dir);
    let m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    let ids: Vec<String> = sdc_gui::model::sets::views(&m)
        .iter()
        .map(|v| v.id.clone())
        .collect();
    assert!(
        ids.iter().any(|i| i == "pbr/iron") && ids.iter().any(|i| i == "armor/steel"),
        "{ids:?}"
    );
    m
}

const IRON_N: &str = "pbr/iron_n.png";

#[test]
fn a_borrowed_slot_shows_the_first_normal_but_bakes_from_the_lenders_priority_pick() {
    use sdc_gui::model::sets::{views, SlotFill, SlotKind};
    let d = tempfile::tempdir().unwrap();
    open_slot_files(d.path());
    let root = d.path().join("textures");
    let bumpy = "pbr/z_n.png";
    bumpy_n(&root.join(bumpy));
    image::RgbImage::from_pixel(64, 64, Rgb([128, 128, 255]))
        .save(root.join(IRON_N))
        .unwrap();
    std::fs::copy(
        root.join("pbr/iron_rmaos.png"),
        root.join("armor/steel_rmaos.png"),
    )
    .unwrap();
    let mut m = open_saved_share(
        d.path(),
        "[files.'pbr/iron_n.png']\nsubtype = 'landscape_normal'\n[files.'pbr/z_n.png']\nsubtype = 'normal'\n",
        "[[sets]]\nid = 'pbr/iron'\nshader = 'pbr'\nadd = ['pbr/z_n.png']\n[[sets]]\nid = 'armor/steel'\nshader = 'pbr'\n",
    );
    assert_eq!(normal_owner(&m, "pbr/iron").as_deref(), Some(IRON_N));
    share(&mut m, IRON_N, "armor/steel");
    assert!(
        matches!(views(&m).iter().find(|v| v.id == "armor/steel").unwrap().slots.get(&SlotKind::Normal),
        Some(SlotFill::Borrowed {item, ..}) if item == IRON_N)
    );
    let check_sources = |m: &AppModel, source: &str| {
        for key in ["armor/steel_rmaos.png", "pbr/iron_rmaos.png"] {
            let job = m.item(key).unwrap().job.as_ref().unwrap();
            assert_eq!(
                job.bake.as_ref().unwrap().normal,
                root.join(source),
                "bake source of {key}"
            );
        }
    };
    check_sources(&m, bumpy);
    let out = m
        .item("armor/steel_rmaos.png")
        .unwrap()
        .job
        .as_ref()
        .unwrap()
        .output
        .clone();
    export_through(&mut m);
    settle(&mut m);
    let bytes = std::fs::read(out).unwrap();
    let dds = engine::dds::read_dds(&bytes).unwrap();
    let red = |level: usize| {
        let (w, h, r) = dds.levels[level].clone();
        let pixels = engine::decode::decode_level_rgba(dds.format, w, h, &dds.data[r]).unwrap();
        pixels[0][0]
    };
    assert!(
        red(1) > red(0) + 2.0 / 255.0,
        "the bumpy normal raises roughness in mip 1"
    );
    m.set_field(IRON_N, "subtype", json!("pbr_normal"));
    settle(&mut m);
    check_sources(&m, IRON_N);
}

/// Share `item` into the empty Normal slot of set `into` as the Sets column does (`drop_choice` returns `Joined`), and
/// let the plan land. A filled slot (`AskReplace`) is `share_replacing`'s.
fn share(m: &mut AppModel, item: &str, into: &str) {
    use sdc_gui::model::sets::{DropChoice, DropOutcome};
    match m.drop_choice(item, into, DropChoice::Share) {
        DropOutcome::Joined => {}
        other => panic!("not an empty slot: {other:?}"),
    }
    settle(m);
}

/// Share `item` over the filled Normal slot of set `into` and confirm Replace, as the Sets column does
/// (`drop_choice` returns `AskReplace`, sets.rs `drop_choice`), and let the plan land.
fn share_replacing(m: &mut AppModel, item: &str, into: &str) {
    use sdc_gui::model::sets::{DropChoice, DropOutcome};
    match m.drop_choice(item, into, DropChoice::Share) {
        DropOutcome::AskReplace { kind, existing } => m
            .drop_replace(item, into, DropChoice::Share, kind, &existing)
            .expect("shared"),
        other => panic!("not a filled slot: {other:?}"),
    }
    settle(m);
}

/// N8-m, N8-r (round 4; this test expected `data` and a list entry before): Share `iron_n` over `armor/steel`'s own
/// `steel_n`, an automatic member, and confirm Replace. The Replace puts `steel_n` in no set (`[project] no_set`), so
/// the borrower's membership changed in the step: it is not known, and the share writes and lists nothing in its step,
/// nor when its plan lands (N8-g). A later step that changes no membership and makes the share newly regular (here
/// the borrower picked PBR, then Default) writes it.
#[test]
fn a_share_replacing_an_automatic_normal_writes_nothing_in_its_step() {
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
    let no_set = |m: &AppModel| m.sets_doc.as_ref().unwrap().no_set();
    assert!(no_set(&m).is_empty(), "precondition");
    share_replacing(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        no_set(&m),
        vec!["armor/steel_n.png".to_string()],
        "precondition: the Replace put the automatic member in no set"
    );
    assert!(
        m.undo_label().is_some_and(|l| l.starts_with("Share into")),
        "precondition: one step"
    );
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "a step that changed the borrower's membership writes nothing"
    );
    assert!(auto_data(&m).is_empty(), "and lists nothing");
    assert_eq!(
        keep_rows(&m, IRON_N),
        1,
        "the landed Replace offers Keep alpha"
    );
    m.set_shader("armor/steel", Some(Shader::Pbr));
    settle(&mut m);
    m.set_shader("armor/steel", Some(Shader::Default));
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
}

/// N8-r: the borrower's own diffuse goes to no set (its view is left with no file, and the listed `data` stays: the
/// step changed its membership); then Back to its automatic set takes it out of no set. The views still show the
/// borrower with no file, but it is not gone: the step that takes a file out of no set knows no set, so it clears
/// nothing, and once the plan lands the borrower reads the shared normal again.
#[test]
fn a_file_back_from_no_set_keeps_the_shared_data() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()], "precondition");
    m.remove_member("armor/steel", "armor/steel.png").unwrap();
    settle(&mut m);
    assert!(
        sdc_gui::model::sets::views(&m)
            .iter()
            .any(|v| v.id == "armor/steel" && v.members.is_empty()),
        "precondition: the borrower has no file"
    );
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: the remove's step changed the borrower's membership and kept it"
    );
    m.back_to_automatic_set(&["armor/steel.png".to_string()])
        .unwrap();
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "a step that takes a file out of no set clears nothing"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
    settle(&mut m);
    assert!(
        sdc_gui::model::sets::views(&m)
            .iter()
            .any(|v| v.id == "armor/steel" && !v.members.is_empty()),
        "the borrower has its file back"
    );
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
}

/// N-10 (a): sharing a PBR set's normal into a regular set's empty Normal slot writes `alpha.policy = "data"` on it in
/// the share's step. (b): the regular set picked PBR takes it back in that step; a regular pick writes it again.
#[test]
fn a_shared_normal_keeps_its_alpha_while_a_regular_set_uses_it() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    assert_eq!(held(&m, IRON_N, "subtype"), Some(json!("pbr_normal")));
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
    assert!(
        m.undo_label().is_some_and(|l| l.starts_with("Share into")),
        "one step"
    );
    assert_eq!(
        m.sets_doc.as_ref().unwrap().auto_data(),
        vec![IRON_N.to_string()]
    );
    let j = m.item(IRON_N).and_then(|i| i.job.clone()).unwrap();
    assert_eq!(j.profile.alpha.policy, AlphaPolicy::Data);
    m.set_shader("armor/steel", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "no regular set uses it now"
    );
    m.set_shader("armor/steel", Some(Shader::Default));
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
}

/// N-10 (b), N8-p: the user's Remove on a listed key takes the entry off in the Remove's own step (its table no longer
/// holds `data`), and a value the user wrote over the step's is never taken back, Data included.
#[test]
fn the_users_alpha_on_a_shared_normal_is_never_taken_back() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: the step wrote it"
    );
    m.set_alpha(IRON_N, AlphaPolicy::Remove);
    assert!(
        m.sets_doc.as_ref().unwrap().auto_data().is_empty(),
        "the entry leaves the list in the Remove's step"
    );
    assert_eq!(m.undo_label(), Some("Alpha"));
    settle(&mut m);
    m.set_alpha(IRON_N, AlphaPolicy::Data);
    settle(&mut m);
    assert!(
        m.sets_doc.as_ref().unwrap().auto_data().is_empty(),
        "the user's Data now"
    );
    m.set_shader("armor/steel", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
}

/// N-34 (review 2 Minor 3): picking Data on a key the share step set to `data` makes it the user's: the key leaves
/// `auto_data` in the pick's step, and the share ending no longer takes it back.
#[test]
fn picking_data_on_a_listed_key_makes_it_the_users() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        m.sets_doc.as_ref().unwrap().auto_data(),
        vec![IRON_N.to_string()],
        "precondition: the share step listed it"
    );
    m.set_alpha(IRON_N, AlphaPolicy::Data);
    settle(&mut m);
    assert!(m.sets_doc.as_ref().unwrap().auto_data().is_empty());
    assert_eq!(m.undo_label(), Some("Alpha"));
    m.set_shader("armor/steel", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
}

/// N-10: a share into another PBR set writes nothing (only ever in PBR sets: dropped, silently).
#[test]
fn a_share_between_pbr_sets_writes_nothing() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    m.set_shader("armor/steel", Some(Shader::Pbr));
    settle(&mut m);
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
}

/// N-10: clearing the regular set's slot 2 ends the last share: `data` goes in that step, and one Undo brings it back.
#[test]
fn clearing_the_share_takes_data_back_in_one_step() {
    use sdc_gui::model::sets::SlotCommand;
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    share(&mut m, IRON_N, "armor/steel");
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
    assert_eq!(m.undo_label(), Some("Clear slot 2 of armor/steel"));
    m.undo();
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
}

/// N-10 (JN1): a two-hop chain (written as a project from before JN3 holds one) from a regular set through a PBR set
/// to the owner is a share into a regular set; clearing the regular set's slot ends it.
#[test]
fn a_chain_into_a_regular_set_keeps_alpha() {
    use sdc_gui::model::sets::SlotCommand;
    let d = tempfile::tempdir().unwrap();
    let mid = d.path().join("textures/mid");
    let plate = d.path().join("textures/plate");
    for f in [&mid, &plate] {
        std::fs::create_dir_all(f).unwrap();
    }
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([70u8, 70, 70]))
        .save(mid.join("bolt.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([100u8, 0, 255]))
        .save(mid.join("bolt_rmaos.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([60u8, 60, 80]))
        .save(plate.join("plate.png"))
        .unwrap();
    let mut m = two_sets(d.path());
    m.edit_docs("seed", None, true, |_, s| {
        s.set_borrow("mid/bolt", 2, Some("pbr/iron"));
        s.set_borrow("plate/plate", 2, Some("mid/bolt"));
    });
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
    m.set_slot_command("plate/plate", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "only a PBR set shares it now"
    );
}

/// N-10: the owner picked regular ends the PBR-into-regular share: `data` goes in the pick's step (and the owner's
/// untouched `pbr_normal` goes back to `normal`, Task 6).
#[test]
fn the_owner_turning_regular_takes_data_back() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    share(&mut m, IRON_N, "armor/steel");
    m.set_shader("pbr/iron", Some(Shader::Default));
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
    assert_eq!(held(&m, IRON_N, "subtype"), Some(json!("normal")));
}

/// Review 2 Important 3: a regular owner whose untouched `normal` a regular set shares; picking PBR on the owner
/// retypes it `pbr_normal` in that step (Task 6), so "already `data`" is read from the step's new type, not the last
/// plan's: `data` is written in the "Shader" step.
#[test]
fn the_owner_turning_pbr_writes_data() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    m.set_shader("pbr/iron", Some(Shader::Default));
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "subtype"), Some(json!("normal")));
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "a regular owner's normal is not written"
    );
    m.set_shader("pbr/iron", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "subtype"), Some(json!("pbr_normal")));
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
    assert_eq!(
        m.sets_doc.as_ref().unwrap().auto_data(),
        vec![IRON_N.to_string()]
    );
    assert_eq!(m.undo_label(), Some("Shader"));
}

/// Review 2 Important 2: removing the owner's Normal-slot file from its set hands it over to the set that shares it
/// (Note JN5), a regular set, so no regular set shares a PBR owner's normal: `data` goes in the remove's step. (The
/// handed `pbr_normal` stays `pbr_normal`: spec limit 2.)
#[test]
fn removing_the_owners_normal_takes_data_back() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    share(&mut m, IRON_N, "armor/steel");
    m.remove_member("pbr/iron", IRON_N).unwrap();
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
    assert!(m.sets_doc.as_ref().unwrap().auto_data().is_empty());
}

/// Review 1 Minor 7: an owner normal already planned `data` (a typed `normal`, whose built-in alpha is `data`) gets no
/// write.
#[test]
fn an_owner_normal_already_data_gets_no_write() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    m.decide(IRON_N, Answer::Subtype(Subtype::Normal));
    settle(&mut m);
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
    assert!(m.sets_doc.as_ref().unwrap().auto_data().is_empty());
    assert_eq!(keep_rows(&m, IRON_N), 0, "the plan already keeps the alpha");
}

/// Review 4 Important 1: an entry whose files are all gone (no files; it borrows `pbr/iron`'s slot 2) reads no
/// texture, so it neither shares the normal nor holds back the clear when the last regular set stops sharing it.
#[test]
fn a_set_with_no_files_does_not_hold_back_a_clear() {
    use sdc_gui::model::sets::SlotCommand;
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    m.edit_docs("seed", None, true, |_, s| {
        s.set_borrow("ghost/ghost", 2, Some("pbr/iron"))
    });
    settle(&mut m);
    assert!(
        sdc_gui::model::sets::views(&m)
            .iter()
            .any(|v| v.id == "ghost/ghost" && v.members.is_empty()),
        "precondition: the entry is shown with no files"
    );
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: the share wrote it"
    );
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
}

/// N-40 (review 5 Minor 1): while a plan is out, a set it does not know yet (here a rename's new id) has no files in
/// its view, but it is not gone: a second user step made before that plan lands keeps the listed `data` that the set's
/// pair holds back.
#[test]
fn a_set_the_running_plan_does_not_know_is_not_gone() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: the share wrote it"
    );
    m.rename_set("armor/steel", "armor/plate").unwrap();
    assert!(
        m.full_plan_pending(),
        "precondition: the rename's plan is still out"
    );
    assert!(
        sdc_gui::model::sets::views(&m)
            .iter()
            .any(|v| v.id == "armor/plate" && v.members.is_empty()),
        "precondition: the new id has no files until the plan lands"
    );
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: the rename's own step holds it back"
    );
    m.set_shader("pbr/iron", Some(Shader::Pbr));
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "a set the running plan does not know yet is not gone"
    );
    settle(&mut m);
}

/// N-39: a shared owner normal whose analysis is known and whose alpha does not vary has no specular to keep: the
/// share writes nothing.
#[test]
fn a_share_of_a_normal_with_no_varying_alpha_writes_nothing() {
    let d = tempfile::tempdir().unwrap();
    open_slot_files(d.path());
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([128u8, 128, 255]))
        .save(d.path().join("textures/pbr/iron_n.png"))
        .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(
        m.item(IRON_N)
            .and_then(|i| i.analysis.as_ref())
            .map(|a| a.alpha != engine::presets::AlphaContent::Varying),
        Some(true),
        "precondition: the analysis is known and the alpha does not vary"
    );
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
    assert!(m.sets_doc.as_ref().unwrap().auto_data().is_empty());
    assert_eq!(keep_rows(&m, IRON_N), 0);
}

/// N-37, a pin of the `edit_docs` read of `automatic_step`: the fuzz switch on open is an automatic step through
/// `edit_docs` (auto_record.rs:234). With accept None the recorder writes nothing, so no batch runs at open and only
/// that read is reached. A saved `auto_data` key that no regular set shares survives the switch; the user's next step
/// clears it.
#[test]
fn an_automatic_step_never_clears_a_leftover() {
    let d = tempfile::tempdir().unwrap();
    let t = d.path().join("textures/m");
    std::fs::create_dir_all(&t).unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
        .save(t.join("rock01.png"))
        .unwrap();
    bumpy_n(&t.join("rock01_n.png"));
    ImageBuffer::from_fn(64, 64, |x, _| Luma([(x * 3) as u8]))
        .save(t.join("rock01_roughness.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |x, _| Rgb([(x * 4) as u8, 200, 40]))
        .save(t.join("rock01_fuzz.png"))
        .unwrap();
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"m/rock01.png\"]\nsubtype = \"skin_albedo\"\n[files.\"m/rock01_n.png\"]\nalpha.policy = \"data\"\n",
    )
    .unwrap();
    std::fs::write(
        d.path().join("textures/sdc-project.toml"),
        "[project]\nauto_data = [\"m/rock01_n.png\"]\n",
    )
    .unwrap();
    let mut m = AppModel::new(sdc_gui::settings::GuiSettings::default(), None, session());
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(d.path().join("textures"));
    settle(&mut m);
    // The switch plans again after its step.
    settle(&mut m);
    assert_eq!(
        held(&m, "m/rock01.png", "subtype"),
        None,
        "precondition: the switch ran"
    );
    assert_eq!(
        held(&m, "m/rock01_n.png", "alpha.policy"),
        Some(json!("data")),
        "no automatic step clears it"
    );
    assert_eq!(
        m.sets_doc.as_ref().unwrap().auto_data(),
        vec!["m/rock01_n.png".to_string()]
    );
    m.set_shader("m/rock01", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(
        held(&m, "m/rock01_n.png", "alpha.policy"),
        None,
        "the user's next step re-checks every listed key"
    );
}

fn auto_data(m: &AppModel) -> Vec<String> {
    m.sets_doc.as_ref().unwrap().auto_data()
}

/// `sdc-project.toml` as saved: `pbr/iron_n.png` listed, and `armor/steel` borrowing `pbr/iron`'s slot 2. Written as a
/// file, not by a seed step: since N8-p a step that runs the follow-on drops an entry whose own table holds no `data`.
const LISTED_AND_BORROWED: &str = "[project]\nauto_data = [\"pbr/iron_n.png\"]\n\n[[sets]]\nid = \"armor/steel\"\nborrow = { slot2 = \"pbr/iron\" }\n";

/// N8-a (Task 8a round 1, finding 1): the user's `alpha.policy = "remove"` under the extension-less spelling and the
/// `subtype` under the full name. The engine applies both tables, so the alpha is the user's: the share writes no
/// `data` in either and lists nothing.
#[test]
fn a_share_writes_no_data_over_the_users_alpha_under_another_spelling() {
    let d = tempfile::tempdir().unwrap();
    open_slot_files(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"pbr/iron_n.png\"]\nsubtype = \"pbr_normal\"\n\n[files.\"pbr/iron_n\"]\nalpha.policy = \"remove\"\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(
        held(&m, "pbr/iron_n", "alpha.policy"),
        Some(json!("remove")),
        "precondition: the user's alpha"
    );
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "precondition: none in the full name's table"
    );
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None, "no data written");
    assert_eq!(
        held(&m, "pbr/iron_n", "alpha.policy"),
        Some(json!("remove"))
    );
    assert!(auto_data(&m).is_empty());
}

/// N8-a: a file whose only `[files]` table is spelled without the extension. The share step's `data` would land in
/// that table, where no later step could tell it from the user's and take it back: the share writes nothing.
#[test]
fn a_share_writes_no_data_into_an_extensionless_table() {
    let d = tempfile::tempdir().unwrap();
    open_slot_files(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"pbr/iron_n\"]\nsubtype = \"pbr_normal\"\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert!(
        !m.project.as_ref().unwrap().has_file_key(IRON_N),
        "precondition: no table under the full name"
    );
    assert_eq!(
        held(&m, IRON_N, "subtype"),
        Some(json!("pbr_normal")),
        "precondition: read through the extension-less spelling"
    );
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        held(&m, "pbr/iron_n", "alpha.policy"),
        None,
        "no data written"
    );
    assert!(auto_data(&m).is_empty());
}

/// N8-a (finding 1): a listed key whose own `[files]` table is gone, with the user's `data` under the extension-less
/// spelling. `file_value` falls back to that table, but it is not the one the share step writes: ending the share
/// leaves the user's `data`.
#[test]
fn ending_a_share_leaves_the_users_data_under_another_spelling() {
    use sdc_gui::model::sets::SlotCommand;
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"pbr/iron_n\"]\nalpha.policy = \"data\"\n",
    )
    .unwrap();
    std::fs::write(
        d.path().join("textures/sdc-project.toml"),
        LISTED_AND_BORROWED,
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    settle(&mut m);
    assert!(
        !m.project.as_ref().unwrap().has_file_key(IRON_N),
        "precondition: no table under the full name"
    );
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: read through the extension-less spelling"
    );
    assert_eq!(
        auto_data(&m),
        vec![IRON_N.to_string()],
        "precondition: listed"
    );
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    assert_eq!(
        held(&m, "pbr/iron_n", "alpha.policy"),
        Some(json!("data")),
        "the user's data stays"
    );
}

/// N8-b (finding 2, JN1 own-first): regular `armor/steel` borrows slot 2 from PBR `pbr/iron`, which owns `iron_n` and
/// also borrows `pbr/gold`'s. Steel reads iron's own normal, as the set resolver fills it: `iron_n` gets `data` and is
/// listed, `gold_n` is untouched.
#[test]
fn an_own_first_chain_writes_the_file_the_regular_set_reads() {
    use sdc_gui::model::sets::{SlotFill, SlotKind};
    const GOLD_N: &str = "pbr/gold_n.png";
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    let pbr = d.path().join("textures/pbr");
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([200u8, 160, 40]))
        .save(pbr.join("gold.png"))
        .unwrap();
    bumpy_n(&pbr.join("gold_n.png"));
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([100u8, 0, 255]))
        .save(pbr.join("gold_rmaos.png"))
        .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.edit_docs("seed", None, true, |_, s| {
        s.set_borrow("pbr/iron", 2, Some("pbr/gold"));
        s.set_borrow("armor/steel", 2, Some("pbr/iron"));
    });
    settle(&mut m);
    let views = sdc_gui::model::sets::set_views(&m);
    let normal = |id: &str| {
        views
            .iter()
            .find(|v| v.id == id)
            .and_then(|v| v.slots.get(&SlotKind::Normal).cloned())
    };
    assert_eq!(
        normal("armor/steel"),
        Some(SlotFill::Borrowed {
            from: "pbr/iron".into(),
            item: IRON_N.into()
        }),
        "precondition: steel reads iron's own normal"
    );
    assert_eq!(
        normal("pbr/iron"),
        Some(SlotFill::Borrowed {
            from: "pbr/gold".into(),
            item: GOLD_N.into()
        }),
        "precondition: iron's own card shows its borrow"
    );
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
    assert_eq!(held(&m, GOLD_N, "alpha.policy"), None);
}

/// N8-b (finding 3): renaming the LENDER retargets the regular set's borrow to an id the running plan does not know
/// yet, so the step cannot tell which file the borrow reads: it clears nothing. The `data` and its list entry survive
/// the rename's step and the plan that lands after it.
#[test]
fn renaming_the_lender_keeps_the_shared_normals_data() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: the share wrote it"
    );
    m.rename_set("pbr/iron", "pbr/plate").unwrap();
    assert!(
        m.full_plan_pending(),
        "precondition: the rename's plan is still out"
    );
    assert!(
        sdc_gui::model::sets::views(&m)
            .iter()
            .any(|v| v.id == "pbr/plate" && v.members.is_empty()),
        "precondition: the new id has no files until the plan lands"
    );
    assert!(
        m.sets_doc
            .as_ref()
            .unwrap()
            .entries()
            .iter()
            .any(|e| e.id == "armor/steel"
                && e.borrow.get(&2).map(String::as_str) == Some("pbr/plate")),
        "precondition: the borrow follows the rename"
    );
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "the rename's step keeps it"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()], "and its entry");
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "after the plan lands"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
}

/// N8-b (finding 3, the automatic rename): a new input name renames the lender's automatic set. `retarget_auto` moves
/// the regular set's borrow to the new id, which has no entry and no view until the plan lands. A user step made
/// before then cannot tell which file the borrow reads: it clears nothing, and after the plan the share holds again.
#[test]
fn an_automatic_rename_of_the_lender_keeps_the_shared_normals_data() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: the share wrote it"
    );
    m.set_input_name("pbr/iron", "textures/pbr/plate").unwrap();
    assert!(
        m.full_plan_pending(),
        "precondition: the rename's plan is still out"
    );
    let entries = m.sets_doc.as_ref().unwrap().entries();
    assert!(
        entries
            .iter()
            .any(|e| e.id == "armor/steel"
                && e.borrow.get(&2).map(String::as_str) == Some("pbr/plate")),
        "precondition: the borrow follows the automatic id"
    );
    assert!(
        !entries.iter().any(|e| e.id == "pbr/plate"),
        "precondition: no entry under the new id"
    );
    assert!(
        !sdc_gui::model::sets::views(&m)
            .iter()
            .any(|v| v.id == "pbr/plate"),
        "precondition: no view under the new id until the plan lands"
    );
    m.set_shader("armor/steel", Some(Shader::Default));
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "a step before the plan lands keeps it"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
    settle(&mut m);
    m.set_shader("armor/steel", None);
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "after the plan the share holds"
    );
}

/// N8-b (review Q6): an explicit PBR owner and an explicit PBR borrower. The owner's `pbr_normal` is retyped `height`,
/// then the borrower is picked Default before the type's plan lands. The last plan still puts the file in the Normal
/// slot, but the document says it is a height map now: it gets no `data`.
#[test]
fn a_normal_retyped_before_its_plan_lands_gets_no_data() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    m.set_shader("pbr/iron", Some(Shader::Pbr));
    settle(&mut m);
    m.set_shader("armor/steel", Some(Shader::Pbr));
    settle(&mut m);
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "precondition: a share between PBR sets writes nothing"
    );
    m.set_field(IRON_N, "subtype", json!("height"));
    assert_eq!(
        m.item(IRON_N)
            .and_then(|i| i.job.as_ref())
            .map(|j| j.profile.subtype),
        Some(Subtype::PbrNormal),
        "precondition: the type's plan has not landed"
    );
    assert_eq!(
        normal_owner(&m, "pbr/iron").as_deref(),
        Some(IRON_N),
        "precondition: the last plan still fills the Normal slot with it"
    );
    m.set_shader("armor/steel", Some(Shader::Default));
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "the retyped file gets no data"
    );
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
}

/// N8-c (finding 5): as saved, a listed key with no alpha (reset), retyped as a diffuse (so no set's Normal slot holds
/// it), which `armor/steel` borrows; the user's first step confirms Data on it: that `data` is the user's. It stays
/// after the step and after a later step that ends the share, and the key is off the list. (Written as files: since
/// N8-p a step that runs the follow-on drops such an entry, so only a project opened so reaches the Data pick with it.)
#[test]
fn a_confirmed_data_on_a_listed_key_is_the_users() {
    use sdc_gui::model::sets::SlotCommand;
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    let t = d.path().join("textures");
    std::fs::write(
        t.join("sdc.toml"),
        "[files.\"pbr/iron_n.png\"]\nsubtype = \"diffuse\"\n",
    )
    .unwrap();
    std::fs::write(t.join("sdc-project.toml"), LISTED_AND_BORROWED).unwrap();
    let mut m = opened_as_the_app(t, d.path().join("export"));
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "precondition: reset"
    );
    assert_eq!(
        auto_data(&m),
        vec![IRON_N.to_string()],
        "precondition: listed"
    );
    assert_ne!(
        normal_owner(&m, "pbr/iron").as_deref(),
        Some(IRON_N),
        "precondition: the file left the Normal slot"
    );
    m.decide(IRON_N, Answer::Alpha(AlphaPolicy::Data));
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "the Data the user confirmed"
    );
    assert!(auto_data(&m).is_empty(), "the user's now");
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "ending the share leaves it"
    );
}

/// N8-d (finding 6): `sdc-project.toml` holds the valid but unusable `project = "bad"`, so `auto_data` cannot be
/// written. The value and its entry go together or not at all: the share writes no `data` and lists nothing.
#[test]
fn a_share_that_cannot_list_writes_no_data() {
    use sdc_gui::model::sets::{SlotFill, SlotKind};
    let d = tempfile::tempdir().unwrap();
    open_slot_files(d.path());
    std::fs::write(
        d.path().join("textures/sdc-project.toml"),
        "project = \"bad\"\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert!(
        !m.sets_doc.as_ref().unwrap().read_only(),
        "precondition: the file opens for editing"
    );
    share(&mut m, IRON_N, "armor/steel");
    assert!(
        matches!(
            sdc_gui::model::sets::set_views(&m)
                .iter()
                .find(|v| v.id == "armor/steel")
                .and_then(|v| v.slots.get(&SlotKind::Normal)),
            Some(SlotFill::Borrowed { item, .. }) if item == IRON_N
        ),
        "precondition: the share happened"
    );
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "no data without its entry"
    );
    assert!(auto_data(&m).is_empty());
}

/// N8-d (finding 7): with an unshared listed `data` present, a user's edit that writes the value already held changes
/// no document: it runs no cleanup and adds no Undo step.
#[test]
fn an_edit_that_writes_the_held_value_clears_nothing() {
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"pbr/iron_n.png\"]\nalpha.policy = \"data\"\n\n[files.\"armor/steel.png\"]\nalpha.policy = \"remove\"\n",
    )
    .unwrap();
    std::fs::write(
        d.path().join("textures/sdc-project.toml"),
        "[project]\nauto_data = [\"pbr/iron_n.png\"]\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: the leftover"
    );
    assert_eq!(
        auto_data(&m),
        vec![IRON_N.to_string()],
        "precondition: listed"
    );
    let label = m.undo_label().map(str::to_string);
    m.set_alpha("armor/steel.png", AlphaPolicy::Remove);
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "an equal write clears nothing"
    );
    assert_eq!(
        m.undo_label().map(str::to_string),
        label,
        "and adds no Undo step"
    );
}

/// `two_sets_files` with the user's `[files]` table `table` holding `alpha.policy = "data"` for `IRON_N`, which is
/// listed although its own lower-case table is absent; `armor/steel` borrows it, then the share ends.
fn ending_the_share_over(table: &str) -> (tempfile::TempDir, AppModel) {
    use sdc_gui::model::sets::SlotCommand;
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        format!("[files.'{table}']\nalpha.policy = \"data\"\n"),
    )
    .unwrap();
    std::fs::write(
        d.path().join("textures/sdc-project.toml"),
        LISTED_AND_BORROWED,
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    settle(&mut m);
    assert!(
        !m.project.as_ref().unwrap().has_file_key(IRON_N),
        "precondition: no table spelled as the list entry"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()], "precondition");
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    (d, m)
}

/// N8-e (round 2, B1): the list entry names the exact table the share step wrote. A case twin of it holding the
/// user's `data` is not that table: ending the share clears nothing.
#[test]
fn ending_a_share_leaves_the_users_data_in_a_case_twin() {
    let (_d, m) = ending_the_share_over("PBR/IRON_N.PNG");
    assert_eq!(
        held(&m, "PBR/IRON_N.PNG", "alpha.policy"),
        Some(json!("data")),
        "the user's data in the case twin stays"
    );
}

/// N8-e (B1): as above, with a twin that differs from the listed spelling only by Unicode case.
#[test]
fn ending_a_share_leaves_the_users_data_in_a_unicode_twin() {
    use sdc_gui::model::sets::SlotCommand;
    const ARZ_N: &str = "pbr/ärz_n.png";
    let d = tempfile::tempdir().unwrap();
    let pbr = d.path().join("textures/pbr");
    let armor = d.path().join("textures/armor");
    for f in [&pbr, &armor] {
        std::fs::create_dir_all(f).unwrap();
    }
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([120u8, 90, 60]))
        .save(pbr.join("ärz.png"))
        .unwrap();
    bumpy_n(&pbr.join("ärz_n.png"));
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([100u8, 0, 255]))
        .save(pbr.join("ärz_rmaos.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([90u8, 90, 100]))
        .save(armor.join("steel.png"))
        .unwrap();
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.'PBR/ÄRZ_N.PNG']\nalpha.policy = \"data\"\n",
    )
    .unwrap();
    std::fs::write(
        d.path().join("textures/sdc-project.toml"),
        "[project]\nauto_data = [\"pbr/ärz_n.png\"]\n\n[[sets]]\nid = \"armor/steel\"\nborrow = { slot2 = \"pbr/ärz\" }\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    settle(&mut m);
    assert!(
        !m.project.as_ref().unwrap().has_file_key(ARZ_N),
        "precondition: no table spelled as the list entry"
    );
    assert_eq!(auto_data(&m), vec![ARZ_N.to_string()], "precondition");
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    assert_eq!(
        held(&m, "PBR/ÄRZ_N.PNG", "alpha.policy"),
        Some(json!("data")),
        "the user's data in the Unicode twin stays"
    );
}

/// N8-e (B1), a pin: a `[files]` key with a literal backslash names no file (the engine matches `/` only), so the
/// share writes and lists the slash-form table, and ending the share clears that one and leaves the other alone.
#[test]
fn a_backslash_table_beside_the_shares_table_is_left_alone() {
    use sdc_gui::model::sets::SlotCommand;
    let d = tempfile::tempdir().unwrap();
    open_slot_files(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.'pbr\\iron_n.png']\nalpha.policy = \"remove\"\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
    assert_eq!(
        held(&m, "pbr\\iron_n.png", "alpha.policy"),
        Some(json!("remove"))
    );
}

/// N8-e (B5, amends N6-j): a listed key whose own table is gone, with the user's `data` in the extension-less table:
/// that `data` is the user's for Task 6 too, so a shader pick neither retypes nor clears the normal's sub-type.
#[test]
fn a_users_stem_data_keeps_the_normal_out_of_shader_picks() {
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"pbr/iron_n\"]\nalpha.policy = \"data\"\n",
    )
    .unwrap();
    std::fs::write(
        d.path().join("textures/sdc-project.toml"),
        "[project]\nauto_data = [\"pbr/iron_n.png\"]\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    let typed = held(&m, IRON_N, "subtype");
    m.set_shader("pbr/iron", Some(Shader::Default));
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "subtype"),
        typed,
        "the pick leaves the type"
    );
    assert_eq!(held(&m, "pbr/iron_n", "alpha.policy"), Some(json!("data")));
}

/// Drop `item` on set `into` as the Sets column does, without waiting for the plan.
fn drop_now(m: &mut AppModel, item: &str, into: &str, choice: sdc_gui::model::sets::DropChoice) {
    use sdc_gui::model::sets::DropOutcome;
    match m.drop_choice(item, into, choice) {
        DropOutcome::AskReplace { kind, existing } => m
            .drop_replace(item, into, choice, kind, &existing)
            .expect("dropped"),
        DropOutcome::Joined => {}
        other => panic!("{other:?}"),
    }
}

/// `two_sets` plus a populated PBR set `pbr/gold` (`gold.png`, `gold_rmaos.png`, and `gold_n.png` when `with_n`);
/// `iron_n` is shared into `armor/steel` and listed.
fn shared_with_gold(dir: &Path, with_n: bool) -> AppModel {
    open_slot_files(dir);
    let pbr = dir.join("textures/pbr");
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([200u8, 160, 40]))
        .save(pbr.join("gold.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([100u8, 0, 255]))
        .save(pbr.join("gold_rmaos.png"))
        .unwrap();
    if with_n {
        bumpy_n(&pbr.join("gold_n.png"));
    }
    let mut m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: the share wrote it"
    );
    m
}

/// N8-f (round 2, B2): the lent normal moves into another populated PBR set whose Normal slot is empty, and the
/// regular set's borrow follows it. The last plan does not have the file in that set yet, so the step cannot name
/// what the borrow reads: the listed `data` stays, and the share holds after the plan.
#[test]
fn moving_a_lent_normal_between_pbr_sets_keeps_its_data() {
    use sdc_gui::model::sets::DropChoice;
    let d = tempfile::tempdir().unwrap();
    let mut m = shared_with_gold(d.path(), false);
    drop_now(&mut m, IRON_N, "pbr/gold", DropChoice::Move);
    assert!(
        m.sets_doc
            .as_ref()
            .unwrap()
            .entries()
            .iter()
            .any(|e| e.id == "armor/steel"
                && e.borrow.get(&2).map(String::as_str) == Some("pbr/gold")),
        "precondition: the borrow follows the file"
    );
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "the move's step keeps it"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
}

/// N8-f (B2): as above with Replace: `pbr/gold` has its own normal, which the move displaces. The listed `data` stays
/// and the displaced normal gets none.
#[test]
fn replacing_a_normal_with_a_lent_one_writes_and_clears_nothing() {
    use sdc_gui::model::sets::DropChoice;
    let d = tempfile::tempdir().unwrap();
    let mut m = shared_with_gold(d.path(), true);
    drop_now(&mut m, IRON_N, "pbr/gold", DropChoice::Move);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "the replace's step keeps it"
    );
    assert_eq!(
        held(&m, "pbr/gold_n.png", "alpha.policy"),
        None,
        "the displaced normal gets no data"
    );
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
    assert_eq!(held(&m, "pbr/gold_n.png", "alpha.policy"), None);
}

/// N8-f (round 2, fresh): after the move, and before its plan lands, the user makes an unrelated `[files]` edit. The
/// views still put `iron_n` in `pbr/iron`, and nothing in that step changes membership, so only the stale-plan check
/// stops the step from reading "`pbr/gold` has no normal" and clearing the shared normal's `data`.
#[test]
fn a_step_before_the_moves_plan_lands_keeps_the_data() {
    use sdc_gui::model::sets::DropChoice;
    let d = tempfile::tempdir().unwrap();
    let mut m = shared_with_gold(d.path(), false);
    drop_now(&mut m, IRON_N, "pbr/gold", DropChoice::Move);
    m.edit_docs("unrelated", None, true, |p, _| {
        p.set_file_value("pbr/gold.png", "alpha.policy", &json!("remove"))
    });
    assert_eq!(
        held(&m, "pbr/gold.png", "alpha.policy"),
        Some(json!("remove")),
        "precondition: the edit landed"
    );
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "a step on a stale plan keeps it"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
}

/// N8-f (B2): one step makes `iron_n` shared by a known regular set and by a set the step creates (no files yet, so
/// unsure). A key any borrower makes unsure is neither written nor cleared.
#[test]
fn an_unsure_key_is_not_written() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    m.edit_docs("seed", None, true, |_, s| {
        s.set_borrow("ghost/new", 2, Some("pbr/iron"));
        s.set_borrow("armor/steel", 2, Some("pbr/iron"));
    });
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "an unsure key gets no data"
    );
    settle(&mut m);
    // While the next plan is out the empty borrower is unsure again, but the regular borrow is known.
    m.replan();
    assert!(m.full_plan_pending());
    assert_eq!(
        keep_rows(&m, IRON_N),
        1,
        "the known borrower still reads the normal"
    );
    settle(&mut m);
}

/// N8-p: Reset all drops the absent full-name entry and leaves the stem table's Data in place.
#[test]
fn a_reset_that_uncovers_data_drops_the_stale_entry_in_the_same_step() {
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"pbr/iron_n.png\"]\nalpha.policy = \"remove\"\n\n[files.\"pbr/iron_n\"]\nalpha.policy = \"data\"\n",
    )
    .unwrap();
    std::fs::write(
        d.path().join("textures/sdc-project.toml"),
        "[project]\nauto_data = [\"pbr/iron_n.png\"]\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("remove")),
        "precondition"
    );
    m.reset_all_settings(&[IRON_N.to_string()]);
    settle(&mut m);
    assert!(
        !m.project.as_ref().unwrap().has_file_key(IRON_N),
        "precondition: the reset removed the table"
    );
    assert_eq!(held(&m, "pbr/iron_n", "alpha.policy"), Some(json!("data")));
    assert!(auto_data(&m).is_empty(), "Reset all drops the stale entry");
    m.undo();
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("remove")));
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
}

/// N8-h: removing a full-name table uncovers the stem's existing Data. Neither table gained Data.
#[test]
fn a_reset_that_uncovers_data_keeps_the_entry_while_a_set_plan_is_pending() {
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    let root = d.path().join("textures");
    std::fs::write(
        root.join("sdc.toml"),
        "[files.\"pbr/iron_n.png\"]\nalpha.policy = \"remove\"\n\n[files.\"pbr/iron_n\"]\nalpha.policy = \"data\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("sdc-project.toml"),
        "[project]\nauto_data = [\"pbr/iron_n.png\"]\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(root, d.path().join("export"));
    m.rename_set("armor/steel", "armor/plate").unwrap();
    settle(&mut m);
    // Undo restores the recorded list and starts a plan for the restored set name.
    m.undo();
    assert!(m.full_plan_pending());
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("remove")));
    assert_eq!(held(&m, "pbr/iron_n", "alpha.policy"), Some(json!("data")));

    m.reset_all_settings(&[IRON_N.to_string()]);
    assert!(!m.project.as_ref().unwrap().has_file_key(IRON_N));
    assert_eq!(held(&m, "pbr/iron_n", "alpha.policy"), Some(json!("data")));
    assert_eq!(
        auto_data(&m),
        vec![IRON_N.to_string()],
        "uncovering existing Data leaves the full-name entry listed"
    );
}

/// N8-h (B4), a pin of the batch compare: a bulk Data on a six-file cube is copied to its other face entries in the
/// same batch (DT31); a listed secondary face that the copy wrote `data` into leaves the list.
#[test]
fn a_six_face_copy_onto_a_listed_face_unlists_it() {
    use sdc_gui::model::bulk::BulkOp;
    const NX: &str = "a/chrome_e_nx.png";
    let d = tempfile::tempdir().unwrap();
    let r = d.path().join("textures/a");
    std::fs::create_dir_all(&r).unwrap();
    for (i, f) in ["px", "nx", "py", "ny", "pz", "nz"].iter().enumerate() {
        image::RgbImage::from_pixel(16, 16, image::Rgb([30 + 40 * i as u8, 20, 10]))
            .save(r.join(format!("chrome_e_{f}.png")))
            .unwrap();
    }
    std::fs::write(
        d.path().join("textures/sdc-project.toml"),
        "[project]\nauto_data = [\"a/chrome_e_nx.png\"]\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    let key = m
        .items
        .iter()
        .find(|i| i.job.is_some())
        .expect("the six-face item")
        .key
        .clone();
    assert_ne!(key, NX, "precondition: the face is a secondary one");
    m.decide(&key, Answer::Subtype(Subtype::Cubemap));
    settle(&mut m);
    m.apply_bulk_edit(
        std::slice::from_ref(&key),
        "alpha.policy",
        BulkOp::Set(json!("data")),
    );
    settle(&mut m);
    assert_eq!(
        held(&m, NX, "alpha.policy"),
        Some(json!("data")),
        "precondition: the copy reached the listed face"
    );
    assert!(auto_data(&m).is_empty(), "the user's batch wrote it");
}

fn aged(p: &Path) {
    std::fs::File::options()
        .write(true)
        .open(p)
        .unwrap()
        .set_modified(std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(86_400))
        .unwrap();
}

/// N8-h (B3): a crash restore is no user alpha edit. Saved: a listed key with its alpha reset. Unsaved at the crash:
/// a later share wrote the automatic `data` back. The restore keeps the list as the snapshot had it, so ending the
/// share later takes the `data` back.
#[test]
fn a_crash_restore_keeps_the_list_it_restores() {
    use sdc_gui::model::sets::SlotCommand;
    let d = tempfile::tempdir().unwrap();
    open_slot_files(d.path());
    // As saved (written as a file: since N8-p a step that runs the follow-on drops such an entry).
    std::fs::write(
        d.path().join("textures/sdc-project.toml"),
        "[project]\nauto_data = [\"pbr/iron_n.png\"]\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "precondition: reset"
    );
    assert_eq!(
        auto_data(&m),
        vec![IRON_N.to_string()],
        "precondition: listed, reset"
    );
    m.save().unwrap();
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: the share wrote it again"
    );
    sdc_gui::crash_log::write_recovery(&m.recovery_changed().unwrap_or_default());
    drop(m);
    let t = d.path().join("textures");
    aged(&t.join("sdc.toml"));
    aged(&t.join("sdc-project.toml"));
    let mut m = opened_as_the_app(t, d.path().join("export"));
    assert_eq!(
        m.undo_label(),
        Some("Restore crash edits"),
        "precondition: restored"
    );
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
    assert_eq!(
        auto_data(&m),
        vec![IRON_N.to_string()],
        "the restore keeps the record"
    );
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "ending the share takes it back"
    );
}

/// N8-i (round 2, B7): the share step's file write fails after the list was written: the list goes back to exactly
/// what it was, a key already listed after a reset and a stale entry included.
#[test]
fn a_failed_write_leaves_the_list_as_it_was() {
    use sdc_gui::model::sets::SlotCommand;
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    share(&mut m, IRON_N, "armor/steel");
    m.clear_alpha(IRON_N);
    settle(&mut m);
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    let start = vec![IRON_N.to_string(), "x/stale.png".to_string()];
    let l = start.clone();
    // No plan, so no follow-on: since N8-p one would drop both entries (neither table holds `data`).
    m.edit_docs("seed", None, false, move |_, s| s.set_auto_data(&l));
    settle(&mut m);
    assert_eq!(auto_data(&m), start, "precondition");
    m.fail_shared_alpha_write = true;
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "precondition: the write failed"
    );
    assert_eq!(auto_data(&m), start, "the list as it was");
}

/// The share's table `pbr/iron_n.png` holds `data`, listed twice in `auto_data`, and the user's `twin` table (another
/// spelling of the same file) holds `remove`; `armor/steel` borrows `pbr/iron`'s normal. Then the share ends.
fn ending_a_twice_listed_share_beside(twin: &str) -> (tempfile::TempDir, AppModel) {
    use sdc_gui::model::sets::SlotCommand;
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        format!(
            "[files.'pbr/iron_n.png']\nalpha.policy = \"data\"\n\n[files.'{twin}']\nalpha.policy = \"remove\"\n"
        ),
    )
    .unwrap();
    std::fs::write(
        d.path().join("textures/sdc-project.toml"),
        "[project]\nauto_data = [\"pbr/iron_n.png\", \"pbr/iron_n.png\"]\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.edit_docs("seed", None, true, |_, s| {
        s.set_borrow("armor/steel", 2, Some("pbr/iron"))
    });
    settle(&mut m);
    // The reviewer's "leave only Data in the listed table": the user takes out the sub-type the recorder wrote, so
    // clearing the `data` empties the table and removes it.
    m.edit_docs("user", None, true, |p, _| {
        p.clear_file_value(IRON_N, "subtype")
    });
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "subtype"),
        None,
        "precondition: the share's table holds only its data"
    );
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: the share's table holds data"
    );
    assert_eq!(auto_data(&m).len(), 2, "precondition: listed twice");
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    (d, m)
}

/// N8-k (round 3, B1): a repeated list entry is one key and its clear is exact and done once: the share's own table
/// loses its `data`, and the user's upper-case twin keeps its `remove`.
#[test]
fn a_twice_listed_clear_leaves_the_users_case_twin() {
    let (_d, m) = ending_a_twice_listed_share_beside("PBR/IRON_N.PNG");
    assert!(
        !m.project.as_ref().unwrap().has_file_key(IRON_N),
        "the share's own table is cleared"
    );
    assert_eq!(
        held(&m, "PBR/IRON_N.PNG", "alpha.policy"),
        Some(json!("remove")),
        "the user's case twin keeps its value"
    );
}

/// N8-k (B1): as above, with the user's extension-less table.
#[test]
fn a_twice_listed_clear_leaves_the_users_stem_twin() {
    let (_d, m) = ending_a_twice_listed_share_beside("pbr/iron_n");
    assert!(
        !m.project.as_ref().unwrap().has_file_key(IRON_N),
        "the share's own table is cleared"
    );
    assert_eq!(
        held(&m, "pbr/iron_n", "alpha.policy"),
        Some(json!("remove")),
        "the user's stem twin keeps its value"
    );
}

/// The reviewer's N8-m fixture: a set the user created (New set from `armor/steel.png`, a diffuse only, its shader
/// chosen Default) borrows `pbr/iron`'s normal, which the share listed; then the diffuse is gone from disk, the project
/// is reopened and settles, and an unrelated diffuse `armor/bronze.png` is in the folder. The borrower is left with no
/// file of its own and still points at the listed normal. Returns the borrower's id.
fn emptied_borrower(dir: &Path) -> (AppModel, String) {
    emptied_borrower_with(dir, false)
}

/// `emptied_borrower`; with `twice`, a second `[[sets]]` entry carrying only the borrower's id is appended by hand
/// before the reopen (the GUI edits the first entry of an id, the engine reads every one).
fn emptied_borrower_with(dir: &Path, twice: bool) -> (AppModel, String) {
    two_sets_files(dir);
    let armor = dir.join("textures/armor");
    std::fs::remove_file(armor.join("steel_n.png")).unwrap();
    let mut m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    let id = m.drop_new_set("armor/steel.png").expect("a new set");
    settle(&mut m);
    m.set_shader(&id, Some(Shader::Default));
    settle(&mut m);
    share(&mut m, IRON_N, &id);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: the share wrote it"
    );
    settle(&mut m);
    m.save().expect("saved");
    drop(m);
    std::fs::remove_file(armor.join("steel.png")).unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([150u8, 110, 60]))
        .save(armor.join("bronze.png"))
        .unwrap();
    if twice {
        let f = dir.join("textures/sdc-project.toml");
        let mut text = std::fs::read_to_string(&f).unwrap();
        let _ = write!(text, "\n[[sets]]\nid = \"{id}\"\n");
        std::fs::write(&f, text).unwrap();
    }
    let mut m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    settle(&mut m);
    assert!(
        sdc_gui::model::sets::views(&m)
            .iter()
            .any(|v| v.id == id && v.members.is_empty()),
        "precondition: the borrower has no file"
    );
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: still data"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()], "precondition");
    (m, id)
}

/// N8-m (round 3, B3): a diffuse duplicated into the emptied borrower gives it its first file in a step whose views
/// still show it empty. Its membership changed, so it is not gone: the listed `data` is not cleared in that step, and
/// the share is live once the plan lands.
#[test]
fn a_duplicate_into_an_emptied_borrower_keeps_the_data() {
    use sdc_gui::model::sets::DropChoice;
    let d = tempfile::tempdir().unwrap();
    let (mut m, id) = emptied_borrower(d.path());
    drop_now(&mut m, "armor/bronze.png", &id, DropChoice::Duplicate);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "a set whose membership the step changed is not gone"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
}

/// N8-u (round 5, the reviewer's sequence): as `a_move_into_an_emptied_borrower_keeps_the_data`, with a second
/// `[[sets]]` entry under the borrower's id written by hand. The Move edits the first entry; the comparison keeps every
/// entry of an id, so the borrower is still a set whose membership the step changed, and the `data` stays.
#[test]
fn a_move_into_an_emptied_borrower_with_a_second_entry_keeps_the_data() {
    use sdc_gui::model::sets::DropChoice;
    let d = tempfile::tempdir().unwrap();
    let (mut m, id) = emptied_borrower_with(d.path(), true);
    let rows = |m: &AppModel| {
        m.sets_doc
            .as_ref()
            .unwrap()
            .entries()
            .iter()
            .filter(|e| e.id == id)
            .count()
    };
    assert_eq!(rows(&m), 2, "precondition: two entries under the id");
    drop_now(&mut m, "armor/bronze.png", &id, DropChoice::Move);
    assert!(
        m.sets_doc
            .as_ref()
            .unwrap()
            .entries()
            .iter()
            .any(|e| e.id == id && e.add.iter().any(|f| f == "armor/bronze.png")),
        "precondition: the Move wrote an add line"
    );
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "a set whose membership the step changed is not gone"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
}

/// N8-m (B3): as `a_duplicate_into_an_emptied_borrower_keeps_the_data`, with the diffuse moved in.
#[test]
fn a_move_into_an_emptied_borrower_keeps_the_data() {
    use sdc_gui::model::sets::DropChoice;
    let d = tempfile::tempdir().unwrap();
    let (mut m, id) = emptied_borrower(d.path());
    drop_now(&mut m, "armor/bronze.png", &id, DropChoice::Move);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "a set whose membership the step changed is not gone"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
}

/// N8-m control: an ordinary Share into the empty Normal slot of a populated borrower changes only its borrow line,
/// so the borrower stays known and the share writes `data` in its own step.
#[test]
fn an_ordinary_share_into_an_empty_slot_writes_in_its_step() {
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    std::fs::remove_file(d.path().join("textures/armor/steel_n.png")).unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    settle(&mut m);
    let lines = |m: &AppModel| {
        m.sets_doc
            .as_ref()
            .unwrap()
            .entries()
            .iter()
            .find(|e| e.id == "armor/steel")
            .map(|e| (e.add.clone(), e.remove.clone()))
            .unwrap_or_default()
    };
    let before = lines(&m);
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        lines(&m),
        before,
        "precondition: only the borrow line changed"
    );
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "the ordinary share writes in its step"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
}

/// N8-n (round 3, B2): `fresh` holds while a `[files]`-only edit's full plan is out, and that plan may give an empty
/// set its files back. The emptied borrower still points at the listed normal; `armor/bronze` shares it too. An
/// unrelated `[files]` edit starts a full plan, and while it is out the user ends `armor/bronze`'s share, the last one a
/// populated regular set holds. The empty borrower is not gone while a plan is out, so the normal stays unsure and its
/// `data` stays.
#[test]
fn an_empty_borrower_is_not_gone_while_a_files_plan_is_out() {
    use sdc_gui::model::sets::SlotCommand;
    let d = tempfile::tempdir().unwrap();
    let (mut m, _id) = emptied_borrower(d.path());
    share(&mut m, IRON_N, "armor/bronze");
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: still data"
    );
    m.edit_docs("unrelated", None, true, |p, _| {
        p.set_file_value("armor/bronze.png", "alpha.policy", &json!("remove"))
    });
    assert!(
        m.full_plan_pending(),
        "precondition: the files edit's full plan is out"
    );
    m.set_slot_command("armor/bronze", 2, SlotCommand::Clear)
        .unwrap();
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "an empty set is not gone while a plan is out"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
    settle(&mut m);
}

/// N8-o (round 3, B5): one setting changed in a settled cs project with 4,000 `[files]` tables and 1,000 listed keys
/// walks the `[files]` keys at most once per document state (before the edit, after it): the list reader, the
/// unlisting and the follow-on share the two indexes. `armor/steel_n.png`'s table is spelled in upper case, so a
/// per-owner read of its sub-type through the document's resolver would miss the exact spelling and walk every key; it
/// goes through the index instead. The real step runs: the user's edit, then the follow-on, which clears the listed
/// keys no regular set shares.
#[test]
fn a_step_walks_the_files_keys_once_per_document_state() {
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    let mut toml = String::from("[files.'ARMOR/STEEL_N.PNG']\nsubtype = \"normal\"\n\n");
    let mut listed = Vec::new();
    for i in 0..4_000 {
        let _ = write!(
            toml,
            "[files.\"old/f{i:04}.png\"]\nalpha.policy = \"data\"\n\n"
        );
        if i % 4 == 0 {
            listed.push(format!("\"old/f{i:04}.png\""));
        }
    }
    std::fs::write(d.path().join("textures/sdc.toml"), toml).unwrap();
    std::fs::write(
        d.path().join("textures/sdc-project.toml"),
        format!("[project]\nauto_data = [{}]\n", listed.join(", ")),
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    settle(&mut m);
    assert_eq!(auto_data(&m).len(), 1_000, "precondition: a long list");
    assert!(
        !m.project
            .as_ref()
            .unwrap()
            .has_file_key("armor/steel_n.png"),
        "precondition: the owner normal's table is spelled in another case"
    );
    let start = sdc_gui::project::key_walks();
    m.edit_docs("user", None, true, |p, _| {
        p.set_file_value("armor/steel.png", "alpha.policy", &json!("remove"))
    });
    let walks = sdc_gui::project::key_walks() - start;
    assert!(
        auto_data(&m).is_empty(),
        "precondition: the follow-on ran and cleared the listed keys"
    );
    assert!(walks <= 2, "{walks} walks of the [files] keys in one step");
}

/// N8-k: `ProjectDoc::clear_exact_file_value` touches only the table spelled byte for byte as the key, and only while
/// it holds the value: no case twin, no extension-less table, no other value.
#[test]
fn the_exact_remover_touches_only_its_own_table_while_it_holds_the_value() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("sdc.toml"),
        "[files.'PBR/IRON_N.PNG']\nalpha.policy = \"data\"\n\n[files.'pbr/iron_n']\nalpha.policy = \"data\"\n\n[files.'pbr/iron_n.png']\nalpha.policy = \"remove\"\nsubtype = \"pbr_normal\"\n",
    )
    .unwrap();
    let mut p = sdc_gui::project::ProjectDoc::open(d.path());
    assert!(!p.read_only(), "{:?}", p.error());
    let start = p.text();
    let data = json!("data");
    assert!(
        !p.clear_exact_file_value("Pbr/Iron_N.png", "alpha.policy", &data),
        "no table spelled so: nothing, no twin"
    );
    assert!(
        !p.clear_exact_file_value("pbr/iron_n.png", "alpha.policy", &data),
        "the exact table holds another value: nothing"
    );
    assert_eq!(p.text(), start, "nothing changed");
    assert!(p.clear_exact_file_value("PBR/IRON_N.PNG", "alpha.policy", &data));
    assert!(
        !p.has_file_key("PBR/IRON_N.PNG"),
        "the emptied exact table goes"
    );
    assert_eq!(p.file_value("pbr/iron_n", "alpha.policy"), Some(data));
    assert_eq!(
        p.file_value("pbr/iron_n.png", "alpha.policy"),
        Some(json!("remove"))
    );
}

/// N8-p (round 4, the reviewer's sequence): the listed table was re-spelled by hand, so no table is spelled as the
/// entry. The next user step that runs the follow-on drops the entry, in that step's own Undo step. Later the user writes
/// their own `data` under the original spelling, between sessions: the next step leaves it, as nothing lists it.
#[test]
fn an_entry_whose_table_is_gone_leaves_the_list_in_the_next_step() {
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    let t = d.path().join("textures");
    std::fs::write(
        t.join("sdc.toml"),
        "[files.'PBR/IRON_N.PNG']\nalpha.policy = \"remove\"\n",
    )
    .unwrap();
    std::fs::write(
        t.join("sdc-project.toml"),
        "[project]\nauto_data = [\"pbr/iron_n.png\"]\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(t.clone(), d.path().join("export"));
    settle(&mut m);
    assert!(
        !m.project.as_ref().unwrap().has_file_key(IRON_N),
        "precondition: no table spelled as the entry"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()], "precondition");
    m.set_alpha("armor/steel.png", AlphaPolicy::Remove);
    assert!(auto_data(&m).is_empty(), "the stale entry is dropped");
    assert_eq!(m.undo_label(), Some("Alpha"), "in the user's own step");
    m.undo();
    assert_eq!(
        auto_data(&m),
        vec![IRON_N.to_string()],
        "one Undo brings both back"
    );
    assert_eq!(held(&m, "armor/steel.png", "alpha.policy"), None);
    m.redo();
    settle(&mut m);
    m.save().expect("saved");
    drop(m);
    let mut text = std::fs::read_to_string(t.join("sdc.toml")).unwrap();
    text.push_str("\n[files.'pbr/iron_n.png']\nalpha.policy = \"data\"\n");
    std::fs::write(t.join("sdc.toml"), text).unwrap();
    let mut m = opened_as_the_app(t, d.path().join("export"));
    settle(&mut m);
    assert!(
        m.project.as_ref().unwrap().has_file_key(IRON_N),
        "precondition: the user's own table under the original spelling"
    );
    m.set_alpha("armor/steel.png", AlphaPolicy::Data);
    assert_eq!(
        m.project
            .as_ref()
            .unwrap()
            .file_value(IRON_N, "alpha.policy"),
        Some(json!("data")),
        "the user's data stays"
    );
}

/// N8-q (round 4): two distinct case spellings of one file, both listed and both holding `data` in their own exact
/// table, as saved. Ending their last share clears both: neither is the other's protected twin in the read-back.
#[test]
fn ending_a_share_clears_two_listed_spellings_of_one_file() {
    use sdc_gui::model::sets::SlotCommand;
    const UPPER: &str = "PBR/IRON_N.PNG";
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    let t = d.path().join("textures");
    std::fs::write(
        t.join("sdc.toml"),
        "[files.'pbr/iron_n.png']\nalpha.policy = \"data\"\n\n[files.'PBR/IRON_N.PNG']\nalpha.policy = \"data\"\n",
    )
    .unwrap();
    std::fs::write(
        t.join("sdc-project.toml"),
        "[project]\nauto_data = [\"pbr/iron_n.png\", \"PBR/IRON_N.PNG\"]\n\n[[sets]]\nid = \"armor/steel\"\nborrow = { slot2 = \"pbr/iron\" }\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(t, d.path().join("export"));
    settle(&mut m);
    let exact_alpha = |m: &AppModel, k: &str| {
        let p = m.project.as_ref().unwrap();
        p.has_file_key(k)
            .then(|| p.file_value(k, "alpha.policy"))
            .flatten()
    };
    assert!(
        !m.project.as_ref().unwrap().read_only() && !m.sets_doc.as_ref().unwrap().read_only(),
        "precondition: both documents open for editing"
    );
    for k in [IRON_N, UPPER] {
        assert_eq!(exact_alpha(&m, k), Some(json!("data")), "precondition: {k}");
    }
    assert_eq!(auto_data(&m), vec![IRON_N.to_string(), UPPER.to_string()]);
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    for k in [IRON_N, UPPER] {
        assert_eq!(exact_alpha(&m, k), None, "{k} is cleared");
    }
    assert!(auto_data(&m).is_empty(), "both entries leave the list");
    assert_eq!(m.undo_label(), Some("Clear slot 2 of armor/steel"));
}

/// N8-p: an entry whose exact table holds another value is dropped too, in a step that writes and clears nothing.
#[test]
fn an_entry_whose_table_holds_another_value_leaves_the_list() {
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    let t = d.path().join("textures");
    std::fs::write(
        t.join("sdc.toml"),
        "[files.'pbr/iron_n.png']\nalpha.policy = \"remove\"\n",
    )
    .unwrap();
    std::fs::write(
        t.join("sdc-project.toml"),
        "[project]\nauto_data = [\"pbr/iron_n.png\"]\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(t, d.path().join("export"));
    settle(&mut m);
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()], "precondition");
    m.set_alpha("armor/steel.png", AlphaPolicy::Remove);
    assert!(auto_data(&m).is_empty(), "the stale entry is dropped");
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("remove")));
}

/// N8-s (round 4, the reviewer's pin): a real Share of an eligible normal that has no `[files]` table, into the empty
/// Normal slot of a populated regular set, on a project with 4,000 `[files]` tables, walks the keys at most once before
/// the edit and once after it: the write creates the table under the spelling the step's index chose, with no lookup.
/// The normal is `pbr_normal` with no table: the saved sets document chooses PBR grass for `landscape/rock`, so the
/// engine types its landscape `_n` itself, and with accept None nothing records a type.
#[test]
fn a_share_that_creates_the_normals_table_walks_the_keys_once_per_document_state() {
    use sdc_gui::model::sets::DropChoice;
    let d = tempfile::tempdir().unwrap();
    rock(d.path());
    let t = d.path().join("textures");
    std::fs::create_dir_all(t.join("armor")).unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([90u8, 90, 100]))
        .save(t.join("armor/steel.png"))
        .unwrap();
    let mut toml = String::new();
    for i in 0..4_000 {
        let _ = write!(
            toml,
            "[files.\"old/f{i:04}.png\"]\nalpha.policy = \"remove\"\n\n"
        );
    }
    std::fs::write(t.join("sdc.toml"), toml).unwrap();
    std::fs::write(
        t.join("sdc-project.toml"),
        "[[sets]]\nid = \"landscape/rock\"\nshader = \"pbr_grass\"\n",
    )
    .unwrap();
    let mut m = AppModel::new(sdc_gui::settings::GuiSettings::default(), None, session());
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(t);
    settle(&mut m);
    let p = m.project.as_ref().unwrap();
    assert!(
        !p.has_file_key(ROCK_N) && p.file_value(ROCK_N, "subtype").is_none(),
        "precondition: no table for the normal"
    );
    let j = m.item(ROCK_N).and_then(|i| i.job.clone()).expect("job");
    assert_eq!(j.profile.subtype, Subtype::PbrNormal, "precondition");
    assert_ne!(j.profile.alpha.policy, AlphaPolicy::Data, "precondition");
    let start = sdc_gui::project::key_walks();
    drop_now(&mut m, ROCK_N, "armor/steel", DropChoice::Share);
    let walks = sdc_gui::project::key_walks() - start;
    let p = m.project.as_ref().unwrap();
    assert!(
        p.has_file_key(ROCK_N),
        "precondition: the share created the table"
    );
    assert_eq!(p.file_value(ROCK_N, "alpha.policy"), Some(json!("data")));
    assert_eq!(auto_data(&m), vec![ROCK_N.to_string()]);
    assert!(walks <= 2, "{walks} walks of the [files] keys in the share");
    settle(&mut m);
}

/// N8-s (B4 Minor): the walk counter counts `[files]` walks only: an alias lookup that misses the exact `[inputs]`
/// spelling compares every `[inputs]` key, which is not a `[files]` walk.
#[test]
fn an_inputs_lookup_is_not_a_files_walk() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("sdc.toml"),
        "[inputs.'PBR/IRON.PNG']\nas = \"pbr/plate.png\"\n\n[files.'a/b.png']\nalpha.policy = \"remove\"\n",
    )
    .unwrap();
    let p = sdc_gui::project::ProjectDoc::open(d.path());
    assert!(!p.read_only(), "{:?}", p.error());
    let start = sdc_gui::project::key_walks();
    assert_eq!(
        p.read_as("pbr/iron.png").as_deref(),
        Some("pbr/plate.png"),
        "precondition: found under another case"
    );
    assert_eq!(sdc_gui::project::key_walks() - start, 0, "no [files] walk");
}

/// N8-s: `ProjectDoc::set_exact_file_value` writes the table spelled byte for byte as the key, created under that
/// spelling when absent, without walking the keys; a case twin and an extension-less table are left as they are.
#[test]
fn the_exact_setter_writes_only_its_own_spelling_without_a_walk() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("sdc.toml"),
        "[files.'PBR/IRON_N.PNG']\nalpha.policy = \"remove\"\n\n[files.'pbr/iron_n']\nsubtype = \"pbr_normal\"\n",
    )
    .unwrap();
    let mut p = sdc_gui::project::ProjectDoc::open(d.path());
    assert!(!p.read_only(), "{:?}", p.error());
    let twins = |p: &sdc_gui::project::ProjectDoc| {
        (
            p.exact_file_entry_text("PBR/IRON_N.PNG"),
            p.exact_file_entry_text("pbr/iron_n"),
        )
    };
    let before = twins(&p);
    let start = sdc_gui::project::key_walks();
    p.set_exact_file_value(IRON_N, "alpha.policy", &json!("data"));
    assert_eq!(sdc_gui::project::key_walks() - start, 0, "no walk");
    assert!(p.has_file_key(IRON_N), "the table under its own spelling");
    assert_eq!(p.file_value(IRON_N, "alpha.policy"), Some(json!("data")));
    assert_eq!(twins(&p), before, "the twins are left as they are");
    p.set_exact_file_value(IRON_N, "alpha.policy", &json!("remove"));
    assert_eq!(sdc_gui::project::key_walks() - start, 0, "no walk");
    assert_eq!(p.file_value(IRON_N, "alpha.policy"), Some(json!("remove")));
    assert_eq!(twins(&p), before);
}

/// N8-t (round 5, the reviewer's sequence): as saved, the listed table was re-spelled by hand (`PBR/IRON_N.PNG` holds
/// the `data` now, no table is spelled as the entry), `armor/steel` borrows the normal, and an unrelated texture is in
/// no set. The user's first step takes that texture back to its automatic set: a rejoin, which writes and clears no
/// file value, but the stale entry still leaves the list in that step. Later the user writes their own `data` under the
/// original spelling between sessions and ends the share: nothing lists it, so it stays.
#[test]
fn a_rejoin_step_drops_a_stale_entry_too() {
    use sdc_gui::model::sets::SlotCommand;
    const UPPER: &str = "PBR/IRON_N.PNG";
    let d = tempfile::tempdir().unwrap();
    open_slot_files(d.path());
    let t = d.path().join("textures");
    std::fs::create_dir_all(t.join("misc")).unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([70u8, 90, 110]))
        .save(t.join("misc/pebble.png"))
        .unwrap();
    std::fs::write(
        t.join("sdc.toml"),
        "[files.'PBR/IRON_N.PNG']\nalpha.policy = \"data\"\n",
    )
    .unwrap();
    std::fs::write(
        t.join("sdc-project.toml"),
        "[project]\nauto_data = [\"pbr/iron_n.png\"]\nno_set = [\"misc/pebble.png\"]\n\n[[sets]]\nid = \"armor/steel\"\nborrow = { slot2 = \"pbr/iron\" }\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(t.clone(), d.path().join("export"));
    settle(&mut m);
    assert!(
        !m.project.as_ref().unwrap().has_file_key(IRON_N),
        "precondition: no table spelled as the entry"
    );
    assert!(m.is_no_set("misc/pebble.png"), "precondition: in no set");
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()], "precondition");
    m.back_to_automatic_set(&["misc/pebble.png".to_string()])
        .unwrap();
    assert!(!m.is_no_set("misc/pebble.png"), "precondition: rejoined");
    assert!(
        auto_data(&m).is_empty(),
        "the stale entry leaves the list in the rejoin step"
    );
    assert_eq!(
        held(&m, UPPER, "alpha.policy"),
        Some(json!("data")),
        "the rejoin step changes no file value"
    );
    settle(&mut m);
    m.save().expect("saved");
    drop(m);
    let mut text = std::fs::read_to_string(t.join("sdc.toml")).unwrap();
    text.push_str("\n[files.'pbr/iron_n.png']\nalpha.policy = \"data\"\n");
    std::fs::write(t.join("sdc.toml"), text).unwrap();
    let mut m = opened_as_the_app(t, d.path().join("export"));
    settle(&mut m);
    assert!(
        m.project.as_ref().unwrap().has_file_key(IRON_N),
        "precondition: the user's own table under the original spelling"
    );
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "ending the share leaves the user's data"
    );
}

/// N8-v (round 5, a pin: it passes on the base): the read-back protects the tables of the intended targets' own file
/// only. Another file's table that the user changed by hand between runs is not read back, so ending the share still
/// clears the listed `data` and leaves the other table as the user wrote it.
#[test]
fn another_files_table_changed_by_hand_does_not_roll_the_step_back() {
    use sdc_gui::model::sets::SlotCommand;
    const OTHER: &str = "armor/other.png";
    let d = tempfile::tempdir().unwrap();
    open_slot_files(d.path());
    let t = d.path().join("textures");
    std::fs::write(
        t.join("sdc.toml"),
        "[files.'pbr/iron_n.png']\nalpha.policy = \"data\"\n\n[files.'armor/other.png']\nalpha.policy = \"remove\"\n",
    )
    .unwrap();
    std::fs::write(t.join("sdc-project.toml"), LISTED_AND_BORROWED).unwrap();
    let mut m = opened_as_the_app(t.clone(), d.path().join("export"));
    settle(&mut m);
    m.save().expect("saved");
    drop(m);
    let text = std::fs::read_to_string(t.join("sdc.toml")).unwrap();
    let edited = text.replace(
        "[files.'armor/other.png']\nalpha.policy = \"remove\"",
        "[files.'armor/other.png']\nalpha.policy = \"data\"",
    );
    assert_ne!(edited, text, "precondition: the hand edit");
    std::fs::write(t.join("sdc.toml"), edited).unwrap();
    let mut m = opened_as_the_app(t, d.path().join("export"));
    settle(&mut m);
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()], "precondition");
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "the step's clear takes"
    );
    assert!(auto_data(&m).is_empty());
    assert_eq!(
        held(&m, OTHER, "alpha.policy"),
        Some(json!("data")),
        "the other file's table stays as the user wrote it"
    );
}

/// N8-x (round 5, a pin): while the share is still live, a listed entry whose own table is gone, with the user's `data`
/// in a case twin, leaves the list in the next step: the stale-entry reader is exact, so the twin's `data` does not
/// keep the string listed. (A reader that took any matching table would keep it, and the exact remover would not show
/// it, since nothing is cleared while the share is live.)
#[test]
fn a_stale_entry_leaves_the_list_while_its_share_is_live() {
    const UPPER: &str = "PBR/IRON_N.PNG";
    let d = tempfile::tempdir().unwrap();
    open_slot_files(d.path());
    let t = d.path().join("textures");
    std::fs::write(
        t.join("sdc.toml"),
        "[files.'PBR/IRON_N.PNG']\nalpha.policy = \"data\"\n",
    )
    .unwrap();
    std::fs::write(t.join("sdc-project.toml"), LISTED_AND_BORROWED).unwrap();
    let mut m = opened_as_the_app(t, d.path().join("export"));
    settle(&mut m);
    assert!(
        !m.project.as_ref().unwrap().has_file_key(IRON_N),
        "precondition: no table spelled as the entry"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()], "precondition");
    m.set_alpha("armor/steel.png", AlphaPolicy::Remove);
    assert!(
        auto_data(&m).is_empty(),
        "the stale entry leaves the list though the share is live"
    );
    assert_eq!(held(&m, UPPER, "alpha.policy"), Some(json!("data")));
}

/// N-10 (batch): the sharer picked Advanced Skin (a batch step: the pick and its diffuse's `skin_albedo`) is no
/// longer a regular set, so `data` goes in that step.
#[test]
fn an_advanced_skin_pick_on_the_sharer_takes_data_back() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    share(&mut m, IRON_N, "armor/steel");
    m.set_shader("armor/steel", Some(Shader::AdvancedSkin));
    settle(&mut m);
    assert_eq!(
        held(&m, "armor/steel.png", "subtype"),
        Some(json!("skin_albedo")),
        "the batch path ran"
    );
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
    assert_eq!(m.undo_label(), Some("Shader"));
}

/// N-33 as the owner narrowed it (N-36 withdrawn), a pin: an automatic step never runs the follow-on, and a leftover
/// listed `data` stays until the user's next step. A saved `auto_data` key that no regular set shares survives the
/// open, whose recorder step (a batch) records the new files' types; the user's next step clears it.
#[test]
fn only_a_users_step_clears_a_leftover() {
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"pbr/iron_n.png\"]\nsubtype = \"normal\"\nalpha.policy = \"data\"\n",
    )
    .unwrap();
    std::fs::write(
        d.path().join("textures/sdc-project.toml"),
        "[project]\nauto_data = [\"pbr/iron_n.png\"]\nauto_normal = { \"pbr/iron_n.png\" = \"normal\" }\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert!(
        m.undo_label().is_some_and(|l| l.ends_with("automatically")),
        "the recorder followed the listed normal while preserving its alpha: {:?}",
        m.undo_label()
    );
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "no automatic step clears it"
    );
    assert_eq!(
        m.sets_doc.as_ref().unwrap().auto_data(),
        vec![IRON_N.to_string()]
    );
    // Review 4 Minor 1: a user batch that changes neither document (a bulk Reset of a field no texture holds) runs no
    // follow-on.
    m.apply_bulk_edit(
        &["armor/steel.png".to_string()],
        "mips.filter",
        sdc_gui::model::bulk::BulkOp::Clear,
    );
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "a batch that changed nothing clears nothing"
    );
    m.set_shader("armor/steel", Some(Shader::Default));
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "the user's next step re-checks every listed key"
    );
}

/// N8-t: an input-name batch drops stale ownership even though changing aliases suppresses file edits.
#[test]
fn an_alias_batch_drops_a_stale_entry_before_the_user_reuses_its_spelling() {
    use sdc_gui::model::sets::SlotCommand;
    const UPPER: &str = "PBR/IRON_N.PNG";
    let d = tempfile::tempdir().unwrap();
    open_slot_files(d.path());
    let t = d.path().join("textures");
    std::fs::copy(t.join("armor/steel.png"), t.join("armor/pebble.png")).unwrap();
    let open = || opened_as_the_app(t.clone(), d.path().join("export"));
    let mut m = open();
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
    m.save().unwrap();
    drop(m);

    let file = t.join("sdc.toml");
    let text = std::fs::read_to_string(&file).unwrap();
    let respelled = text.replace(IRON_N, UPPER);
    assert_ne!(respelled, text);
    std::fs::write(&file, respelled).unwrap();
    let mut m = open();
    assert!(!m.project.as_ref().unwrap().has_file_key(IRON_N));
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
    m.set_input_name("armor/pebble", "textures/armor/stone")
        .unwrap();
    assert!(m.read_as_of("armor/pebble.png").is_some());
    assert!(
        auto_data(&m).is_empty(),
        "the stale entry leaves in the alias batch"
    );
    assert_eq!(held(&m, UPPER, "alpha.policy"), Some(json!("data")));
    settle(&mut m);
    m.save().unwrap();
    drop(m);

    let mut text = std::fs::read_to_string(&file).unwrap();
    text.push_str("\n[files.'pbr/iron_n.png']\nalpha.policy = \"data\"\n");
    std::fs::write(file, text).unwrap();
    let mut m = open();
    assert!(auto_data(&m).is_empty(), "the alias batch saved the list");
    let borrows_iron = |m: &AppModel| {
        m.sets_doc.as_ref().unwrap().entries().iter().any(|e| {
            e.id == "armor/steel" && e.borrow.get(&2).map(String::as_str) == Some("pbr/iron")
        })
    };
    assert!(borrows_iron(&m));
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    assert!(!borrows_iron(&m));
    assert!(m.project.as_ref().unwrap().has_file_key(IRON_N));
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "ending the share leaves the user's data"
    );
}

/// How many Keep alpha rows name `key`.
fn keep_rows(m: &AppModel, key: &str) -> usize {
    use sdc_gui::model::bottom::{problems, At, Fix};
    problems(m)
        .iter()
        .filter(|r| {
            matches!(&r.at, At::Texture(k) if k == key)
                && matches!(r.fix, Some(Fix::KeepAlpha { .. }))
        })
        .count()
}

/// A share as an older Tessalith left it (shared into the regular set, no `data`, nothing listed), saved, and opened
/// again as the app opens a folder: nothing is written on open.
fn old_share(dir: &Path) -> AppModel {
    let mut m = two_sets(dir);
    share(&mut m, IRON_N, "armor/steel");
    m.edit_docs("seed", None, true, |p, s| {
        p.clear_file_value(IRON_N, "alpha.policy");
        s.set_auto_data(&[]);
    });
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None, "precondition");
    assert!(m.save().is_ok());
    drop(m);
    let m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "nothing is written on open"
    );
    m
}

const KEEP_SENTENCE: &str =
    "iron_n.png is shared with a regular set, which reads its alpha as specular; its alpha is not kept";

const STEEL_BORROWS: &str = "[[sets]]\nid = \"armor/steel\"\nborrow = { slot2 = \"pbr/iron\" }\n";

fn open_saved_share(dir: &Path, files: &str, sets: &str) -> AppModel {
    let root = dir.join("textures");
    std::fs::write(root.join("sdc.toml"), files).unwrap();
    std::fs::write(root.join("sdc-project.toml"), sets).unwrap();
    opened_as_the_app(root, dir.join("export"))
}

#[test]
fn keep_alpha_on_a_reopened_listed_normal_survives_unshare() {
    use sdc_gui::model::sets::SlotCommand;
    use sdc_gui::project::ProjectDoc;
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("textures");
    let mut m = two_sets(d.path());
    share(&mut m, IRON_N, "armor/steel");
    m.save().unwrap();
    drop(m);

    let mut files = ProjectDoc::open(&root);
    assert!(files.clear_exact_file_value(IRON_N, "alpha.policy", &json!("data")));
    files.save(false).unwrap();
    let mut m = opened_as_the_app(root.clone(), d.path().join("export"));
    assert_eq!(auto_data(&m), [IRON_N]);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
    m.keep_alpha(IRON_N).unwrap();
    let listed_after_keep = auto_data(&m);
    settle(&mut m);
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    m.save().unwrap();

    let saved = ProjectDoc::open(&root);
    assert_eq!(
        saved.file_value(IRON_N, "alpha.policy"),
        Some(json!("data"))
    );
    let sets = sdc_gui::sets_doc::SetsDoc::open(&root);
    assert!(listed_after_keep.is_empty(), "Keep alpha takes ownership");
    assert!(sets.auto_data().is_empty());
    assert!(sets.entries().iter().all(|e| !e.borrow.contains_key(&2)));
}

#[test]
fn keep_alpha_writes_one_literal_table_and_undo_restores_both_documents() {
    for (spellings, target) in [
        (vec![], IRON_N),
        (vec!["pbr/iron_n"], "pbr/iron_n"),
        (vec!["PBR/IRON_N"], "PBR/IRON_N"),
        (vec!["PBR/IRON_N.PNG", "pbr/iron_n"], "PBR/IRON_N.PNG"),
        (vec!["PBR/IRON_N.PNG", IRON_N, "Pbr/Iron_N.png"], IRON_N),
    ] {
        let d = tempfile::tempdir().unwrap();
        open_slot_files(d.path());
        // A rule supplies the type in the table-free case, so the recorder leaves the file unconfigured.
        let mut files =
            String::from("[[rules]]\nmatch.glob = '**/iron_n.png'\nsubtype = 'pbr_normal'\n");
        for k in &spellings {
            let _ = write!(files, "[files.\"{k}\"]\nsubtype = \"pbr_normal\"\n");
        }
        let mut m = open_saved_share(d.path(), &files, LISTED_AND_BORROWED);
        assert_eq!(
            m.project.as_ref().unwrap().has_file_key(IRON_N),
            spellings.contains(&IRON_N)
        );
        let before = m.project.as_ref().unwrap().text();
        let sets = m.sets_doc.as_ref().unwrap().text();
        assert_eq!(keep_rows(&m, IRON_N), 1);
        assert_eq!(auto_data(&m), [IRON_N]);
        m.keep_alpha(IRON_N).unwrap();
        settle(&mut m);
        m.save().unwrap();
        let saved: toml_edit::DocumentMut =
            std::fs::read_to_string(d.path().join("textures/sdc.toml"))
                .unwrap()
                .parse()
                .unwrap();
        let policies: Vec<_> = saved["files"]
            .as_table_like()
            .unwrap()
            .iter()
            .filter_map(|(k, t)| Some((k, t.get("alpha")?.get("policy")?.as_str()?)))
            .collect();
        assert_eq!(policies, [(target, "data")]);
        assert!(auto_data(&m).is_empty());
        assert_eq!(m.undo_label(), Some("Keep alpha"));
        m.undo();
        settle(&mut m);
        assert_eq!(m.project.as_ref().unwrap().text(), before);
        assert_eq!(m.sets_doc.as_ref().unwrap().text(), sets);
    }
}

#[test]
fn a_share_leaves_an_aliased_double_extension_inputs_policy_alone() {
    const OTHER: &str = "pbr/iron_n.png.tga";
    for (has_full_table, config) in [
        (true, "[files.'pbr/iron_n.png']\nsubtype = 'pbr_normal'\n"),
        (
            false,
            "[[rules]]\nmatch.glob = '**/iron_n.png'\nsubtype = 'pbr_normal'\n",
        ),
    ] {
        let d = tempfile::tempdir().unwrap();
        open_slot_files(d.path());
        let root = d.path().join("textures");
        bumpy_n(&root.join(OTHER));
        let files = format!(
            "[inputs.'{OTHER}']\nas = 'pbr/copper.tga'\n{config}\
             [files.'{OTHER}']\nsubtype = 'pbr_albedo'\n"
        );
        let mut m = open_saved_share(d.path(), &files, "");
        assert_eq!(
            m.project.as_ref().unwrap().has_file_key(IRON_N),
            has_full_table
        );
        let other_policy = |m: &AppModel| {
            let item = m.item(OTHER).unwrap();
            let job = item
                .job
                .as_ref()
                .unwrap_or_else(|| panic!("{:?}", item.status));
            assert_eq!(job.output.file_name().unwrap(), "copper.dds");
            job.profile.alpha.policy
        };
        let before = other_policy(&m);
        assert_ne!(before, AlphaPolicy::Data);
        share(&mut m, IRON_N, "armor/steel");
        m.save().unwrap();
        let saved = sdc_gui::project::ProjectDoc::open(&root);
        for key in saved.file_keys() {
            if engine::overrides::path_key_matches(&key, OTHER) {
                assert_eq!(saved.file_value(&key, "alpha.policy"), None, "{key}");
            }
        }
        assert_eq!(saved.has_file_key(IRON_N), has_full_table);
        assert!(sdc_gui::sets_doc::SetsDoc::open(&root)
            .auto_data()
            .is_empty());
        assert_eq!(other_policy(&m), before);
        assert_eq!(keep_rows(&m, IRON_N), 1);
    }
}

#[test]
fn ending_a_share_keeps_data_if_a_second_input_now_matches_the_listed_table() {
    use sdc_gui::model::sets::SlotCommand;
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    let root = d.path().join("textures");
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(auto_data(&m), [IRON_N]);
    m.save().unwrap();
    drop(m);
    bumpy_n(&root.join("pbr/iron_n.png.tga"));
    let path = root.join("sdc.toml");
    let mut files = std::fs::read_to_string(&path).unwrap();
    files.push_str(
        "\n[inputs.'pbr/iron_n.png.tga']\nas = 'pbr/copper.tga'\n\
         [files.'pbr/iron_n.png.tga']\nsubtype = 'pbr_albedo'\n",
    );
    std::fs::write(path, files).unwrap();
    let mut m = opened_as_the_app(root.clone(), d.path().join("export"));
    let item = m.item("pbr/iron_n.png.tga").unwrap();
    let other = item
        .job
        .as_ref()
        .unwrap_or_else(|| panic!("{:?}", item.status));
    assert_eq!(other.output.file_name().unwrap(), "copper.dds");
    assert_eq!(other.profile.alpha.policy, AlphaPolicy::Data);
    assert_eq!(auto_data(&m), [IRON_N]);
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    assert_eq!(m.undo_label(), Some("Clear slot 2 of armor/steel"));
    settle(&mut m);
    m.save().unwrap();
    let saved = sdc_gui::project::ProjectDoc::open(&root);
    assert_eq!(
        saved.file_value(IRON_N, "alpha.policy"),
        Some(json!("data"))
    );
    let sets = sdc_gui::sets_doc::SetsDoc::open(&root);
    assert!(sets.entries().iter().all(|e| !e.borrow.contains_key(&2)));
    assert_eq!(
        sets.auto_data(),
        [IRON_N],
        "Unshare keeps the exact listed key on disk"
    );
}

#[test]
fn picking_data_for_the_tga_keeps_the_png_listed_until_unshare() {
    use sdc_gui::model::sets::SlotCommand;
    const OTHER: &str = "pbr/iron_n.png.tga";
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("textures");
    let open = || opened_as_the_app(root.clone(), d.path().join("export"));
    let mut m = two_sets(d.path());
    share(&mut m, IRON_N, "armor/steel");
    m.save().unwrap();
    drop(m);

    bumpy_n(&root.join(OTHER));
    let path = root.join("sdc.toml");
    let mut files = std::fs::read_to_string(&path).unwrap();
    files.push_str(
        "\n[inputs.'pbr/iron_n.png.tga']\nas = 'pbr/copper.tga'\n\
         [files.'pbr/iron_n.png.tga']\nsubtype = 'pbr_albedo'\nalpha.policy = 'remove'\n",
    );
    std::fs::write(path, files).unwrap();
    let mut m = open();
    assert_eq!(auto_data(&m), [IRON_N]);
    assert_eq!(held(&m, OTHER, "alpha.policy"), Some(json!("remove")));
    m.select_item(OTHER);
    let mut h = keep_harness(m);
    pick_data_over_remove(&mut h);
    h.state_mut().save().unwrap();
    drop(h);

    let m = open();
    assert_eq!(held(&m, OTHER, "alpha.policy"), Some(json!("data")));
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
    assert_eq!(auto_data(&m), [IRON_N], "the picker claimed only the TGA");
    drop(m);
    std::fs::remove_file(root.join(OTHER)).unwrap();
    let mut m = open();
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    m.save().unwrap();
    let m = open();
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
    assert!(auto_data(&m).is_empty());
    assert_eq!(held(&m, OTHER, "alpha.policy"), Some(json!("data")));
}

#[test]
fn changing_an_exact_table_to_data_keeps_its_listed_case_twin() {
    const TWIN: &str = "PBR/IRON_N.PNG";
    let d = tempfile::tempdir().unwrap();
    open_slot_files(d.path());
    let mut m = open_saved_share(
        d.path(),
        "[files.'PBR/IRON_N.PNG']\nalpha.policy = 'data'\n\
         [files.'pbr/iron_n.png']\nalpha.policy = 'remove'\n",
        &format!("[project]\nauto_data = ['{TWIN}']\n{STEEL_BORROWS}"),
    );
    assert_eq!(auto_data(&m), [TWIN]);
    m.set_field(IRON_N, "alpha.policy", json!("data"));
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
    assert_eq!(auto_data(&m), [TWIN]);
}

#[test]
fn keep_alpha_leaves_an_aliased_double_extension_input_alone() {
    let d = tempfile::tempdir().unwrap();
    open_slot_files(d.path());
    let root = d.path().join("textures");
    let other = "pbr/iron_n.png.tga";
    bumpy_n(&root.join(other));
    let mut m = open_saved_share(
        d.path(),
        "[inputs.'pbr/iron_n.png.tga']\nas = 'pbr/copper.tga'\n\
         [files.'pbr/iron_n']\nsubtype = 'pbr_normal'\n",
        STEEL_BORROWS,
    );
    let other_policy = |m: &AppModel| {
        let job = m.item(other).unwrap().job.as_ref().unwrap();
        assert_eq!(job.output.file_name().unwrap(), "copper.dds");
        job.profile.alpha.policy
    };
    let before = other_policy(&m);
    assert_ne!(before, AlphaPolicy::Data);
    m.keep_alpha(IRON_N).unwrap();
    settle(&mut m);
    m.save().unwrap();
    assert_eq!(
        other_policy(&m),
        before,
        "Keep alpha changed the TGA's policy"
    );
    let saved = sdc_gui::project::ProjectDoc::open(&root);
    assert_eq!(
        saved.file_value(IRON_N, "alpha.policy"),
        Some(json!("data"))
    );
    assert!(!saved.has_file_key(IRON_N));
}

#[test]
fn keep_alpha_refuses_a_table_that_also_names_another_input() {
    use egui_kittest::kittest::{NodeT, Queryable};
    for (other, table) in [
        ("pbr/iron_n.png.tga", None),
        ("pbr/iron_n.png.tga", Some(IRON_N)),
        ("pbr/iron_n.tga", Some("pbr/iron_n")),
    ] {
        let d = tempfile::tempdir().unwrap();
        open_slot_files(d.path());
        bumpy_n(&d.path().join("textures").join(other));
        let mut files = format!(
            "[inputs.'{other}']\nas = 'pbr/copper.tga'\n[files.'{other}']\nsubtype = 'pbr_albedo'\n\
             [[rules]]\nmatch.glob = '**/iron_n.png'\nsubtype = 'pbr_normal'\n"
        );
        if let Some(table) = table {
            let _ = write!(files, "[files.'{table}']\nsubtype = 'pbr_normal'\n");
        }
        let mut m = open_saved_share(d.path(), &files, STEEL_BORROWS);
        assert_eq!(keep_rows(&m, IRON_N), 1);
        assert_eq!(
            m.project.as_ref().unwrap().has_file_key(IRON_N),
            table == Some(IRON_N)
        );
        m.open_problems(Some(sdc_gui::model::bottom::ProblemsFilter::Notes));
        let mut h = keep_harness(m);
        let button = h.get_by_role_and_label(
            eframe::egui::accesskit::Role::Button,
            "Keep alpha for pbr/iron_n.png",
        );
        assert!(button.accesskit_node().is_disabled(), "{other}, {table:?}");
        let before = h.state().project.as_ref().unwrap().text();
        let reason = h.state_mut().keep_alpha(IRON_N).unwrap_err();
        assert!(reason.contains("sdc.toml"), "{reason}");
        assert_eq!(h.state().project.as_ref().unwrap().text(), before);
    }
}

#[test]
fn keep_alpha_then_reset_preserves_the_saved_warning_acknowledgement() {
    use egui_kittest::kittest::Queryable;
    let d = tempfile::tempdir().unwrap();
    open_slot_files(d.path());
    let mut m = open_saved_share(
        d.path(),
        "[files.'pbr/iron_n']\nsubtype = 'pbr_normal'\nallow = ['CHAN-NOT-GRAY']\n",
        STEEL_BORROWS,
    );
    assert_eq!(held(&m, IRON_N, "allow"), Some(json!(["CHAN-NOT-GRAY"])));
    m.keep_alpha(IRON_N).unwrap();
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
    m.click_item(IRON_N, false, false);
    m.ask_reset_all();
    let mut h = keep_harness(m);
    h.get_by_label("Reset all settings of 1 texture?");
    h.get_by_role_and_label(eframe::egui::accesskit::Role::Button, "Reset")
        .click();
    h.step();
    settle(h.state_mut());
    assert!(h.state().dialog.is_none());
    h.state_mut().save().unwrap();
    let saved = sdc_gui::project::ProjectDoc::open(&d.path().join("textures"));
    assert_eq!(saved.file_value(IRON_N, "alpha.policy"), None);
    assert_eq!(
        saved.file_value(IRON_N, "allow"),
        Some(json!(["CHAN-NOT-GRAY"]))
    );
}

#[test]
fn keep_alpha_refuses_two_folded_full_tables() {
    use egui_kittest::kittest::{NodeT, Queryable};
    let d = tempfile::tempdir().unwrap();
    open_slot_files(d.path());
    let mut m = open_saved_share(d.path(), "[files.\"PBR/IRON_N.PNG\"]\nsubtype = \"pbr_normal\"\n[files.\"Pbr/Iron_N.png\"]\nsubtype = \"pbr_normal\"\n", STEEL_BORROWS);
    assert_eq!(keep_rows(&m, IRON_N), 1);
    m.open_problems(None);
    let mut h = keep_harness(m);
    let button = h
        .query_all_by(|n| n.label().is_some_and(|s| s.starts_with("Keep alpha for ")))
        .next()
        .unwrap();
    assert!(button.accesskit_node().is_disabled());
    let before = h.state().project.as_ref().unwrap().text();
    let reason = h.state_mut().keep_alpha(IRON_N).unwrap_err();
    assert!(reason.contains("sdc.toml"), "{reason}");
    assert_eq!(h.state().project.as_ref().unwrap().text(), before);
}

#[test]
fn keep_alpha_counts_a_resolved_record_fill() {
    use sdc_gui::model::sets::borrowed_normal;
    let d = tempfile::tempdir().unwrap();
    open_slot_files(d.path());
    let mut m = open_saved_share(d.path(), "", "[[sets]]\nid = \"armor/steel\"\n[sets.txst]\nplugin = \"a.esp\"\nformid = \"1\"\nedid = \"E\"\nslot2 = \"textures/pbr/iron_n.dds\"\n");
    assert_eq!(
        borrowed_normal(&m).get("armor/steel").map(String::as_str),
        Some("pbr/iron")
    );
    assert_eq!(keep_rows(&m, IRON_N), 1);
    m.set_slot_lock("armor/steel", 1, true).unwrap();
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
    assert!(auto_data(&m).is_empty());
}

#[test]
fn keep_alpha_follows_the_visible_borrow_for_duplicate_set_ids() {
    use sdc_gui::model::sets::borrowed_normal;
    let d = tempfile::tempdir().unwrap();
    open_slot_files(d.path());
    rock(d.path());
    let m = open_saved_share(d.path(), "", &format!("{STEEL_BORROWS}[[sets]]\nid = \"ARMOR/STEEL\"\nborrow = {{ slot2 = \"landscape/rock\" }}\n"));
    assert_eq!(
        borrowed_normal(&m).get("armor/steel").map(String::as_str),
        Some("pbr/iron")
    );
    assert_eq!((keep_rows(&m, IRON_N), keep_rows(&m, ROCK_N)), (1, 0));
}

#[test]
fn a_duplicate_borrower_keeps_the_first_shares_saved_data() {
    use sdc_gui::model::sets::{borrowed_normal, SlotCommand};
    let d = tempfile::tempdir().unwrap();
    rock(d.path());
    let mut m = two_sets(d.path());
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(auto_data(&m), [IRON_N]);
    m.save().unwrap();
    drop(m);

    let root = d.path().join("textures");
    let path = root.join("sdc-project.toml");
    let mut sets = std::fs::read_to_string(&path).unwrap();
    sets.push_str("\n[[sets]]\nid = 'ARMOR/STEEL'\nborrow = { slot2 = 'landscape/rock' }\n");
    std::fs::write(path, sets).unwrap();
    let open = || opened_as_the_app(root.clone(), d.path().join("export"));
    let mut m = open();
    assert_eq!(borrowed_normal(&m)["armor/steel"], "pbr/iron");
    m.set_alpha("armor/steel.png", AlphaPolicy::Remove);
    settle(&mut m);
    m.save().unwrap();
    drop(m);
    let mut m = open();
    assert_eq!(
        held(&m, "armor/steel.png", "alpha.policy"),
        Some(json!("remove"))
    );
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
    assert_eq!(auto_data(&m), [IRON_N]);
    assert_eq!(held(&m, ROCK_N, "alpha.policy"), None);
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    assert!(!borrowed_normal(&m).contains_key("armor/steel"));
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
    assert_eq!(held(&m, ROCK_N, "alpha.policy"), None);
    assert!(auto_data(&m).is_empty());
}

fn pinned_iron_lends_to_steel(dir: &Path, files: &str, steel: &str) -> AppModel {
    use sdc_gui::model::sets::borrowed_normal;
    open_slot_files(dir);
    rock(dir);
    let m = open_saved_share(dir, files, &format!(
        "{steel}[[sets]]\nid = 'pbr/iron'\nborrow = {{ slot2 = 'landscape/rock' }}\nlock = ['slot2']\n"
    ));
    let borrows = borrowed_normal(&m);
    assert_eq!(borrows["pbr/iron"], "landscape/rock");
    assert_eq!(borrows["armor/steel"], "pbr/iron");
    assert!(m.slot_locked("pbr/iron", 2));
    m
}

#[test]
fn the_lending_slot_lock_prevents_an_automatic_data_write() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pinned_iron_lends_to_steel(
        d.path(),
        "[files.'landscape/rock_n.png']\nsubtype = 'pbr_normal'\n",
        &format!("{STEEL_BORROWS}shader = 'pbr'\n"),
    );
    assert_eq!(
        m.item(ROCK_N)
            .unwrap()
            .job
            .as_ref()
            .unwrap()
            .profile
            .alpha
            .policy,
        AlphaPolicy::Remove
    );
    m.set_shader("armor/steel", Some(Shader::Default));
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
    assert!(auto_data(&m).is_empty());
    m.set_shader("pbr/iron", Some(Shader::Default));
    settle(&mut m);
    assert_eq!(held(&m, ROCK_N, "alpha.policy"), Some(json!("data")));
    assert_eq!(auto_data(&m), [ROCK_N], "rock's own slot is unlocked");
}

#[test]
fn the_lending_slot_lock_prevents_an_automatic_data_clear() {
    use sdc_gui::model::sets::SlotCommand;
    let d = tempfile::tempdir().unwrap();
    let mut m = pinned_iron_lends_to_steel(
        d.path(),
        "[files.'pbr/iron_n.png']\nsubtype = 'pbr_normal'\nalpha.policy = 'data'\n",
        LISTED_AND_BORROWED,
    );
    assert_eq!(auto_data(&m), [IRON_N]);
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
    assert_eq!(auto_data(&m), [IRON_N]);
}

#[test]
fn keep_alpha_checks_the_lending_owners_own_slot_lock() {
    use egui_kittest::kittest::{NodeT, Queryable};
    let d = tempfile::tempdir().unwrap();
    open_slot_files(d.path());
    rock(d.path());
    let mut m = open_saved_share(d.path(), "", &format!("{STEEL_BORROWS}[[sets]]\nid = \"pbr/iron\"\nborrow = {{ slot2 = \"landscape/rock\" }}\nlock = [\"slot2\"]\n"));
    assert_eq!(keep_rows(&m, IRON_N), 1);
    let why = m
        .keep_alpha(IRON_N)
        .expect_err("iron lends its own locked Normal slot");
    assert!(why.contains("slot 2 of pbr/iron"), "{why}");
    m.open_problems(None);
    let mut h = keep_harness(m);
    let disabled = |h: &egui_kittest::Harness<'_, AppModel>| {
        h.query_all_by(|n| n.label().is_some_and(|s| s.starts_with("Keep alpha for ")))
            .next()
            .unwrap()
            .accesskit_node()
            .is_disabled()
    };
    assert!(disabled(&h));
    h.state_mut().set_slot_lock("pbr/iron", 2, false).unwrap();
    settle(h.state_mut());
    h.run();
    assert!(!disabled(&h));
}

#[test]
fn keep_alpha_repaints_without_searching_for_a_slot_lock() {
    use egui_kittest::kittest::Queryable;
    use sdc_gui::model::bottom::KEEP_ALPHA_NAMES;
    use sdc_gui::model::sets::LOCK_SEARCHES;
    let d = tempfile::tempdir().unwrap();
    let mut m = old_share(d.path());
    m.open_problems(None);
    let mut h = keep_harness(m);
    let before = LOCK_SEARCHES.get();
    h.get_by_role_and_label(
        eframe::egui::accesskit::Role::Button,
        "Keep alpha for pbr/iron_n.png",
    );
    let names = KEEP_ALPHA_NAMES.get();
    assert!(names > 0);
    h.run_steps(3);
    assert_eq!(LOCK_SEARCHES.get(), before);
    assert_eq!(
        KEEP_ALPHA_NAMES.get(),
        names,
        "repaints rebuilt the accessible name"
    );
}

#[test]
fn keep_alpha_waits_for_a_pending_replace() {
    use sdc_gui::model::sets::{DropChoice, DropOutcome};
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    let DropOutcome::AskReplace { kind, existing } =
        m.drop_choice(IRON_N, "armor/steel", DropChoice::Share)
    else {
        panic!("steel's normal requires Replace");
    };
    m.drop_replace(IRON_N, "armor/steel", DropChoice::Share, kind, &existing)
        .unwrap();
    assert_eq!(
        m.keep_alpha(IRON_N).unwrap_err(),
        "Wait for the plan to finish"
    );
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
    settle(&mut m);
    assert_eq!(keep_rows(&m, IRON_N), 1);
}

#[test]
fn keep_alpha_refreshes_on_undo_and_redo_before_the_plan_lands() {
    let d = tempfile::tempdir().unwrap();
    let mut m = old_share(d.path());
    m.set_alpha(IRON_N, AlphaPolicy::Remove);
    settle(&mut m);
    m.clear_field(IRON_N, "alpha.policy");
    settle(&mut m);
    assert_eq!(keep_rows(&m, IRON_N), 1);
    m.undo();
    assert_eq!(keep_rows(&m, IRON_N), 0);
    m.redo();
    assert_eq!(keep_rows(&m, IRON_N), 1);
}

#[test]
fn keep_alpha_names_each_project_relative_path() {
    use egui_kittest::kittest::Queryable;
    let d = tempfile::tempdir().unwrap();
    open_slot_files(d.path());
    let root = d.path().join("textures");
    std::fs::create_dir_all(root.join("weapons")).unwrap();
    for name in ["iron.png", "iron_n.png", "iron_rmaos.png"] {
        std::fs::copy(root.join("pbr").join(name), root.join("weapons").join(name)).unwrap();
    }
    std::fs::copy(root.join("armor/steel.png"), root.join("armor/helm.png")).unwrap();
    let mut m = open_saved_share(d.path(), "", &format!("{STEEL_BORROWS}[[sets]]\nid = \"armor/helm\"\nborrow = {{ slot2 = \"weapons/iron\" }}\n"));
    m.open_problems(None);
    let h = keep_harness(m);
    for key in [IRON_N, "weapons/iron_n.png"] {
        assert!(
            h.query_by_role_and_label(
                eframe::egui::accesskit::Role::Button,
                &format!("Keep alpha for {key}")
            )
            .is_some(),
            "{key}"
        );
    }
}

fn keep_harness(mut m: AppModel) -> egui_kittest::Harness<'static, AppModel> {
    open_texture_groups(&mut m); // the groups start closed (Task 8)
    let mut h = egui_kittest::Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(eframe::egui::vec2(1400.0, 900.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    h.run();
    h
}

fn pick_data_over_remove(h: &mut egui_kittest::Harness<'_, AppModel>) {
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::{NodeT, Queryable};
    let combos: Vec<_> = h
        .query_all_by_role(Role::ComboBox)
        .filter(|n| n.accesskit_node().value().as_deref() == Some("Remove"))
        .collect();
    assert_eq!(combos.len(), 1, "the Alpha combo shows Remove");
    assert!(
        !combos[0].accesskit_node().is_disabled(),
        "the Alpha row is on"
    );
    combos[0].click();
    h.run();
    let data: Vec<_> = h.query_all_by_label("Data").collect();
    assert_eq!(data.len(), 1, "one Data option, in the open Alpha list");
    data[0].click();
    h.step();
    settle(h.state_mut());
}

/// N-10: a project opened with a share already in that state shows one Note row on the shared normal; Keep alpha
/// (Enter works) writes `data` as one step named "Keep alpha", and the row goes.
#[test]
fn an_old_share_gets_a_keep_alpha_row() {
    use egui_kittest::kittest::Queryable;
    use sdc_gui::model::bottom::{problems, At, Fix};
    let d = tempfile::tempdir().unwrap();
    let mut m = old_share(d.path());
    let rows: Vec<_> = problems(&m)
        .iter()
        .filter(|r| {
            matches!(&r.at, At::Texture(k) if k == IRON_N)
                && r.fix
                    == Some(Fix::KeepAlpha {
                        table: Ok(IRON_N.into()),
                        name: format!("Keep alpha for {IRON_N}"),
                    })
        })
        .cloned()
        .collect();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].is_note());
    assert_eq!(rows[0].shown(), KEEP_SENTENCE);
    m.open_problems(None);
    let mut h = keep_harness(m);
    h.get_by_role_and_label(
        eframe::egui::accesskit::Role::Button,
        "Keep alpha for pbr/iron_n.png",
    )
    .focus();
    h.run();
    h.key_press(eframe::egui::Key::Enter);
    h.step();
    assert!(h.state_mut().wait_idle(Duration::from_secs(60)));
    h.run();
    assert_eq!(held(h.state(), IRON_N, "alpha.policy"), Some(json!("data")));
    assert_eq!(h.state().undo_label(), Some("Keep alpha"));
    assert_eq!(keep_rows(h.state(), IRON_N), 0);
}

/// N-10, review 1 Minor 8: with the owner's Normal slot locked, the share step writes nothing; the row shows Keep
/// alpha off with the lock's reason, and the model refuses it too.
#[test]
fn keep_alpha_is_off_on_a_locked_slot() {
    use egui_kittest::kittest::{NodeT, Queryable};
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    m.set_slot_lock("pbr/iron", 2, true).unwrap();
    settle(&mut m);
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "the share step skips a locked owner"
    );
    assert!(m.keep_alpha(IRON_N).is_err());
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
    m.open_problems(None);
    let h = keep_harness(m);
    let b = h.get_by_role_and_label(
        eframe::egui::accesskit::Role::Button,
        "Keep alpha for pbr/iron_n.png",
    );
    assert!(b.accesskit_node().is_disabled());
}

/// N-10: an alpha policy the user wrote is the user's choice: no row.
#[test]
fn the_users_own_alpha_gets_no_row() {
    let d = tempfile::tempdir().unwrap();
    let mut m = old_share(d.path());
    m.set_alpha(IRON_N, AlphaPolicy::Remove);
    settle(&mut m);
    assert_eq!(keep_rows(&m, IRON_N), 0);
}

/// Review 3 Important 2: a regular owner's recorded `normal` is shared into a regular set (no write: the owner is not
/// PBR); an `_rmaos` arrives, and the recorder's automatic step retypes it `pbr_normal`. No automatic step writes
/// `data`; the row shows it.
#[test]
fn a_shared_recorded_normal_that_follows_its_owner_into_pbr_gets_the_row() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    let mut m = two_sets(d.path());
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
    share(&mut m, WALL_N, "armor/steel");
    assert_eq!(
        held(&m, WALL_N, "alpha.policy"),
        None,
        "a regular owner's normal is not written"
    );
    add_rmaos(&t, &mut m);
    assert_eq!(
        held(&m, WALL_N, "subtype"),
        Some(json!("pbr_normal")),
        "the recorder's follow pass"
    );
    assert_eq!(
        held(&m, WALL_N, "alpha.policy"),
        None,
        "no automatic step writes it"
    );
    assert_eq!(keep_rows(&m, WALL_N), 1);
}

/// N-33 narrowed: the sharer's diffuse decided `pbr_albedo` turns it PBR only at the plan that decision causes; no
/// step runs after the plan, so the leftover `data` stays (spec limit 3) and no row shows (its planned alpha is
/// `data`). The user's next step clears it.
#[test]
fn a_diffuse_decision_that_turns_the_sharer_pbr_leaves_data() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    share(&mut m, IRON_N, "armor/steel");
    m.decide("armor/steel.png", Answer::Subtype(Subtype::PbrAlbedo));
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
    assert_eq!(
        m.sets_doc.as_ref().unwrap().auto_data(),
        vec![IRON_N.to_string()]
    );
    assert_eq!(keep_rows(&m, IRON_N), 0);
    m.set_shader("pbr/iron", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "the next user step re-checks every listed key"
    );
}

/// N-33 narrowed: a PBR sharer whose diffuse is decided back to `diffuse` is a regular set only after the plan; no
/// step writes `data` then, and the row shows it.
#[test]
fn a_sharer_that_leaves_pbr_by_its_diffuse_gets_the_row() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    m.decide("armor/steel.png", Answer::Subtype(Subtype::PbrAlbedo));
    settle(&mut m);
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None, "PBR into PBR");
    m.decide("armor/steel.png", Answer::Subtype(Subtype::Diffuse));
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
    assert_eq!(keep_rows(&m, IRON_N), 1);
}

#[test]
fn a_rejoined_borrower_offers_keep_alpha_for_its_shared_normal() {
    use sdc_gui::model::sets::SlotCommand;
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    share(&mut m, IRON_N, "armor/steel");
    m.remove_member("armor/steel", "armor/steel.png").unwrap();
    settle(&mut m);
    m.set_shader("pbr/iron", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "the empty borrower reads nothing"
    );
    assert_eq!(keep_rows(&m, IRON_N), 0);

    m.back_to_automatic_set(&["armor/steel.png".to_string()])
        .unwrap();
    settle(&mut m);
    assert!(sdc_gui::model::sets::views(&m)
        .iter()
        .any(|v| v.id == "armor/steel" && v.members.iter().any(|k| k == "armor/steel.png")));
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "the rejoin abstained"
    );
    assert_eq!(keep_rows(&m, IRON_N), 1);
    m.keep_alpha(IRON_N).unwrap();
    settle(&mut m);
    assert!(auto_data(&m).is_empty());
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
}

#[test]
fn a_users_remove_in_a_matching_stem_table_gets_no_row() {
    let d = tempfile::tempdir().unwrap();
    let mut m = old_share(d.path());
    m.edit_docs("stem policy", None, true, |p, _| {
        p.set_file_value("pbr/iron_n", "alpha.policy", &json!("remove"));
    });
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "the full-name table has only the type"
    );
    assert_eq!(
        held(&m, "pbr/iron_n", "alpha.policy"),
        Some(json!("remove"))
    );
    assert_eq!(keep_rows(&m, IRON_N), 0);
}
