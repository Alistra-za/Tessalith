//! Note GO: the shape card's Texture set dropdown (TXSTs plugins assign to the shape's 3D index, preview only).
#[path = "../../esp/tests/common/mod.rs"]
mod b;
#[path = "../../modscan/tests/common/mod.rs"]
mod scan_fixture;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use modscan::plugin_store::{MeshUse, ShapeSwap};
use modscan::store::{NifRow, ShapeView};
use render3d::ShapeId;
use sdc_gui::model::meshes::MeshesModel;
use sdc_gui::model::viewport3d::Content;
use sdc_gui::settings::MeshesSettings;
use std::sync::Arc;

const DIRT: u32 = 0x0000_0ABC;
const MOSS: u32 = 0x0000_0ABD;

fn harness(m: MeshesModel) -> Harness<'static, MeshesModel> {
    let h = Harness::builder()
        .with_size(eframe::egui::vec2(1400.0, 900.0))
        .build_ui_state(
            |ui, m: &mut MeshesModel| sdc_gui::ui::meshes::show(ui, m),
            m,
        );
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h
}

fn shape(block: u32, name: &str, index3d: u32) -> ShapeView {
    ShapeView {
        block,
        name: Some(name.into()),
        kind: "BSTriShape".into(),
        shader_kind: 1,
        shader_type: Some(0),
        slsf1: Some(0x8000_0101),
        slsf2: Some(0x21),
        textures: vec![Some("textures/a/body.dds".into())],
        raw: vec![None],
        alpha_flags: None,
        alpha_threshold: None,
        params: None,
        skinned: false,
        vertex_desc: None,
        num_vertices: Some(3),
        num_triangles: Some(1),
        index3d: Some(index3d),
    }
}

fn swap(name: &str, txst: u32, edid: &str, index: i32) -> ShapeSwap {
    ShapeSwap {
        shape_name: name.into(),
        txst_formid: txst,
        txst_edid: (txst != 0).then(|| edid.into()),
        index,
    }
}

fn arma(formid: u32, edid: &str, swaps: Vec<ShapeSwap>) -> MeshUse {
    MeshUse {
        formid,
        edid: Some(edid.into()),
        sig: "ARMA".into(),
        plugin: "Skins.esp".into(),
        model_slot: 2,
        n_overrides: 0,
        swaps,
    }
}

/// An LE mesh (nothing loads) with Body (#0, block 3) and Hands (#1, block 5); plugin records assign DirtTXST to #0
/// (twice: once naming another shape), MossTXST to #1, a TXST to #7 (no such shape) and an unresolved one to #0.
fn model() -> MeshesModel {
    let mut m = MeshesModel::empty();
    m.nifs = Arc::new(vec![NifRow {
        copy_id: 7,
        path: "meshes/x.nif".into(),
        wins: true,
        overridden_by: None,
        n_copies: 1,
        derived: false,
        le_mesh: true,
        problem: None,
        patched_in_game: false,
    }]);
    m.selected_nif = Some(7);
    m.shapes = vec![shape(3, "Body", 0), shape(5, "Hands", 1)];
    m.shapes_col = true;
    m.plugin_uses = vec![
        arma(
            0x100,
            "SkinA",
            vec![
                swap("Body", DIRT, "DirtTXST", 0),
                swap("Hands", MOSS, "MossTXST", 1),
            ],
        ),
        arma(
            0x101,
            "SkinB",
            vec![
                swap("Hands", DIRT, "DirtTXST", 0),
                swap("Body", 0, "", 0),
                swap("Tail", MOSS, "MossTXST", 7),
            ],
        ),
    ];
    m
}

#[test]
fn choices_follow_the_3d_index_not_the_shape_name() {
    let m = model();
    let c0 = m.txst_choices(&m.shapes[0]);
    assert_eq!(c0.len(), 1, "DirtTXST once, the unresolved one left out");
    assert_eq!(
        (c0[0].formid, c0[0].edid.as_deref()),
        (DIRT, Some("DirtTXST"))
    );
    assert_eq!(
        c0[0].users,
        [
            (0, Some("Body".to_string())),
            (1, Some("Hands".to_string()))
        ],
        "SkinB names Hands but assigns index 0"
    );
    let c1 = m.txst_choices(&m.shapes[1]);
    assert_eq!(c1.len(), 1);
    assert_eq!(c1[0].formid, MOSS);
    assert_eq!(c1[0].users, [(0, Some("Hands".to_string()))]);
    assert!(m.txst_choices(&shape(9, "Other", 2)).is_empty());
}

#[test]
fn each_card_has_its_dropdown_and_plugin_overrides_flag_a_missing_index() {
    let mut h = harness(model());
    h.run();
    h.get_by_label("Texture set of block 3: From the NIF");
    h.get_by_label("Texture set of block 5: From the NIF");
    h.get_by_label("Plugin overrides (2)").click();
    h.run();
    h.get_by_label("#0 Body → DirtTXST [00000ABC]");
    h.get_by_label("#7 Tail → MossTXST [00000ABD]");
    assert_eq!(
        h.query_all_by_label("No shape #7 in this NIF").count(),
        1,
        "only the swap whose index no shape has"
    );
}

