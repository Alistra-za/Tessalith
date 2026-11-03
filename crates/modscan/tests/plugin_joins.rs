//! esp scanner spec §6 "Store joins": set → TXST, NIF → alternate texture.
#[path = "../../esp/tests/common/mod.rs"]
mod b;
mod common;
use b::{alt, edid, plugin, rec, sub, top, zstr};
use common::{instance_fixture, put};
use modscan::mo2::Instance;
use modscan::plugin_store::{ShapeSwap, TxstUse};
use modscan::scan::{run, Report, Source};
use modscan::store::{Store, TexKey};
use std::sync::atomic::AtomicBool;

const DIRT: u32 = 0x0000_0ABC;
const GHOST: u32 = 0x0000_0AB0;

/// Skyrim.esm: DirtTXST (TX00 dirt02, TX01 dirt02_n, TX02 old/gone), GhostTXST (missing texture), RockStat
/// (meshes/x.nif with Shape0 → DirtTXST), GhostStat (missing mesh). Nothing places them (no CELL group): since Note
/// GJ7 that does not hide a record. A.esp overrides DirtTXST without TX02. Loose textures: dirt02, dirt02_n,
/// effects/fire, unused/lonely, pbr/landscape/dirt02.
fn fixture() -> tempfile::TempDir {
    let d = instance_fixture("+B\n+A\n");
    let r = d.path();
    let nif = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../nif/tests/fixtures/TestNifFile_Skinned_SE.nif"
    ))
    .unwrap();
    put(r, "mods/A/meshes/x.nif", &nif);
    for t in [
        "landscape/dirt02.dds",
        "landscape/dirt02_n.dds",
        "effects/fire.dds",
        "unused/lonely.dds",
        "pbr/landscape/dirt02.dds",
    ] {
        put(r, &format!("mods/A/textures/{t}"), b"dds");
    }
    let dirt = |old: bool| {
        let mut s = vec![
            edid("DirtTXST"),
            sub(b"TX00", &zstr("Landscape\\Dirt02.dds")),
            sub(b"TX01", &zstr("Landscape\\Dirt02_n.dds")),
        ];
        if old {
            s.push(sub(b"TX02", &zstr("Old\\Gone.dds")));
        }
        rec(b"TXST", DIRT, &s)
    };
    put(
        r,
        "Game/Data/Skyrim.esm",
        &plugin(
            esp::FLAG_ESM,
            &[],
            &[
                top(
                    b"TXST",
                    &[
                        dirt(true),
                        rec(
                            b"TXST",
                            GHOST,
                            &[
                                edid("GhostTXST"),
                                sub(b"TX00", &zstr("Missing\\NotHere.dds")),
                            ],
                        ),
                    ],
                ),
                top(
                    b"STAT",
                    &[
                        rec(
                            b"STAT",
                            0xDEF,
                            &[
                                edid("RockStat"),
                                sub(b"MODL", &zstr("x.nif")),
                                sub(b"MODS", &alt(&[("Shape0", DIRT, 0)])),
                            ],
                        ),
                        rec(
                            b"STAT",
                            0xD00,
                            &[edid("GhostStat"), sub(b"MODL", &zstr("Missing\\Ghost.nif"))],
                        ),
                    ],
                ),
            ],
        ),
    );
    put(
        r,
        "mods/A/A.esp",
        &plugin(0, &["Skyrim.esm"], &[top(b"TXST", &[dirt(false)])]),
    );
    put(r, "profiles/Default/plugins.txt", b"*A.esp\n");
    d
}

fn mo2(d: &tempfile::TempDir, store: &mut Store) -> Report {
    let src = Source::Mo2(Instance::load(d.path(), None).unwrap());
    run(&src, store, &AtomicBool::new(false), &|_| {}, &|| vec![]).unwrap()
}

fn dirt_key() -> TexKey {
    TexKey::Suffix {
        slot: 0,
        stem: "landscape\\dirt02".into(),
    }
}

