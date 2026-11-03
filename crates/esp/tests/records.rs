//! esp spec §3: one fixture per record type, layouts as cited in the plan (xEdit dev-4.1.6).
mod common;
use common::*;
use esp::{parse, AltTex, ModelRef, Rec};

fn one(group: &[u8; 4], r: Vec<u8>) -> Rec {
    let f = parse("x.esp", &plugin(0, &[], &[top(group, &[r])]));
    assert!(f.errors.is_empty(), "{:?}", f.errors);
    assert_eq!(f.records.len(), 1);
    f.records.into_iter().next().unwrap()
}

#[test]
fn txst_slots_decal_and_flags() {
    let r = one(
        b"TXST",
        rec(
            b"TXST",
            0x0100_0ABC,
            &[
                edid("DirtTXST"),
                sub(b"OBND", &[0; 12]),
                sub(b"TX00", &zstr("Landscape\\Dirt02.dds")),
                sub(b"TX01", &zstr("textures\\Landscape\\Dirt02_N.dds")),
                sub(b"TX07", &zstr("Data\\Textures\\landscape\\dirt02_s.dds")),
                sub(b"DODT", &[0; 36]),
                sub(b"DNAM", &3u16.to_le_bytes()),
            ],
        ),
    );
    assert_eq!(r.edid.as_deref(), Some("DirtTXST"));
    let t = r.payload.txst.unwrap();
    assert_eq!(t.slots[0].as_deref(), Some("textures/landscape/dirt02.dds"));
    assert_eq!(
        t.slots[1].as_deref(),
        Some("textures/landscape/dirt02_n.dds")
    );
    assert_eq!(
        t.slots[7].as_deref(),
        Some("textures/landscape/dirt02_s.dds")
    );
    assert!(t.slots[2..7].iter().all(Option::is_none));
    assert!(t.decal);
    assert_eq!(t.flags, 3);
}

#[test]
fn a_txst_without_dodt_is_not_a_decal() {
    let r = one(
        b"TXST",
        rec(b"TXST", 1, &[edid("Plain"), sub(b"TX00", &zstr("a.dds"))]),
    );
    let t = r.payload.txst.unwrap();
    assert!(!t.decal);
    assert_eq!(t.flags, 0);
}

/// Each alternate-texture list belongs to the model with its slot (MO2S → MOD2, MO4S → MOD4).
#[test]
fn models_carry_their_own_alternate_textures() {
    let r = one(
        b"ARMA",
        rec(
            b"ARMA",
            0x0100_0900,
            &[
                edid("BootsAA"),
                sub(b"MOD2", &zstr("Armor\\Boots\\M.nif")),
                sub(b"MO2T", &[0; 12]),
                sub(
                    b"MO2S",
                    &alt(&[("BootsShape", 0x0000_0ABC, 0), ("Strap", 0x0100_0ABD, 2)]),
                ),
                sub(b"MOD3", &zstr("Armor\\Boots\\F.nif")),
                sub(b"MOD4", &zstr("meshes\\Armor\\Boots\\M_1st.NIF")),
                sub(
                    b"MO4S",
                    &alt(&[("FirstPerson", 0x0000_0ABE, 1), ("NoTexture", 0, 3)]),
                ),
                form(b"NAM0", 0x0000_0D00),
                form(b"NAM1", 0),
                form(b"NAM2", 0x0000_0F00),
            ],
        ),
    );
    assert_eq!(
        r.payload.models,
        vec![
            ModelRef {
                slot: 2,
                path: "meshes/armor/boots/m.nif".into(),
                alt: vec![
                    AltTex {
                        shape_name: "BootsShape".into(),
                        txst_local_formid: 0xABC,
                        index: 0
                    },
                    AltTex {
                        shape_name: "Strap".into(),
                        txst_local_formid: 0x0100_0ABD,
                        index: 2
                    },
                ],
            },
            ModelRef {
                slot: 3,
                path: "meshes/armor/boots/f.nif".into(),
                alt: vec![]
            },
            ModelRef {
                slot: 4,
                path: "meshes/armor/boots/m_1st.nif".into(),
                alt: vec![AltTex {
                    shape_name: "FirstPerson".into(),
                    txst_local_formid: 0xABE,
                    index: 1
                }],
            },
        ],
        "an entry with TXST 0 is no swap"
    );
}