/// Note GO4: UBE's body ARMAs (NakedTorso, MOD3 + NAM1, no MO3S): the skin texture is offered on every Skin Tint
/// shape whatever its index, on no other shape, and Plugin overrides names the field.
#[test]
fn arma_skin_textures_go_to_the_skin_tint_shapes() {
    use modscan::plugin_store::SKIN_TEXTURE;
    let mut m = model();
    let mut body = shape(3, "BaseShape", 0);
    body.shader_type = Some(5);
    let mut hands = shape(5, "Hands", 4);
    hands.shader_type = Some(5);
    m.shapes = vec![body, hands, shape(6, "Nails", 1)];
    let mut skin = arma(
        0x200,
        "00UBE_NakedTorso",
        vec![swap("", DIRT, "SkinBodyFemale", SKIN_TEXTURE)],
    );
    skin.model_slot = 3;
    m.plugin_uses = vec![skin];
    for i in [0, 1] {
        let c = m.txst_choices(&m.shapes[i]);
        assert_eq!(c.len(), 1, "Skin Tint shape {i}");
        assert_eq!((c[0].formid, c[0].users.clone()), (DIRT, vec![(0, None)]));
    }
    assert!(
        m.txst_choices(&m.shapes[2]).is_empty(),
        "not a Skin Tint shape"
    );
    let mut h = harness(m);
    h.run();
    h.get_by_label("Texture set of block 3: From the NIF");
    h.get_by_label("Texture set of block 5: From the NIF");
    assert!(h
        .query_by_label("Texture set of block 6: From the NIF")
        .is_none());
    h.get_by_label("Plugin overrides (1)").click();
    h.run();
    h.get_by_label("female skin texture (NAM1) → SkinBodyFemale [00000ABC]");
    assert!(h.query_by_label("No Skin Tint shape in this NIF").is_none());
}

#[test]
fn a_skin_texture_on_a_mesh_without_skin_shapes_is_flagged() {
    use modscan::plugin_store::SKIN_TEXTURE;
    let mut m = model();
    m.plugin_uses = vec![arma(
        0x200,
        "Torso",
        vec![swap("", DIRT, "SkinBodyMale", SKIN_TEXTURE)],
    )];
    let mut h = harness(m);
    h.run();
    h.get_by_label("Plugin overrides (1)").click();
    h.run();
    h.get_by_label("male skin texture (NAM0) → SkinBodyMale [00000ABC]");
    h.get_by_label("No Skin Tint shape in this NIF");
}

#[test]
fn a_card_with_no_assignment_has_no_dropdown() {
    let mut m = model();
    m.plugin_uses.clear();
    let mut h = harness(m);
    h.run();
    assert!(h.query_by_label_contains("Texture set of block").is_none());
}

#[test]
fn a_pick_outside_a_loaded_mesh_is_refused() {
    let mut m = model();
    assert!(m.set_txst(3, Some(DIRT)).is_err(), "no mesh in the 3D view");
    assert!(m.txst_pick(3).is_none());
}