#[test]
fn set_key_finds_the_winning_txst() {
    let d = fixture();
    let mut s = Store::open(&d.path().join("scan.db")).unwrap();
    mo2(&d, &mut s);
    let want = TxstUse {
        formid: DIRT,
        edid: Some("DirtTXST".into()),
        plugin: "A.esp".into(),
        slot: 0,
        path: "textures/landscape/dirt02.dds".into(),
        n_overrides: 1,
    };
    assert_eq!(s.txsts_matching(&dirt_key(), 50).unwrap(), vec![want]);
    let normal = s
        .txsts_matching(
            &TexKey::Exact {
                slot: 1,
                path: "textures\\Landscape\\Dirt02_n.dds".into(),
            },
            50,
        )
        .unwrap();
    assert_eq!(normal.len(), 1);
    assert_eq!(normal[0].slot, 1);
    assert!(
        s.txsts_matching(
            &TexKey::Suffix {
                slot: 0,
                stem: "irt02".into()
            },
            50
        )
        .unwrap()
        .is_empty(),
        "whole components"
    );
}

#[test]
fn txst_users_and_mesh_overrides() {
    let d = fixture();
    let mut s = Store::open(&d.path().join("scan.db")).unwrap();
    mo2(&d, &mut s);
    let swap = ShapeSwap {
        shape_name: "Shape0".into(),
        txst_formid: DIRT,
        txst_edid: Some("DirtTXST".into()),
        index: 0,
    };
    let uses = s.plugin_uses_of_mesh("meshes/x.nif").unwrap();
    assert_eq!(uses.len(), 1);
    assert_eq!(
        (uses[0].formid, uses[0].model_slot, uses[0].n_overrides),
        (0xDEF, 0, 0)
    );
    assert_eq!(uses[0].swaps, vec![swap]);
    assert!(s
        .plugin_uses_of_mesh("meshes/other.nif")
        .unwrap()
        .is_empty());
}

/// Review Focus 4.
#[test]
fn a_folder_scan_hides_plugin_rows_and_the_next_mo2_scan_restores_them() {
    let d = fixture();
    let mut s = Store::open(&d.path().join("scan.db")).unwrap();
    mo2(&d, &mut s);
    let other = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(other.path().join("Loose")).unwrap();
    run(
        &Source::Folder(other.path().join("Loose")),
        &mut s,
        &AtomicBool::new(false),
        &|_| {},
        &|| vec![],
    )
    .unwrap();
    assert!(s.txsts_matching(&dirt_key(), 50).unwrap().is_empty());
    let r = mo2(&d, &mut s);
    let p = r.plugins.unwrap();
    assert_eq!(p.cached, p.n, "nothing re-parsed");
    assert_eq!(s.txsts_matching(&dirt_key(), 50).unwrap().len(), 1);
}

