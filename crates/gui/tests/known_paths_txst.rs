//! Note HG: the scan's `txst_slot.slot` is the raw TXST `TX0n` index; `known_paths::read` returns lighting slots.
use modscan::store::{CopyResult, OriginSpec, Store, KIND_MOD};
use sdc_gui::model::known_paths::{self, KnownPaths};

const FORM: u32 = 0x800;
/// (raw TX index, path); the lighting slot (0-based) each lands in is checked against `Store::txst_textures`.
const RECORD: [(i64, &str); 6] = [
    (0, "textures/a/x.dds"),
    (1, "textures/a/x_n.dds"),
    (2, "textures/a/x_rmaos.dds"),
    (3, "textures/a/x_sk.dds"),
    (4, "textures/a/x_p.dds"),
    (7, "textures/a/x_s.dds"),
];

fn path_id(c: &rusqlite::Connection, p: &str) -> i64 {
    c.execute("INSERT OR IGNORE INTO path (p) VALUES (?1)", [p])
        .unwrap();
    c.query_row("SELECT id FROM path WHERE p = ?1", [p], |r| r.get(0))
        .unwrap()
}

fn txst(c: &rusqlite::Connection, form: u32, rows: &[(i64, &str)]) {
    c.execute(
        "INSERT INTO rec (formid, local, plugin_id, sig, edid, owner_ok) VALUES (?1, ?1, 1, 'TXST', 'T', 1)",
        [form],
    )
    .unwrap();
    c.execute(
        "INSERT INTO rec_winner (formid, plugin_id, n_overrides) VALUES (?1, 1, 0)",
        [form],
    )
    .unwrap();
    for (tx, p) in rows {
        let pid = path_id(c, p);
        c.execute(
            "INSERT INTO txst_slot (formid, local, plugin_id, slot, path_id) VALUES (?1, ?1, 1, ?2, ?3)",
            rusqlite::params![form, tx, pid],
        )
        .unwrap();
    }
}

/// A temp scan with plugin `T.esp`, two winning TXSTs, and one live mesh whose set has a map in slot 5.
fn db(dir: &std::path::Path) -> std::path::PathBuf {
    let p = dir.join("scan.db");
    let mut s = Store::open(&p).unwrap();
    s.begin_scan().unwrap();
    let o = s
        .upsert_origin(&OriginSpec {
            kind: KIND_MOD,
            name: "M".into(),
            root: dir.join("mods/M").to_string_lossy().into_owned(),
            parent: None,
            priority: 0,
            derived_by: None,
            derived_state: None,
        })
        .unwrap();
    let mesh = "meshes/m/m.nif";
    let scan = nif::scan::NifScan {
        bsver: 100,
        le_mesh: false,
        shapes: vec![shape(&[
            "textures/m/d.dds",
            "",
            "",
            "",
            "",
            "textures/m/five.dds",
        ])],
        problems: vec![],
        index3d: Default::default(),
    };
    let id = s
        .upsert_copy(mesh, o, 1, 1, CopyResult::Scanned(scan))
        .unwrap();
    let pid = s.path_ids(&[mesh]).unwrap()[0];
    s.set_winners(&[(pid, (id, 1))].into_iter().collect(), &Default::default())
        .unwrap();
    let c = s.conn();
    c.execute(
        "INSERT INTO plugin (id, name, path, size, mtime, lo_index, light, flags, masters_json)
         VALUES (1, 'T.esp', 'T.esp', 1, 1, 0, 0, 0, '[]')",
        [],
    )
    .unwrap();
    txst(c, FORM, &RECORD);
    // A record without TX04: its TX03 is the skin tint, not a height.
    txst(
        c,
        FORM + 1,
        &[(0, "textures/a/y.dds"), (3, "textures/a/y_sk.dds")],
    );
    p
}

/// `complex_grass.rs`'s `lighting_shape`, trimmed: test binaries cannot share it.
fn shape(slots: &[&str]) -> nif::scan::ShapeRow {
    use nif::props::{Lighting, LightingParams, LightingTrailer};
    use nif::shape_head::{ShapeHead, ShapeKind};
    nif::scan::ShapeRow {
        block: 3,
        head: ShapeHead {
            kind: ShapeKind::BSTriShape,
            name: Some("S".into()),
            skinned: false,
            shader: 1,
            alpha: -1,
            vertex_desc: None,
            num_triangles: Some(1),
            num_vertices: Some(3),
        },
        shader: nif::scan::Shader::Lighting(
            Lighting {
                name: None,
                shader_type: 0,
                slsf1: 0,
                slsf2: 0,
                texture_set: 2,
                params: LightingParams {
                    uv_offset: [0.0; 2],
                    uv_scale: [1.0; 2],
                    emissive_color: [0.0; 3],
                    emissive_multiple: 1.0,
                    texture_clamp_mode: 3,
                    alpha: 1.0,
                    refraction_strength: 0.0,
                    glossiness: 80.0,
                    specular_color: [1.0; 3],
                    specular_strength: 1.0,
                    lighting_effect_1: 0.3,
                    lighting_effect_2: 2.0,
                    trailer: LightingTrailer::None,
                },
            },
            slots
                .iter()
                .map(|p| nif::path::canon_texture(p.as_bytes()))
                .collect(),
        ),
        alpha: None,
    }
}