/// Skyrim.esm: DirtTXST (TX00 dirt02, TX01 dirt02_n, TX02 → NIF slot 6 env mask) and RockStat (x.nif, alternate
/// texture DirtTXST on 3D index 0). A real scan, then the pick goes into the 3D view's spec with its textures'
/// sources, keeps the skin card's picks, and "From the NIF" takes it out; a reload of the shapes drops it.
#[test]
fn a_pick_goes_into_the_3d_view_and_from_the_nif_takes_it_out() {
    let inst = scan_fixture::instance_fixture("+A\n");
    let root = inst.path();
    let nif = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../nif/tests/fixtures/TestNifFile_Skinned_SE.nif"
    ))
    .unwrap();
    scan_fixture::put(root, "mods/A/meshes/x.nif", &nif);
    for t in ["dirt02.dds", "dirt02_n.dds", "dirt02_m.dds"] {
        scan_fixture::put(root, &format!("mods/A/textures/landscape/{t}"), b"dds");
    }
    let txst = b::rec(
        b"TXST",
        DIRT,
        &[
            b::edid("DirtTXST"),
            b::sub(b"TX00", &b::zstr("Landscape\\Dirt02.dds")),
            b::sub(b"TX01", &b::zstr("Landscape\\Dirt02_n.dds")),
            b::sub(b"TX02", &b::zstr("Landscape\\Dirt02_m.dds")),
        ],
    );
    let stat = b::rec(
        b"STAT",
        0xDEF,
        &[
            b::edid("RockStat"),
            b::sub(b"MODL", &b::zstr("x.nif")),
            b::sub(b"MODS", &b::alt(&[("Whatever", DIRT, 0)])),
        ],
    );
    scan_fixture::put(
        root,
        "Game/Data/Skyrim.esm",
        &b::plugin(
            esp::FLAG_ESM,
            &[],
            &[b::top(b"TXST", &[txst]), b::top(b"STAT", &[stat])],
        ),
    );
    scan_fixture::put(root, "profiles/Default/plugins.txt", b"");
    let mut m = MeshesModel::new(Some(root.join("scan.db")), MeshesSettings::default());
    m.procs = sdc_gui::model::meshes::no_procs;
    m.activate_with(None);
    m.use_instance(root.to_path_buf(), "Default".into());
    m.request_scan();
    assert!(m.take_request());
    m.start();
    let t = std::time::Instant::now();
    m.poll();
    while m.scanning() || m.rows_loading() {
        assert!(t.elapsed().as_secs() < 60, "scan did not end");
        std::thread::sleep(std::time::Duration::from_millis(10));
        m.poll();
    }
    assert_eq!(m.error, None);
    let a = m.mods.iter().find(|r| r.name == "A").expect("mod A").id;
    m.select_mod(a);
    while m.rows_loading() {
        assert!(t.elapsed().as_secs() < 60, "rows did not load");
        std::thread::sleep(std::time::Duration::from_millis(10));
        m.poll();
    }
    let copy = m
        .nifs
        .iter()
        .find(|r| r.path == "meshes/x.nif")
        .expect("x.nif scanned")
        .copy_id;
    m.select_nif(copy);
    let block = m
        .shapes
        .iter()
        .find(|s| s.index3d == Some(0))
        .expect("a shape with 3D index 0")
        .block;
    let s0 = m.shapes.iter().find(|s| s.block == block).unwrap().clone();
    assert_eq!(m.txst_choices(&s0).len(), 1);
    let spec = |m: &MeshesModel| match m.v3.content() {
        Some(Content::Mesh(s)) => s.clone(),
        other => panic!("not a scanned mesh: {other:?}"),
    };
    assert!(spec(&m).txst.is_empty());

    m.set_skin_wet(block, 0.5);
    m.set_txst(block, Some(DIRT)).unwrap();
    let s = spec(&m);
    let slots = &s.txst[&ShapeId(block)];
    assert_eq!(slots[0], "textures/landscape/dirt02.dds");
    assert_eq!(slots[1], "textures/landscape/dirt02_n.dds");
    assert_eq!(
        slots[5], "textures/landscape/dirt02_m.dds",
        "TX02 is the env mask, NIF slot 6"
    );
    assert!(slots[2].is_empty());
    assert!(s.textures.contains_key("textures/landscape/dirt02_n.dds"));
    assert_eq!(m.txst_pick(block).map(|(f, _)| f), Some(DIRT));
    assert!(
        m.skin_entry(block).is_some(),
        "the same model: skin picks stay"
    );

    m.set_txst(block, None).unwrap();
    assert!(spec(&m).txst.is_empty());
    assert!(m.txst_pick(block).is_none());

    m.set_txst(block, Some(DIRT)).unwrap();
    m.select_nif(copy);
    m.reload();
    assert!(m.txst_pick(block).is_none(), "the shapes reloaded: no pick");
    assert!(spec(&m).txst.is_empty());
}

/// The render thread loads a scanned mesh with the pick's slots in place of the shape's own (WARP, stand-in shaders).
#[test]
fn the_render_thread_loads_the_picked_slots() {
    use sdc_gui::model::viewport3d::{FileSource, MeshSpec, Viewport3d};
    use std::collections::{BTreeMap, HashMap};
    use std::time::{Duration, Instant};
    let standin = std::path::PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../render3d/tests/fixtures/standin"
    ));
    let cache = tempfile::tempdir().unwrap();
    let mut v = Viewport3d::new();
    v.use_shader_folder(standin);
    v.cache_dir = Some(cache.path().to_path_buf());
    let nif = std::path::PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../nif/tests/fixtures/TestNifFile_Skinned_SE.nif"
    ));
    let spec = |key: i64, txst: BTreeMap<ShapeId, Vec<String>>| MeshSpec {
        key,
        nif: FileSource::Loose(nif.clone()),
        textures: HashMap::new(),
        le_mesh: false,
        head_part: false,
        txst,
    };
    let slot0 = |v: &mut Viewport3d| {
        let t = Instant::now();
        loop {
            v.poll();
            v.request_frame(64, 64);
            if let Some(s) = v.shapes.first() {
                return (s.id, s.material.slots[0].norm.clone());
            }
            assert!(t.elapsed() < Duration::from_secs(60), "never loaded");
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    v.set_mesh(Some(spec(1, BTreeMap::new())));
    let (id, own) = slot0(&mut v);
    let mut slots = vec![String::new(); 9];
    slots[0] = "textures/landscape/dirt02.dds".into();
    v.set_content(
        Some(Content::Mesh(Arc::new(spec(
            1,
            BTreeMap::from([(id, slots)]),
        )))),
        true,
    );
    let (_, picked) = slot0(&mut v);
    assert_ne!(own, picked);
    assert_eq!(picked, "textures/landscape/dirt02.dds");
}