/// Skyrim.esm (master Missing.esm, not loaded) with an unresolved STAT whose raw FormID collides with a resolved one.
fn fixture2() -> tempfile::TempDir {
    let d = instance_fixture("+A\\n");
    let r = d.path();
    let stat =
        |id: u32, name: &str, m: &str| rec(b"STAT", id, &[edid(name), sub(b"MODL", &zstr(m))]);
    put(
        r,
        "Game/Data/Skyrim.esm",
        &plugin(
            esp::FLAG_ESM,
            &["Missing.esm"],
            &[top(
                b"STAT",
                &[
                    stat(0x0000_0ABC, "Orphan", "Missing\\Orphan.nif"),
                    stat(0x0100_0ABC, "Real", "Real.nif"),
                ],
            )],
        ),
    );
    put(r, "profiles/Default/plugins.txt", b"# none\\n");
    d
}
/// An unresolved record (owner_ok = 0) keeps a raw FormID that can equal a resolved winner's.
#[test]
fn an_unresolved_record_colliding_with_a_winner_is_ignored() {
    let d = fixture2();
    let mut s = Store::open(&d.path().join("scan.db")).unwrap();
    mo2(&d, &mut s);
    let ok: i64 = s
        .conn()
        .query_row(
            "SELECT count(*) FROM rec WHERE edid = 'Orphan' AND owner_ok = 0",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(ok, 1, "fixture: Orphan is unresolved");
    assert!(s
        .plugin_uses_of_mesh("meshes/missing/orphan.nif")
        .unwrap()
        .is_empty());
    let real = s.plugin_uses_of_mesh("meshes/real.nif").unwrap();
    assert_eq!(real.len(), 1);
    assert_eq!(real[0].edid.as_deref(), Some("Real"));
}

#[test]
fn zero_limit_matches_nothing() {
    let d = fixture();
    let mut s = Store::open(&d.path().join("scan.db")).unwrap();
    mo2(&d, &mut s);
    assert!(s.txsts_matching(&dirt_key(), 0).unwrap().is_empty());
}

/// Final review I1: a scan cancelled mid-parse leaves every origin disabled (the store reads as incomplete). The
/// record winners of the earlier scan are not touched by it (only a taint or a cancel at the plugin pass empties
/// them, below), so this pins the disabled origins and the `Cancelled` result.
#[test]
fn a_cancelled_scan_leaves_every_origin_disabled() {
    let d = fixture();
    let mut s = Store::open(&d.path().join("scan.db")).unwrap();
    mo2(&d, &mut s);
    let enabled = |s: &Store| -> i64 {
        s.conn()
            .query_row("SELECT count(*) FROM origin WHERE enabled = 1", [], |r| {
                r.get(0)
            })
            .unwrap()
    };
    assert!(enabled(&s) > 0, "fixture: a finished scan enables origins");
    for i in 0..600 {
        put(d.path(), &format!("mods/A/meshes/c{i}.nif"), b"x");
    }
    let cancel = AtomicBool::new(false);
    let src = Source::Mo2(Instance::load(d.path(), None).unwrap());
    let r = run(
        &src,
        &mut s,
        &cancel,
        &|p| {
            if p.phase == "parse" {
                cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        },
        &|| vec![],
    );
    assert!(matches!(r, Err(modscan::ScanError::Cancelled)), "{r:?}");
    assert_eq!(enabled(&s), 0);
}
/// Trace-debug wave 2 #7: a tainted scan (PGPatcher started mid-scan) empties the winners; the plugin rows are
/// then empty, not stale.
#[test]
fn a_tainted_scan_leaves_plugin_rows_empty() {
    let d = fixture();
    let mut s = Store::open(&d.path().join("scan.db")).unwrap();
    mo2(&d, &mut s);
    assert!(!s.txsts_matching(&dirt_key(), 50).unwrap().is_empty());
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let src = Source::Mo2(Instance::load(d.path(), None).unwrap());
    let r = run(&src, &mut s, &AtomicBool::new(false), &|_| {}, &|| {
        if calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
            vec![]
        } else {
            vec!["pgtools.exe".into()]
        }
    })
    .unwrap();
    assert!(r.tainted);
    assert!(s.txsts_matching(&dirt_key(), 50).unwrap().is_empty());
}

/// Codex review of BW: a scan cancelled at the start of the plugin pass has already replaced the asset winners;
/// the old record winners must not survive, and the next clean scan restores them.
#[test]
fn a_scan_cancelled_at_the_plugin_pass_leaves_plugin_rows_empty() {
    let d = fixture();
    let mut s = Store::open(&d.path().join("scan.db")).unwrap();
    mo2(&d, &mut s);
    assert!(!s.txsts_matching(&dirt_key(), 50).unwrap().is_empty());
    std::fs::write(d.path().join("profiles/Default/modlist.txt"), "-B\n-A\n").unwrap();
    let cancel = AtomicBool::new(false);
    let src = Source::Mo2(Instance::load(d.path(), None).unwrap());
    let r = run(
        &src,
        &mut s,
        &cancel,
        &|p| {
            if p.phase == "resolve" {
                cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        },
        &|| vec![],
    )
    .unwrap();
    assert!(r.plugins.is_none(), "{:?}", r.notes);
    assert!(s.txsts_matching(&dirt_key(), 50).unwrap().is_empty());
    std::fs::write(d.path().join("profiles/Default/modlist.txt"), "+B\n+A\n").unwrap();
    mo2(&d, &mut s);
    assert!(!s.txsts_matching(&dirt_key(), 50).unwrap().is_empty());
}