#[test]
fn every_model_record_type_reads_modl_and_mods() {
    for sig in [
        b"STAT", b"MSTT", b"ACTI", b"FURN", b"DOOR", b"CONT", b"LIGH", b"MISC", b"TREE", b"FLOR",
        b"GRAS", b"ARMA", b"WEAP", b"AMMO", b"BOOK", b"INGR", b"ALCH", b"KEYM", b"SLGM", b"SCRL",
        b"HDPT", b"TACT", b"ANIO", b"CLMT", b"IDLM", b"PROJ", b"HAZD", b"EXPL", b"ADDN", b"CAMS",
        b"ARTO", b"MATO", b"LVLN", b"APPA", b"SCOL", b"BPTD",
    ] {
        let r = one(
            sig,
            rec(
                sig,
                0x0100_0800,
                &[
                    edid("M"),
                    sub(b"MODL", &zstr("Clutter\\Thing.nif")),
                    sub(b"MODS", &alt(&[("S", 0xABC, 0)])),
                ],
            ),
        );
        let name = String::from_utf8_lossy(sig);
        assert_eq!(r.payload.models.len(), 1, "{name}");
        assert_eq!(
            r.payload.models[0].path, "meshes/clutter/thing.nif",
            "{name}"
        );
        assert_eq!(r.payload.models[0].alt.len(), 1, "{name}");
    }
}

#[test]
fn weapon_scope_and_destruction_models_have_their_own_slots() {
    let r = one(
        b"WEAP",
        rec(
            b"WEAP",
            1,
            &[
                sub(b"MODL", &zstr("Weapons\\Bow.nif")),
                sub(b"MOD3", &zstr("Weapons\\Scope.nif")),
                sub(b"MO3S", &alt(&[("Lens", 0xA0, 0)])),
                sub(b"DMDL", &zstr("Weapons\\Broken.nif")),
                sub(b"DMDS", &alt(&[("Shard", 0xA1, 0)])),
            ],
        ),
    );
    let slots: Vec<(u8, usize)> = r
        .payload
        .models
        .iter()
        .map(|m| (m.slot, m.alt.len()))
        .collect();
    assert_eq!(slots, [(0, 0), (3, 1), (6, 1)]);
}

/// A list with no model of its slot before it has nothing to attach to; ARMO's ICON (inventory art) is not in §3.
#[test]
fn unlisted_subrecords_and_orphan_lists_are_skipped() {
    let r = one(
        b"ARMO",
        rec(
            b"ARMO",
            1,
            &[
                sub(b"MO2S", &alt(&[("X", 0xA0, 0)])),
                sub(b"ICON", &zstr("Armor\\Icon.dds")),
            ],
        ),
    );
    assert!(r.payload.models.is_empty() && r.payload.txst.is_none());
}

/// Note GJ7: types whose only facts fed the dropped reference tables (LTEX, NPC_, EFSH, FLST) are not read at all.
#[test]
fn reference_only_record_types_are_skipped() {
    for sig in [b"LTEX", b"NPC_", b"EFSH", b"FLST"] {
        let f = parse(
            "x.esp",
            &plugin(
                0,
                &[],
                &[top(sig, &[rec(sig, 1, &[edid("X"), form(b"TNAM", 0xABC)])])],
            ),
        );
        assert!(f.errors.is_empty(), "{:?}", f.errors);
        assert!(f.records.is_empty(), "{}", String::from_utf8_lossy(sig));
    }
}