fn read() -> (tempfile::TempDir, std::path::PathBuf, KnownPaths) {
    let d = tempfile::tempdir().unwrap();
    let p = db(d.path());
    let k = known_paths::read(&p).unwrap();
    (d, p, k)
}

#[test]
fn a_records_rmaos_is_supplied_in_slot_six() {
    let (_d, _p, k) = read();
    assert_ne!(k.supplies["textures/a/x.dds"] & (1 << 5), 0);
}

#[test]
fn a_records_sk_is_not_a_height() {
    let (_d, _p, k) = read();
    let bits = k.supplies["textures/a/y.dds"];
    assert_eq!(bits & (1 << 3), 0, "{bits:#b}");
    assert_ne!(bits & (1 << 2), 0, "{bits:#b}");
}

#[test]
fn a_records_height_is_slot_four() {
    let (_d, _p, k) = read();
    assert_ne!(k.supplies["textures/a/x.dds"] & (1 << 3), 0);
}

#[test]
fn a_records_texture_lists_under_its_slot() {
    let (_d, _p, k) = read();
    assert_eq!(k.textures["textures/a/x_rmaos.dds"], 1 << 5);
    assert_eq!(k.textures["textures/a/x_p.dds"], 1 << 3);
    assert_eq!(k.textures["textures/a/x_sk.dds"], 1 << 2);
}

#[test]
fn the_slots_match_txst_textures() {
    let (_d, p, k) = read();
    let tex = Store::open_read_only(&p)
        .unwrap()
        .txst_textures(FORM)
        .unwrap()
        .unwrap();
    for (_, path) in RECORD {
        let bit = k.textures[path].trailing_zeros() as usize;
        assert_eq!(k.textures[path].count_ones(), 1, "{path}");
        assert_eq!(tex[bit], path);
    }
}

#[test]
fn a_meshes_slots_are_unchanged() {
    let (_d, _p, k) = read();
    assert_eq!(k.textures["textures/m/five.dds"], 1 << 5);
    assert_ne!(k.supplies["textures/m/d.dds"] & (1 << 5), 0);
}

// ---- Task 6: the winning records the picker lists (Note HG) ----

use sdc_gui::model::txst_map::formid_text;

fn plugin(c: &rusqlite::Connection, id: i64, name: &str, masters: &[&str]) {
    c.execute(
        "INSERT INTO plugin (id, name, path, size, mtime, lo_index, light, flags, masters_json)
         VALUES (?1, ?2, ?2, 1, 1, ?1, 0, 0, ?3)",
        rusqlite::params![id, name, serde_json::to_string(masters).unwrap()],
    )
    .unwrap();
}

/// A TXST copy in plugin `pid`: `formid` is the load-order one, `local` the plugin's own. `wins` adds its
/// `rec_winner` row; `owner_ok` is the scan's flag.
fn copy(
    c: &rusqlite::Connection,
    (pid, local, formid): (i64, u32, u32),
    edid: &str,
    (wins, owner_ok): (bool, bool),
    rows: &[(i64, &str)],
) {
    c.execute(
        "INSERT INTO rec (formid, local, plugin_id, sig, edid, owner_ok) VALUES (?1, ?2, ?3, 'TXST', ?4, ?5)",
        rusqlite::params![formid, local, pid, edid, owner_ok],
    )
    .unwrap();
    if wins {
        c.execute(
            "INSERT INTO rec_winner (formid, plugin_id, n_overrides) VALUES (?1, ?2, 0)",
            rusqlite::params![formid, pid],
        )
        .unwrap();
    }
    for (tx, p) in rows {
        let path = path_id(c, p);
        c.execute(
            "INSERT INTO txst_slot (formid, local, plugin_id, slot, path_id) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![formid, local, pid, tx, path],
        )
        .unwrap();
    }
}