#[test]
fn head_part_model_is_read_and_tri_parts_are_not() {
    let h = one(
        b"HDPT",
        rec(
            b"HDPT",
            2,
            &[
                edid("Brows"),
                sub(b"MODL", &zstr("Actors\\Character\\Brows.nif")),
                form(b"TNAM", 0xABD),
                sub(b"NAM1", &zstr("Actors\\Character\\Brows.tri")),
            ],
        ),
    );
    assert_eq!(h.payload.models.len(), 1, "NAM1 .tri parts are not models");
    assert_eq!(
        h.payload.models[0].path,
        "meshes/actors/character/brows.nif"
    );
}

#[test]
fn race_models_are_read_and_dds_paths_are_not() {
    let r = one(
        b"RACE",
        rec(
            b"RACE",
            4,
            &[
                edid("NordRace"),
                form(b"WNAM", 0x0000_0D64),
                sub(
                    b"ANAM",
                    &zstr("Actors\\Character\\Character Assets\\skeleton.nif"),
                ),
                sub(
                    b"MODL",
                    &zstr("Actors\\Character\\Character Assets\\MaleBody.dds"),
                ),
                sub(
                    b"MODL",
                    &zstr("Actors\\Character\\Character Assets\\Head.nif"),
                ),
                form(b"DFTM", 0xA01),
                form(b"DFTF", 0xA02),
                form(b"FTSM", 0xA03),
                form(b"FTSF", 0xA04),
                sub(
                    b"TINT",
                    &zstr("Actors\\Character\\Character Assets\\TintMasks\\Cheek.dds"),
                ),
            ],
        ),
    );
    let models: Vec<&str> = r.payload.models.iter().map(|m| m.path.as_str()).collect();
    assert_eq!(
        models,
        [
            "meshes/actors/character/character assets/skeleton.nif",
            "meshes/actors/character/character assets/head.nif"
        ]
    );
}

/// WTHR is read for its aurora model only (Note GJ7: the cloud-layer textures are not read).
#[test]
fn weather_aurora_model_is_read() {
    let subs = [
        edid("SkyrimClear"),
        sub(b"00TX", &zstr("Sky\\Cloud00.dds")),
        sub(b"MODL", &zstr("Sky\\Aurora.nif")),
    ];
    let r = one(b"WTHR", rec(b"WTHR", 6, &subs));
    let models: Vec<&str> = r.payload.models.iter().map(|m| m.path.as_str()).collect();
    assert_eq!(models, ["meshes/sky/aurora.nif"]);
}

#[test]
fn race_dds_classification_ignores_trailing_space() {
    let r = one(
        b"RACE",
        rec(b"RACE", 4, &[sub(b"MODL", &zstr("Actors\\Body.dds "))]),
    );
    assert!(r.payload.models.is_empty());
}

fn weap_alt(d: &[u8]) -> Rec {
    one(
        b"WEAP",
        rec(b"WEAP", 1, &[sub(b"MODL", &zstr("a.nif")), sub(b"MODS", d)]),
    )
}

#[test]
fn truncated_and_odd_subrecords_do_not_panic_or_lose_earlier_data() {
    // 2 declared entries, the 2nd cut off after its name length.
    let mut d = 2u32.to_le_bytes().to_vec();
    for (name, txst, idx) in [("A", 0xABCu32, -1i32), ("B", 0xABD, 0)] {
        d.extend((name.len() as u32).to_le_bytes());
        d.extend(name.as_bytes());
        d.extend(txst.to_le_bytes());
        d.extend(idx.to_le_bytes());
    }
    d.truncate(d.len() - 6);
    let alt = &weap_alt(&d).payload.models[0].alt;
    assert_eq!(alt.len(), 1);
    assert_eq!(
        (alt[0].shape_name.as_str(), alt[0].index),
        ("A", -1),
        "negative index round-trips"
    );

    // Oversized name length: nothing read, no panic.
    let mut d = 1u32.to_le_bytes().to_vec();
    d.extend(u32::MAX.to_le_bytes());
    d.extend([0; 8]);
    assert!(weap_alt(&d).payload.models[0].alt.is_empty());

    // Short DNAM keeps flags 0.
    let t = one(b"TXST", rec(b"TXST", 1, &[sub(b"DNAM", &[7])]))
        .payload
        .txst
        .unwrap();
    assert_eq!(t.flags, 0);
}