/// `db` plus the records of the owner's layout: a light plugin with five masters, an override, a losing copy, two
/// masters sharing one number, and a winner the scan flagged `owner_ok = 0`.
fn records_db(dir: &std::path::Path) -> std::path::PathBuf {
    let p = db(dir);
    let s = Store::open(&p).unwrap();
    let c = s.conn();
    let five = [
        "Skyrim.esm",
        "Update.esm",
        "Dawnguard.esm",
        "HearthFires.esm",
        "Dragonborn.esm",
    ];
    plugin(c, 2, "Arch.esp", &five);
    plugin(c, 3, "Patch.esp", &["Skyrim.esm"]);
    plugin(c, 4, "Old.esp", &["Skyrim.esm"]);
    plugin(c, 5, "Two.esp", &["A.esm", "B.esm"]);
    plugin(c, 6, "Off.esp", &[]);
    copy(
        c,
        (2, 0x0500_0814, 0xFE4C_D814),
        "Arch_skinbody_elf",
        (true, true),
        &[
            (0, "textures/arch/skin/elf/femalebody.dds"),
            (1, "textures/arch/skin/body_n.dds"),
            (2, "textures/arch/skin/body_rmaos.dds"),
            (3, "textures/arch/skin/body_sk.dds"),
            (7, "textures/arch/skin/elf/body_s.dds"),
        ],
    );
    copy(
        c,
        (3, 0x0001_2EB7, 0x0001_2EB7),
        "Won",
        (true, true),
        &[(0, "textures/won.dds")],
    );
    copy(
        c,
        (4, 0x0001_2EB7, 0x0001_2EB7),
        "Lost",
        (false, true),
        &[(0, "textures/lost.dds")],
    );
    copy(
        c,
        (5, 0x0000_0801, 0x2000_0801),
        "ZeroMaster",
        (true, true),
        &[(0, "textures/z0.dds")],
    );
    copy(
        c,
        (5, 0x0100_0801, 0x2100_0801),
        "OneMaster",
        (true, true),
        &[(0, "textures/z1.dds")],
    );
    copy(
        c,
        (6, 0x0000_0800, 0x2200_0800),
        "Flagged",
        (true, false),
        &[(0, "textures/f.dds")],
    );
    p
}

fn rec<'a>(k: &'a KnownPaths, edid: &str) -> &'a sdc_gui::model::known_paths::TxstRecord {
    k.records
        .iter()
        .find(|r| r.edid == edid)
        .unwrap_or_else(|| panic!("no record {edid}: {:?}", k.records))
}

fn records() -> (tempfile::TempDir, KnownPaths) {
    let d = tempfile::tempdir().unwrap();
    let k = known_paths::read(&records_db(d.path())).unwrap();
    (d, k)
}

#[test]
fn formid_text_drops_the_first_two_digits() {
    assert_eq!(formid_text(0x0500_0814), "0x814");
    assert_eq!(formid_text(0x0001_2EB7), "0x12EB7");
    assert_eq!(formid_text(0x0500_0FE6), "0xFE6");
}

#[test]
fn a_light_plugin_uses_its_own_copy() {
    let (_d, k) = records();
    let r = rec(&k, "Arch_skinbody_elf");
    assert_eq!(
        (r.formid.as_str(), r.plugin.as_str()),
        ("0x814", "Arch.esp")
    );
    assert_eq!(r.winner, "Arch.esp");
}

#[test]
fn an_override_is_keyed_by_its_master() {
    let (_d, k) = records();
    let r = rec(&k, "Won");
    assert_eq!(
        (r.formid.as_str(), r.plugin.as_str()),
        ("0x12EB7", "Skyrim.esm")
    );
    assert_eq!(r.winner, "Patch.esp");
}

#[test]
fn two_masters_with_one_number_stay_apart() {
    let (_d, k) = records();
    let (a, b) = (rec(&k, "ZeroMaster"), rec(&k, "OneMaster"));
    assert_eq!((a.formid.as_str(), b.formid.as_str()), ("0x801", "0x801"));
    assert_eq!((a.plugin.as_str(), b.plugin.as_str()), ("A.esm", "B.esm"));
}

#[test]
fn only_winners_are_listed() {
    let (_d, k) = records();
    let names: Vec<&str> = k.records.iter().map(|r| r.edid.as_str()).collect();
    assert!(!names.contains(&"Lost"), "{names:?}");
    assert!(!names.contains(&"Flagged"), "{names:?}");
    assert!(names.contains(&"Won"));
}

#[test]
fn records_keep_their_slots() {
    let (_d, k) = records();
    let r = rec(&k, "Arch_skinbody_elf");
    assert_eq!(r.slots[&6], "textures/arch/skin/body_rmaos.dds");
    assert_eq!(r.slots[&3], "textures/arch/skin/body_sk.dds");
    assert_eq!(r.slots[&8], "textures/arch/skin/elf/body_s.dds");
    assert_eq!(r.slots.keys().copied().collect::<Vec<_>>(), [1, 2, 3, 6, 8]);
}

/// Pin of the ordering: by EDID, not by plugin or FormID.
#[test]
fn records_are_ordered_by_edid() {
    let (_d, k) = records();
    let names: Vec<&str> = k.records.iter().map(|r| r.edid.as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort_by_key(|n| n.to_lowercase());
    assert_eq!(names, sorted);
}

#[test]
fn a_failing_record_query_keeps_the_pickers() {
    let d = tempfile::tempdir().unwrap();
    let p = db(d.path());
    // The record query joins `plugin`; without it only that query fails.
    Store::open(&p)
        .unwrap()
        .conn()
        .execute_batch("DROP TABLE plugin")
        .unwrap();
    let k = known_paths::read(&p).unwrap();
    assert!(k.records.is_empty());
    assert_eq!(k.textures["textures/a/x.dds"], 1 << 0);
}