/// xEdit dev-4.1.6 `wbDefinitionsTES5.pas` l.6238-6276: the IPCT model is read like every model record (ruling R11);
/// its texture sets and sounds are not read (Note GJ7).
#[test]
fn impact_model_is_read() {
    let r = one(
        b"IPCT",
        rec(
            b"IPCT",
            1,
            &[
                edid("Imp"),
                sub(b"MODL", &zstr("Effects\\Imp.nif")),
                sub(b"DATA", &[0u8; 24]),
                form(b"DNAM", 0xA1),
                form(b"ENAM", 0xA2),
                form(b"SNAM", 0xFF),
            ],
        ),
    );
    let models: Vec<&str> = r.payload.models.iter().map(|m| m.path.as_str()).collect();
    assert_eq!(models, ["meshes/effects/imp.nif"]);
}

/// Ruling F-2: Skyrim ARMO MODL is an ARMA FormID (per armature), not a mesh path; world models are MOD2/MOD4.
#[test]
fn armor_modl_names_armatures() {
    let r = one(
        b"ARMO",
        rec(
            b"ARMO",
            6,
            &[
                edid("Arm"),
                form(b"MODL", 0xA1),
                form(b"MODL", 0),
                sub(b"MOD2", &zstr("Armor\\X.nif")),
            ],
        ),
    );
    assert_eq!(
        r.payload.models.len(),
        1,
        "MODL is an ARMA FormID, not a path"
    );
    assert_eq!(r.payload.models[0].slot, 2);
}

/// Re-review: the WRLD record's cloud model (xEdit `wbRecord(WRLD…)`, `wbGenericModel`) is read with its alternate
/// textures; the world's children (cells, references) are skipped (Note GJ7).
#[test]
fn a_worldspace_cloud_model_is_read_and_its_children_are_skipped() {
    let id = 0x3000u32.to_le_bytes();
    let refr = rec(b"REFR", 0x3100, &[form(b"NAME", 0xB01)]);
    let f = parse(
        "x.esp",
        &plugin(
            0,
            &[],
            &[top(
                b"WRLD",
                &[
                    rec(
                        b"WRLD",
                        0x3000,
                        &[
                            edid("World"),
                            sub(b"MODL", &zstr("Sky/Clouds.nif")),
                            sub(b"MODS", &alt(&[("S", 0xA01, 0)])),
                        ],
                    ),
                    grup(&id, 1, &[grup(&id, 8, &[refr])]),
                ],
            )],
        ),
    );
    assert!(f.errors.is_empty(), "{:?}", f.errors);
    assert_eq!(f.records.len(), 1, "the REFR is not a record");
    let m = &f.records[0].payload.models;
    assert_eq!((m.len(), m[0].alt.len()), (1, 1));
}

/// Design §6 W3: the MATO DATA flags u32 (bit 0 = single pass) sits at offset 44; 52-byte records add a trailing word.
#[test]
fn mato_data_flags_are_read_when_present() {
    let data = |n: usize, flags: u32| {
        let mut d = vec![0u8; n];
        if n >= 48 {
            d[44..48].copy_from_slice(&flags.to_le_bytes());
        }
        if n >= 52 {
            d[48..52].copy_from_slice(&7u32.to_le_bytes());
        }
        d
    };
    for (id, n) in [(0x0100_0A00, 48), (0x0100_0A01, 52)] {
        let r = one(
            b"MATO",
            rec(b"MATO", id, &[edid("SnowMat1P"), sub(b"DATA", &data(n, 1))]),
        );
        assert_eq!(r.payload.mato_flags, Some(1));
    }
    let r = one(
        b"MATO",
        rec(
            b"MATO",
            0x0100_0A02,
            &[edid("Short"), sub(b"DATA", &data(44, 1))],
        ),
    );
    assert_eq!(r.payload.mato_flags, None);
    let r = one(b"MATO", rec(b"MATO", 0x0100_0A03, &[edid("NoData")]));
    assert_eq!(r.payload.mato_flags, None);
}
