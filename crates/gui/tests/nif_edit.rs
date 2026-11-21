use nif::edit::{Anchor, Op, Role};
use nif::fields::{Field, Value};
use sdc_gui::model::meshes::edit_doc::*;
use std::path::Path;

fn sample_op() -> Op {
    Op::SetField {
        at: Anchor {
            block: 3,
            kind: "BSLightingShaderProperty".into(),
            shape: Some("Shape".into()),
            role: Role::Shader,
        },
        field: Field::ShaderType,
        value: Value::U32(1),
        base: Value::U32(0),
    }
}

fn doc() -> EditDoc {
    EditDoc::new(SourceId {
        origin: "A".into(),
        data_path: "meshes/x.nif".into(),
        archive: None,
        size: 3,
        sha256: "ab".into(),
    })
}

#[test]
fn edit_doc_round_trips_and_tracks_pending() {
    let mut d = doc();
    assert!(!d.pending());
    d.ops.push(DocOp {
        op: sample_op(),
        status: OpStatus::Applied,
    });
    assert!(d.pending());
    let t = d.to_text();
    assert_eq!(EditDoc::from_text(&t).unwrap(), d);
    d.export.ops_hash = Some(d.ops_hash());
    assert!(!d.pending());
    assert!(!d.has_conflicts());
    d.ops[0].status = OpStatus::Conflict;
    assert!(d.has_conflicts());
    assert_eq!(d.ops().len(), 1);
}

#[test]
fn negative_zero_survives_the_document() {
    let mut d = doc();
    let op = Op::SetField {
        at: Anchor {
            block: 3,
            kind: "BSLightingShaderProperty".into(),
            shape: None,
            role: Role::Shader,
        },
        field: Field::UvOffset(0),
        value: Value::F32((-0.0f32).to_bits()),
        base: Value::F32(0.0f32.to_bits()),
    };
    d.ops.push(DocOp {
        op,
        status: OpStatus::Applied,
    });
    let back = EditDoc::from_text(&d.to_text()).unwrap();
    match &back.ops[0].op {
        Op::SetField {
            value: Value::F32(b),
            ..
        } => assert_eq!(*b, 0x8000_0000),
        o => panic!("{o:?}"),
    }
}

use nif::keys::{Key, KeyData, KeyGroup};

fn fkey(time: f32, value: f32, tbc: Option<[f32; 3]>) -> Key<f32> {
    Key {
        time,
        value,
        forward: None,
        backward: None,
        tbc,
    }
}

#[test]
fn key_bits_survive_the_document() {
    let mut d = doc();
    let op = Op::SetKeys {
        at: sample_op_anchor(),
        clip: None,
        clip_name: None,
        keys: KeyData::Float(KeyGroup {
            key_type: Some(1),
            keys: vec![fkey(-0.0, f32::from_bits(0x7FC0_0001), None)],
        }),
        base: KeyData::Float(KeyGroup {
            key_type: None,
            keys: vec![],
        }),
    };
    d.ops.push(DocOp {
        op,
        status: OpStatus::Applied,
    });
    let back = EditDoc::from_text(&d.to_text()).unwrap();
    match &back.ops[0].op {
        Op::SetKeys {
            keys: KeyData::Float(g),
            ..
        } => {
            assert_eq!(g.keys[0].time.to_bits(), 0x8000_0000);
            assert_eq!(g.keys[0].value.to_bits(), 0x7FC0_0001);
        }
        o => panic!("{o:?}"),
    }
}

fn sample_op_anchor() -> Anchor {
    match sample_op() {
        Op::SetField { at, .. } => at,
        _ => unreachable!(),
    }
}

#[test]
fn doc_and_export_paths_follow_the_spec() {
    let m = Path::new(r"C:\I\mods\sdc NIF Edits");
    assert_eq!(
        doc_path(m, "meshes/clutter/foo.nif").unwrap(),
        Path::new(
            r"C:\I\mods\sdc NIF Edits\.sdc\nif-edits\meshes\clutter\foo.nif.sdc-nifedit.toml"
        )
    );
    assert_eq!(
        export_path(m, "meshes/clutter/foo.nif").unwrap(),
        Path::new(r"C:\I\mods\sdc NIF Edits\meshes\clutter\foo.nif")
    );
}

const ESCAPING: [&str; 5] = [
    r"..\x.nif",
    r"C:\x.nif",
    r"\x.nif",
    r"meshes\..\..\x.nif",
    "meshes/a:b.nif",
];

/// Final review I3: a data path from a crafted BSA must not lead a write out of the edit mod.
#[test]
fn a_data_path_that_leaves_the_edit_mod_is_refused() {
    let m = Path::new(r"C:\I\mods\sdc NIF Edits");
    for p in ESCAPING {
        let want = format!("This NIF's path cannot be used for an edit: {p}");
        assert_eq!(doc_path(m, p), Err(want.clone()), "{p}");
        assert_eq!(export_path(m, p), Err(want), "{p}");
    }
}

#[test]
fn an_edit_on_an_escaping_path_does_not_open() {
    let d = tempfile::tempdir().unwrap();
    let edit_mod = d.path().join("mods").join(EDIT_MOD);
    for p in ESCAPING {
        let r = NifEdit::open(
            fixture("TestNifFile_Skinned_SE.nif"),
            copy("A", 1, false),
            p,
            None,
            Some(&edit_mod),
            None,
        );
        assert_eq!(
            r.err(),
            Some(format!("This NIF's path cannot be used for an edit: {p}")),
            "{p}"
        );
    }
    assert!(!edit_mod.exists(), "nothing written");
}

#[test]
fn save_and_load_use_a_temp_dir() {
    let dir = std::env::temp_dir().join(format!("sdc-nifedit-{}", std::process::id()));
    let p = dir.join("sub").join("a.toml");
    assert_eq!(load(&p).unwrap(), None);
    let mut d = doc();
    d.ops.push(DocOp {
        op: sample_op(),
        status: OpStatus::Applied,
    });
    save(&p, &d).unwrap();
    assert_eq!(load(&p).unwrap(), Some(d));
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn history_merges_drags_caps_at_200_and_redoes() {
    let mut h = NifHistory::default();
    assert!(!h.can_undo() && !h.can_redo());
    h.record("a".into(), "Drag", Some("drag".into()));
    h.record("b".into(), "Drag", Some("drag".into()));
    assert_eq!(
        h.undo_label(),
        Some("Drag"),
        "a merged drag is one named step"
    );
    assert_eq!(h.undo("now".into()), Some("a".into()));
    assert!(!h.can_undo() && h.can_redo());
    assert_eq!((h.undo_label(), h.redo_label()), (None, Some("Drag")));
    assert_eq!(h.redo("a".into()), Some("now".into()));

    let mut h = NifHistory::default();
    h.record("a".into(), "Drag", Some("drag".into()));
    h.break_merge();
    h.record("b".into(), "Drag", Some("drag".into()));
    assert_eq!(h.undo("c".into()), Some("b".into()));
    assert_eq!(h.undo("b".into()), Some("a".into()));
    assert_eq!(h.undo("a".into()), None);

    h.record("x".into(), "X", None);
    assert!(!h.can_redo());

    let mut h = NifHistory::default();
    for i in 0..NifHistory::LIMIT + 5 {
        h.record(i.to_string(), "Step", None);
    }
    let mut n = 0;
    let mut last = String::new();
    while let Some(s) = h.undo("cur".into()) {
        n += 1;
        last = s;
    }
    assert_eq!(n, NifHistory::LIMIT);
    assert_eq!(last, "5");
}

/// B-P41 (final review minor 6): AddController's frequency, start and stop are stored as bits, like key floats.
#[test]
fn add_controller_times_round_trip_as_bits() {
    let op = Op::AddController {
        at: Anchor {
            block: 3,
            kind: "BSLightingShaderProperty".into(),
            shape: Some("S".into()),
            role: Role::Shader,
        },
        class: "BSLightingShaderPropertyFloatController".into(),
        var: Some(9),
        keys: KeyData::Float(KeyGroup {
            key_type: Some(1),
            keys: vec![],
        }),
        flags: 0x48,
        freq: f32::from_bits(0xFFC0_0001), // a NaN with a payload and the sign bit
        start: -0.0,
        stop: f32::INFINITY,
    };
    let mut d = doc();
    d.ops = vec![DocOp {
        op,
        status: OpStatus::Applied,
    }];
    let back = EditDoc::from_text(&d.to_text()).unwrap();
    let Op::AddController {
        freq, start, stop, ..
    } = &back.ops[0].op
    else {
        panic!("{:?}", back.ops)
    };
    assert_eq!(
        [freq, start, stop].map(|f| f.to_bits()),
        [0xFFC0_0001, 0x8000_0000, f32::INFINITY.to_bits()]
    );
    // A document saved before B-P41 holds plain numbers: it still opens.
    let t = d.to_text();
    assert!(t.contains("freq = 4290772993"), "{t}");
    let old = EditDoc::from_text(&t.replace("freq = 4290772993", "freq = 1.5")).unwrap();
    assert!(matches!(old.ops[0].op, Op::AddController { freq, .. } if freq == 1.5));
}

#[test]
fn every_op_variant_round_trips() {
    use nif::edit::Owner;
    use nif::extra::ExtraValue as E;
    let shader = Anchor {
        block: 3,
        kind: "BSLightingShaderProperty".into(),
        shape: Some("S".into()),
        role: Role::Shader,
    };
    let ctrl = Anchor {
        block: 9,
        kind: "BSLightingShaderPropertyFloatController".into(),
        shape: Some("S".into()),
        role: Role::Ctrl {
            owner: Owner::Shader,
            class: "BSLightingShaderPropertyFloatController".into(),
            var: Some(2),
            nth: 1,
        },
    };
    let extra = Anchor {
        block: 4,
        kind: "NiStringExtraData".into(),
        shape: None,
        role: Role::Extra {
            on_root: true,
            name: "n".into(),
            kind: "NiStringExtraData".into(),
            nth: 0,
        },
    };
    let mut ops = vec![
        sample_op(),
        Op::SetBits {
            at: shader.clone(),
            field: Field::Slsf1,
            mask: 0xF0,
            value: 0x30,
            base: 0x10,
        },
        Op::SetTexturePath {
            at: shader.clone(),
            slot: 1,
            path: "a\\b.dds".into(),
            base: "".into(),
        },
        Op::SplitAlpha { at: shader.clone() },
        Op::AddAlpha {
            at: shader.clone(),
            flags: 4845,
            threshold: 128,
        },
        Op::RemoveAlpha { at: ctrl.clone() },
        Op::EditExtra {
            at: extra.clone(),
            value: E::Int(1),
            base: E::Int(2),
        },
        Op::RemoveExtra { at: extra.clone() },
        Op::SetKeys {
            at: ctrl.clone(),
            clip: Some(2),
            clip_name: Some("Open".into()),
            keys: KeyData::Float(KeyGroup {
                key_type: Some(1),
                keys: vec![fkey(-0.0, 1e38, None), fkey(1.0, -0.0, None)],
            }),
            base: KeyData::Pos(KeyGroup {
                key_type: Some(2),
                keys: vec![Key {
                    time: 0.5,
                    value: [1.0, -2.0, 3.0],
                    forward: Some([0.1, 0.2, 0.3]),
                    backward: Some([-0.1, -0.2, -0.3]),
                    tbc: None,
                }],
            }),
        },
        Op::SetKeys {
            at: ctrl.clone(),
            clip: None,
            clip_name: None,
            keys: KeyData::Float(KeyGroup {
                key_type: Some(3),
                keys: vec![fkey(0.0, 2.0, Some([0.5, -0.25, 1.0]))],
            }),
            base: KeyData::Bool(KeyGroup {
                key_type: None,
                keys: vec![],
            }),
        },
        Op::AddController {
            at: shader.clone(),
            class: "BSLightingShaderPropertyFloatController".into(),
            var: Some(9),
            keys: KeyData::Float(KeyGroup {
                key_type: Some(1),
                keys: vec![fkey(0.0, 1.0, None), fkey(2.0, -0.0, None)],
            }),
            flags: 0x48,
            freq: 1.0,
            start: 0.0,
            stop: 2.0,
        },
        Op::AddController {
            at: shader.clone(),
            class: "BSNiAlphaPropertyTestRefController".into(),
            var: None,
            keys: KeyData::Pos(KeyGroup {
                key_type: Some(1),
                keys: vec![Key {
                    time: 0.0,
                    value: [1.0, 0.5, 0.25],
                    forward: None,
                    backward: None,
                    tbc: None,
                }],
            }),
            flags: 0x48,
            freq: 1.0,
            start: 0.0,
            stop: 2.0,
        },
        Op::RemoveController { at: ctrl.clone() },
    ];
    for v in [
        E::Bsx(7),
        E::Str("a\r\nb".into()),
        E::Int(5),
        E::Bool(1),
        E::Float((-0.0f32).to_bits()),
        E::Inv {
            rot: [1, 2, 3],
            zoom: 9,
        },
    ] {
        ops.push(Op::AddExtra {
            at: extra.clone(),
            kind: "k".into(),
            name: "n".into(),
            value: v,
        });
    }
    let mut d = doc();
    d.ops = ops
        .into_iter()
        .map(|op| DocOp {
            op,
            status: OpStatus::Applied,
        })
        .collect();
    assert_eq!(EditDoc::from_text(&d.to_text()).unwrap(), d);
    assert_eq!(EditDoc::from_text(&doc().to_text()).unwrap(), doc());
}

// Documents written before clip editing was removed (Note GR2): the variants as the serializer wrote them.
const DOC_HEAD: &str = "version = 1\n\n[source]\norigin = \"A\"\ndata_path = \"meshes/x.nif\"\nsize = 3\nsha256 = \"ab\"\n\n";
const OP_SET_FIELD: &str = r#"[[ops]]
status = "Conflict"

[ops.op.SetField]
field = "ShaderType"

[ops.op.SetField.at]
block = 3
kind = "BSLightingShaderProperty"
shape = "Shape"
role = "Shader"

[ops.op.SetField.value]
U32 = 1

[ops.op.SetField.base]
U32 = 0

"#;
const OP_ADD_SEQUENCE: &str = r#"[[ops]]
status = "Applied"

[ops.op.AddSequence]
name = "Open"
cycle = 2
freq = 1065353216
start = 2147483648
stop = 4290772993
tracks = []

[ops.op.AddSequence.at]
block = 0
kind = "NiNode"
role = "Root"

"#;
const OP_RENAME_SEQUENCE: &str = r#"[[ops]]
status = "Applied"

[ops.op.RenameSequence]
name = "Open"
nth = 0
new_name = "Close"

[ops.op.RenameSequence.at]
block = 0
kind = "NiNode"
role = "Root"

"#;
const OP_SET_KEYS_CLIP: &str = r#"[[ops]]
status = "Applied"

[ops.op.SetKeys]
clip = 2
clip_name = "Open"

[ops.op.SetKeys.at]
block = 9
kind = "BSLightingShaderPropertyFloatController"
shape = "S"

[ops.op.SetKeys.at.role.Ctrl]
owner = "Shader"
class = "BSLightingShaderPropertyFloatController"
var = 2
nth = 1

[ops.op.SetKeys.keys.Float]
key_type = 1

[[ops.op.SetKeys.keys.Float.keys]]
time = 1056964608
value = 1065353216

[ops.op.SetKeys.base.Float]
key_type = 1
keys = []

"#;
const EXPORTED: &str = "[export]\nsha256 = \"ee\"\nops_hash = \"0123\"\n\n";
const CLIP_ONLY_LINE: &str =
    "meshes/x.nif: 2 clip edit(s) dropped: NIF Edit no longer edits clips.";

fn clip_only_text() -> String {
    [DOC_HEAD, EXPORTED, OP_ADD_SEQUENCE, OP_RENAME_SEQUENCE].concat()
}

/// A session on x.nif in the project `project` (its draft is under it), no export folder.
fn open_with(project: &Path, autosave: Option<&Path>) -> NifEdit {
    NifEdit::open(
        fixture("TestNifFile_Skinned_SE.nif"),
        copy("A", 1, false),
        "meshes/x.nif",
        Some(project),
        None,
        autosave,
    )
    .unwrap()
}

#[test]
fn a_document_of_only_clip_ops_opens_empty_and_says_so() {
    let d = tempfile::tempdir().unwrap();
    let p = draft_path(d.path(), "meshes/x.nif").unwrap();
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, clip_only_text()).unwrap();
    let e = open_with(d.path(), None);
    assert_eq!(e.doc.ops.len(), 0);
    assert!(
        e.doc.pending(),
        "the export has the clip edits, this does not"
    );
    assert_eq!(e.open_lines, vec![CLIP_ONLY_LINE.to_string()]);
}

#[test]
fn a_mixed_document_keeps_its_other_ops_in_order() {
    let t = [
        DOC_HEAD,
        OP_SET_FIELD,
        OP_ADD_SEQUENCE,
        OP_SET_KEYS_CLIP,
        OP_RENAME_SEQUENCE,
    ]
    .concat();
    let doc = EditDoc::from_text(&t).unwrap();
    assert_eq!(doc.dropped_clip_ops, 2);
    assert_eq!(doc.ops.len(), 2);
    assert!(matches!(doc.ops[0].op, Op::SetField { .. }));
    assert_eq!(doc.ops[0].status, OpStatus::Conflict);
    assert!(matches!(
        &doc.ops[1].op,
        Op::SetKeys { clip: Some(2), clip_name: Some(n), .. } if n == "Open"
    ));
    assert_eq!(doc.ops[1].status, OpStatus::Applied);
    assert_eq!(
        EditDoc::from_text(&doc.to_text()).unwrap().dropped_clip_ops,
        0
    );
}

#[test]
fn an_autosave_with_clip_ops_recovers() {
    let d = tempfile::tempdir().unwrap();
    let auto = d.path().join("auto.toml");
    std::fs::write(&auto, clip_only_text()).unwrap();
    let e = open_with(&d.path().join("mod"), Some(&auto));
    assert!(e.open_lines.iter().all(|l| !l.contains("does not load")));
    assert_eq!(
        e.open_lines,
        vec![
            format!(
                "Recovered the edits of meshes/x.nif autosaved in {}.",
                auto.display()
            ),
            CLIP_ONLY_LINE.to_string(),
        ]
    );
    assert_eq!(e.doc.ops.len(), 0);
}

#[test]
fn the_badge_scan_reads_a_document_with_clip_ops() {
    let d = tempfile::tempdir().unwrap();
    let p = doc_path(d.path(), "meshes/x.nif").unwrap();
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, clip_only_text()).unwrap();
    assert_eq!(
        pending_docs(d.path()),
        vec![("meshes/x.nif".to_string(), DocBadge::Pending)]
    );
}

/// Final review I4: a typo in any of the five names would make a whole document fail to open.
#[test]
fn every_removed_clip_op_name_is_dropped() {
    for name in [
        "AddSequence",
        "RemoveSequence",
        "RenameSequence",
        "SetSequenceTiming",
        "ConvertChainToClips",
    ] {
        let t =
            format!("{DOC_HEAD}[[ops]]\nstatus = \"Applied\"\n\n[ops.op.{name}]\nname = \"x\"\n");
        let d = EditDoc::from_text(&t).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!((d.ops.len(), d.dropped_clip_ops), (0, 1), "{name}");
    }
}

/// Final review I4: pending follows the hash of the ops that are left, not a constant.
#[test]
fn pending_follows_the_hash_of_the_ops_left_after_the_drop() {
    let mixed = |hash: &str| {
        let export = format!("[export]\nsha256 = \"ee\"\nops_hash = \"{hash}\"\n\n");
        [DOC_HEAD, &export, OP_SET_FIELD, OP_ADD_SEQUENCE].concat()
    };
    let real = EditDoc::from_text(&mixed("0")).unwrap().ops_hash();
    let d = EditDoc::from_text(&mixed(&real)).unwrap();
    assert_eq!(d.dropped_clip_ops, 1);
    assert!(!d.pending(), "the hash of the kept ops matches");
    assert!(EditDoc::from_text(&mixed("0123")).unwrap().pending());
    // a clip-only document exported before the drop: its exported hash was of ops that are gone
    let mut gone = EditDoc::from_text(&clip_only_text()).unwrap();
    assert!(gone.pending());
    gone.export.ops_hash = Some(EditDoc::new(gone.source.clone()).ops_hash());
    assert!(
        !gone.pending(),
        "an empty op list with its own hash is not pending"
    );
}

/// Final review Minor 5: a bad field reports its line, with or without clip ops in the document.
#[test]
fn a_bad_field_in_a_document_reports_its_line() {
    let bad = OP_SET_FIELD.replace("Conflict", "Bogus");
    let e = EditDoc::from_text(&[DOC_HEAD, &bad].concat()).unwrap_err();
    assert!(e.contains("line") && e.contains("Bogus"), "{e}");
    let e = EditDoc::from_text(&[DOC_HEAD, OP_ADD_SEQUENCE, &bad].concat()).unwrap_err();
    assert!(
        e.contains("line") && e.contains("Bogus"),
        "with clip ops: {e}"
    );
}

/// Final review I1: a clip key edit whose `AddSequence` (or Convert) was dropped names a clip that is no longer
/// there. It is never applied and never panics: the row is red (Conflict or Refused), the bytes are the source's, and
/// Revert clears it.
#[test]
fn a_clip_key_edit_that_depended_on_a_dropped_clip_op_does_not_apply() {
    let bytes = with_uv_ctrl(&fixture("TestNifFile_Skinned_SE.nif"));
    let open = |project: Option<&Path>| {
        NifEdit::open(
            bytes.clone(),
            copy("A", 1, false),
            "meshes/x.nif",
            project,
            None,
            None,
        )
        .unwrap()
    };
    let probe = open(None);
    let (shape, _) = lit_shape(&probe);
    let role = Role::Ctrl {
        owner: nif::edit::Owner::Shader,
        class: "BSLightingShaderPropertyFloatController".into(),
        var: Some(20),
        nth: 0,
    };
    let at = nif::edit::anchor_role(&probe.model, Some(shape), role).unwrap();
    let key = |time: f32, value: f32| Key {
        time,
        value,
        forward: None,
        backward: None,
        tbc: None,
    };
    let group = |v: f32| {
        KeyData::Float(KeyGroup {
            key_type: Some(1),
            keys: vec![key(0.0, 0.0), key(1.0, v)],
        })
    };
    // a block past the end of the file (the appended sequence), and one inside it that is no sequence
    let blocks = nif::header::Header::parse(&bytes).unwrap().offsets().len() as u32;
    for clip in [blocks + 3, 1] {
        let mut d = EditDoc::new(probe.doc.source.clone());
        d.ops.push(DocOp {
            op: Op::SetKeys {
                at: at.clone(),
                clip: Some(clip),
                clip_name: Some("SpecialIdle".into()),
                keys: group(2.0),
                base: group(1.0),
            },
            status: OpStatus::Applied,
        });
        let text = d
            .to_text()
            .replacen("[[ops]]", &format!("{OP_ADD_SEQUENCE}[[ops]]"), 1);
        let dir = tempfile::tempdir().unwrap();
        let p = draft_path(dir.path(), "meshes/x.nif").unwrap();
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
        let mut e = open(Some(dir.path()));
        assert_eq!(e.open_lines.len(), 1, "{:?}", e.open_lines);
        assert_eq!(e.doc.ops.len(), 1);
        assert!(
            matches!(
                e.outcomes[0],
                nif::edit::Outcome::Conflict(_) | nif::edit::Outcome::Refused(_)
            ),
            "clip {clip}: {:?}",
            e.outcomes
        );
        assert_eq!(*e.patched, *e.source, "clip {clip}: nothing was applied");
        assert!(e.export_block().is_some(), "the red row blocks the export");
        e.revert(0);
        assert!(
            e.outcomes.is_empty() && !e.doc.has_conflicts(),
            "Revert clears it"
        );
    }
}

/// Final review I3 (Review E6: it is a project's draft now, not the export folder's document): opening a project
/// reaches Messages with the drop line when the project's draft opens in place of an untouched session. A touched
/// session keeps its own document (nothing is loaded into it), and the project's draft, clip ops and all, is kept
/// aside whole at the next save.
#[test]
fn opening_a_project_says_what_it_dropped_and_keeps_the_other_draft_whole() {
    let draft_in = |proj: &Path| {
        let p = draft(proj);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, clip_only_text()).unwrap();
        p
    };
    // untouched: the same NIF reopens from the project's draft
    let (d, mut m) = export_session();
    m.open_edit();
    let proj = project_folder(&d, "proj");
    draft_in(&proj);
    m.messages.clear();
    m.set_project_root(Some(&proj));
    assert_eq!(m.messages, vec![CLIP_ONLY_LINE.to_string()]);
    assert_eq!(m.edit.as_ref().unwrap().doc.ops.len(), 0);
    // touched: its edit moves; the destination's draft is not loaded, so nothing is dropped
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    let proj = project_folder(&d, "proj");
    let p = draft_in(&proj);
    make_old(&p); // ruling E6-D: the live edit is the newer, so it is the one carried
    m.messages.clear();
    m.set_project_root(Some(&proj));
    assert!(
        m.messages.iter().all(|l| !l.contains("dropped")),
        "{:?}",
        m.messages
    );
    gloss(&mut m, 13.5);
    let aside: Vec<_> = std::fs::read_dir(p.parent().unwrap())
        .unwrap()
        .flatten()
        .map(|f| f.path())
        .filter(|f| *f != p)
        .collect();
    assert_eq!(aside.len(), 1, "{aside:?}");
    let kept = std::fs::read_to_string(&aside[0]).unwrap();
    assert!(kept.contains("AddSequence") && kept.contains("RenameSequence"));
}

/// Spec decision 4: a document of only clip ops, exported before the drop, opens pending; Review lists it, nothing
/// says "No edits to export.", and its export writes the unedited NIF over the old export.
#[test]
fn the_next_export_of_a_clip_only_document_writes_the_nif_without_them() {
    let (d, mut m) = export_session();
    let out = d.path().join("mods").join(EDIT_MOD);
    let old = b"exported with clip edits";
    let target = exported(&d);
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::fs::write(&target, old).unwrap();
    let text = [
        DOC_HEAD,
        &format!(
            "[export]\nsha256 = \"{}\"\nops_hash = \"0123\"\n\n",
            sha256_hex(old)
        ),
        OP_ADD_SEQUENCE,
        OP_RENAME_SEQUENCE,
    ]
    .concat();
    let p = nif_doc(&out);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, text).unwrap();
    m.open_edit();
    assert_eq!(m.error, None);
    let e = m.edit.as_ref().unwrap();
    assert_eq!(e.doc.ops.len(), 0);
    assert!(e.pending());
    assert_eq!(
        e.blocked, None,
        "the footer is Ready, not No edits to export"
    );
    let rows = m.nif_review_rows();
    assert_eq!(rows.len(), 1, "Review lists it: {rows:?}");
    assert_eq!(rows[0].blocked, None, "and Export is not off for it");
    assert_eq!(export(&mut m), Ok(1));
    let e = m.edit.as_ref().unwrap();
    assert_eq!(
        std::fs::read(&target).unwrap(),
        *e.source,
        "the unedited NIF"
    );
    assert!(!e.doc.pending());
}

#[test]
fn bad_documents_are_refused() {
    let dir = std::env::temp_dir().join(format!("sdc-nifedit-bad-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("bad.toml");
    std::fs::write(&p, "not = [valid").unwrap();
    assert!(load(&p).is_err());
    std::fs::remove_dir_all(&dir).ok();

    let t = doc().to_text();
    let no_ops: String = t
        .lines()
        .filter(|l| !l.starts_with("ops"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(EditDoc::from_text(&no_ops).is_err());
    let v2 = t.replace("version = 1", "version = 2");
    assert!(EditDoc::from_text(&v2).unwrap_err().contains("newer sdc"));

    let mut a = doc();
    a.ops.push(DocOp {
        op: sample_op(),
        status: OpStatus::Applied,
    });
    let mut b = a.clone();
    if let Op::SetField { value, .. } = &mut b.ops[0].op {
        *value = Value::U32(2);
    }
    assert_ne!(a.ops_hash(), b.ops_hash());
}

// ---- v2 A16: the edit session, source resolution and the Parsed preview ----

mod support;
use modscan::store::PathCopy;
use sdc_gui::model::meshes::nif_edit::*;
use sdc_gui::model::meshes::MeshesModel;
use sdc_gui::model::viewport3d::{Content, V3Cmd};
use sdc_gui::settings::MeshesSettings;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

fn copy(origin: &str, prio: i32, derived: bool) -> PathCopy {
    PathCopy {
        copy_id: prio as i64,
        origin: origin.into(),
        kind: modscan::store::KIND_MOD,
        root: format!(r"C:\I\mods\{origin}"),
        priority: prio,
        derived,
    }
}

#[test]
fn pick_source_skips_the_edit_mod_and_pgpatcher_output() {
    let m = Path::new(r"C:\I\mods\sdc NIF Edits");
    let c = [
        copy("PG output", 9, true),
        copy(EDIT_MOD, 8, false),
        copy("A", 3, false),
    ];
    assert_eq!(
        pick_source(&c, m, NIF).map(|c| c.origin.as_str()),
        Some("A")
    );
    assert!(pick_source(&c[..2], m, NIF).is_none());
}

const NIF: &str = r"meshes\a.nif";

/// Writes `<root>\.sdc\nif-edits\<path>.sdc-nifedit.toml`, as every Tessalith export folder does.
fn write_doc(root: &Path, data_path: &str) {
    let p = sdc_gui::model::meshes::edit_doc::doc_path(root, data_path).unwrap();
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, "").unwrap();
}

fn copy_at(root: &Path, origin: &str, prio: i32) -> PathCopy {
    PathCopy {
        root: root.to_string_lossy().into_owned(),
        ..copy(origin, prio, false)
    }
}

/// Note GQ3 (extends GD28): any folder holding our edit document for this NIF is Tessalith's own export, never the source.
#[test]
fn pick_source_skips_a_folder_holding_our_edit_document_for_this_nif() {
    let d = tempfile::tempdir().unwrap();
    let (old, edit, real) = (
        d.path().join("old"),
        d.path().join("edit"),
        d.path().join("A"),
    );
    write_doc(&old, NIF);
    let c = [
        copy_at(&old, "old export", 9),
        copy_at(&edit, "edit", 8),
        copy_at(&real, "A", 3),
    ];
    assert_eq!(
        pick_source(&c, &edit, NIF).map(|c| c.origin.as_str()),
        Some("A")
    );
    assert!(pick_source(&c[..1], &edit, NIF).is_none());
}

#[test]
fn a_folder_with_our_edit_document_for_another_nif_is_still_a_source() {
    let d = tempfile::tempdir().unwrap();
    let (old, edit) = (d.path().join("old"), d.path().join("edit"));
    write_doc(&old, r"meshes\other.nif");
    let c = [copy_at(&old, "old", 9)];
    assert_eq!(
        pick_source(&c, &edit, NIF).map(|c| c.origin.as_str()),
        Some("old")
    );
}

/// Note GD28: the 1.x default edit mod (`mods\sdc NIF Edits`, CI3) holds Tessalith's own earlier output: it is never
/// the edit source, with no folder chosen or another one.
#[test]
fn the_legacy_edit_mod_is_never_the_source() {
    let c = [copy("SDC NIF Edits", 8, false), copy("A", 3, false)];
    let src = |m: &str| pick_source(&c, Path::new(m), NIF).map(|c| c.origin.clone());
    assert_eq!(src(""), Some("A".into()), "no folder chosen");
    assert_eq!(src(r"C:\I\mods\MyOut"), Some("A".into()), "another folder");
    let mut slash = c[0].clone();
    slash.root = r"C:\I/mods\sdc NIF Edits\".into();
    assert!(pick_source(&[slash], Path::new(""), NIF).is_none());
}

/// Final review M2: Windows folder names ignore case, so `SDC NIF Edits` is still the edit mod.
#[test]
fn the_edit_mod_is_matched_ignoring_case() {
    let m = Path::new(r"C:\I\mods\sdc NIF Edits");
    let other = "SDC NIF Edits";
    let c = [copy(other, 8, false), copy("A", 3, false)];
    assert_eq!(
        pick_source(&c, m, NIF).map(|c| c.origin.as_str()),
        Some("A")
    );
    let mods = [
        mod_row("PGP", 9, true),
        mod_row(other, 5, false),
        mod_row("A", 3, false),
    ];
    let copies = [copy("PGP", 9, true), c[0].clone(), c[1].clone()];
    assert_eq!(placement(&copies, &mods, EDIT_MOD), Placement::Ok);
}

/// Trace 2: MO2 roots use `/` after the instance folder, the export folder picker uses `\`.
#[test]
fn the_edit_mod_is_matched_across_separator_spelling() {
    let m = Path::new(r"X:\Instance\mods\sdc NIF Edits");
    let mut edit = copy("sdc NIF Edits", 8, false);
    edit.root = r"X:\Instance/mods\sdc NIF Edits".into();
    let mut a = copy("A", 3, false);
    a.root = r"X:\Instance/mods\A".into();
    let c = [edit.clone(), a];
    assert_eq!(
        pick_source(&c, m, NIF).map(|c| c.origin.as_str()),
        Some("A")
    );
    edit.root = r"X:\Instance/mods\sdc NIF Edits\".into();
    assert!(pick_source(&[edit], m, NIF).is_none());
}

#[test]
fn generated_winner_warns() {
    assert_eq!(
        generated_warning("DynDOLOD Output").as_deref(),
        Some("DynDOLOD Output is generated by a tool: its next run will overwrite this edit.")
    );
    assert!(generated_warning("A").is_none());
}

#[test]
fn slot_labels_follow_shader_type_and_flags() {
    assert_eq!(edit_slot_label(5, 0, 0, 2), "Skin");
    assert_eq!(edit_slot_label(6, 0, 0, 2), "Hair");
    assert_eq!(edit_slot_label(0, 0, 1 << 6, 2), "Glow");
    assert_eq!(edit_slot_label(11, 0, 0, 6), "Inner layer");
    assert_eq!(edit_slot_label(1, 0, 0, 4), "Env cube");
    assert_eq!(edit_slot_label(0, 0, 1 << 23, 5), "RMAOS");
}

/// Trace 2026-10-28 #1 (fix round 1): 2000 112-byte BSTriShapes (a head, no geometry) that all name one 100 000-byte
/// string (0x80, which decodes to three bytes). The shape cards keep the one shared name, not a copy per card.
#[test]
fn shape_cards_share_one_name_across_many_shapes() {
    // a BSTriShape head naming string 30 (the large one), shader and alpha refs -1: 112 bytes
    const HEAD: [u8; 112] = {
        let mut h = [0u8; 112];
        h[0] = 30; // name
        h[8] = 0xFF; // controller -1
        h[9] = 0xFF;
        h[10] = 0xFF;
        h[11] = 0xFF;
        h[12] = 14; // flags
        h[68] = 0xFF; // collision -1 at 16 + 52 (translation 12, rotation 36 and scale 4 stay zero)
        h[69] = 0xFF;
        h[70] = 0xFF;
        h[71] = 0xFF;
        // bounding sphere 16 at 72..88; skin, shader, alpha refs -1 at 88..100
        let mut i = 88;
        while i < 100 {
            h[i] = 0xFF;
            i += 1;
        }
        h
    };
    let mut b = b"Gamebryo File Format, Version 20.2.0.7\n".to_vec();
    b.extend(0x1402_0007u32.to_le_bytes());
    b.push(1);
    b.extend(12u32.to_le_bytes());
    b.extend(2000u32.to_le_bytes());
    b.extend(100u32.to_le_bytes());
    b.extend([1u8, 0, 1, 0, 1, 0]); // author, process and export script
    b.extend(1u16.to_le_bytes());
    b.extend(10u32.to_le_bytes());
    b.extend(b"BSTriShape");
    b.extend([0u8; 2 * 2000]); // type indices
    for _ in 0..2000 {
        b.extend((HEAD.len() as u32).to_le_bytes()); // sizes
    }
    b.extend(31u32.to_le_bytes());
    b.extend(100_000u32.to_le_bytes());
    for k in 0..30 {
        b.extend(1u32.to_le_bytes());
        b.push(b'a' + k as u8);
    }
    b.extend(100_000u32.to_le_bytes());
    b.extend(vec![0x80u8; 100_000]);
    b.extend(0u32.to_le_bytes()); // groups
    for _ in 0..2000 {
        b.extend(&HEAD);
    }
    b.extend(30u32.to_le_bytes()); // footer: 30 roots
    b.extend([0u8; 4 * 30]);
    let cards = modscan::store::shape_views(&nif::scan::scan(&b).unwrap());
    assert_eq!(cards.len(), 2000);
    let ptrs: std::collections::BTreeSet<_> = cards
        .iter()
        .map(|c| c.name.as_deref().map(str::as_ptr))
        .collect();
    assert_eq!(ptrs.len(), 1, "{} distinct name copies", ptrs.len());
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/../nif/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn put(root: &Path, rel: &str, bytes: Option<&[u8]>) {
    let fixture = fixture("TestNifFile_Skinned_SE.nif");
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, bytes.unwrap_or(&fixture)).unwrap();
}

/// An MO2 instance with profile-local INIs (no real Documents folder is read), as `modscan`'s tests build it.
fn instance(modlist: &str) -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let r = d.path();
    let game = r.join("Game");
    std::fs::create_dir_all(game.join("Data")).unwrap();
    std::fs::write(
        r.join("ModOrganizer.ini"),
        format!(
            "[General]\ngamePath=@ByteArray({})\nselected_profile=@ByteArray(Default)\n",
            game.display().to_string().replace('\\', "\\\\")
        ),
    )
    .unwrap();
    for m in ["A", "PGP"] {
        std::fs::create_dir_all(r.join("mods").join(m)).unwrap();
    }
    std::fs::create_dir_all(r.join("overwrite")).unwrap();
    std::fs::create_dir_all(r.join("profiles/Default")).unwrap();
    std::fs::write(r.join("profiles/Default/modlist.txt"), modlist).unwrap();
    std::fs::write(
        r.join("profiles/Default/settings.ini"),
        "[General]\nLocalSettings=true\n",
    )
    .unwrap();
    d
}

/// Mods A (x.nif, a loose texture no shape uses) and PGP (PGPatcher's patch of x.nif); an LE x.nif in A alone.
fn files(le: bool) -> tempfile::TempDir {
    files_with(le, "+PGP\n+A\n")
}

fn files_with(le: bool, modlist: &str) -> tempfile::TempDir {
    let d = instance(modlist);
    let le_bytes = fixture("TestNifFile_Animated_LE.nif");
    put(d.path(), "mods/A/meshes/x.nif", le.then_some(&le_bytes[..]));
    put(d.path(), "mods/A/textures/new.dds", Some(b"dds"));
    if !le {
        let a = std::fs::read(d.path().join("mods/A/meshes/x.nif")).unwrap();
        put(d.path(), "mods/PGP/meshes/x.nif", None);
        put(d.path(), "mods/PGP/PGPatcher.esp", Some(b"e"));
        let diff = format!(
            r#"{{"meshes\\x.nif":{{"crc32original":{},"crc32patched":1}}}}"#,
            crc32fast::hash(&a)
        );
        put(
            d.path(),
            "mods/PGP/ParallaxGen_Diff.json",
            Some(diff.as_bytes()),
        );
    }
    d
}

/// Scans `root` into `m`'s store and selects A's x.nif.
fn scan_and_select(m: &mut MeshesModel, root: &Path) {
    m.activate_with(None);
    m.use_instance(root.to_path_buf(), "Default".into());
    m.request_scan();
    assert!(m.take_request());
    m.start();
    let t = Instant::now();
    m.poll();
    while m.scanning() {
        assert!(t.elapsed() < Duration::from_secs(60), "scan did not end");
        std::thread::sleep(Duration::from_millis(10));
        m.poll();
    }
    assert_eq!(m.error, None);
    let a = m.mods.iter().find(|r| r.name == "A").unwrap().id;
    m.select_mod(a);
    let c = m.nifs[0].copy_id;
    m.select_nif(c);
    // Note GD5: NIF edits have no default mod; the tests choose `<mods>\sdc NIF Edits`, CI3's old default.
    m.set_nif_export_root(Some(root.join("mods").join(EDIT_MOD)));
}

fn session(le: bool) -> (tempfile::TempDir, MeshesModel) {
    let d = files(le);
    let mut m = MeshesModel::new(Some(d.path().join("scan.db")), MeshesSettings::default());
    m.procs = sdc_gui::model::meshes::no_procs;
    scan_and_select(&mut m, d.path());
    (d, m)
}

/// Stands in for the render thread.
fn attach(m: &mut MeshesModel) -> Receiver<V3Cmd> {
    m.v3.attach_test_thread()
}

fn loads(cmds: &Receiver<V3Cmd>) -> Vec<V3Cmd> {
    cmds.try_iter()
        .filter(|c| {
            matches!(
                c,
                V3Cmd::Load { .. } | V3Cmd::LoadParsed { .. } | V3Cmd::LoadBuiltin { .. }
            )
        })
        .collect()
}

/// (shape block, its BSLightingShaderProperty block) of the first lit shape.
fn lit_shape(e: &NifEdit) -> (u32, u32) {
    e.model
        .shapes
        .iter()
        .find_map(|s| {
            s.shader
                .filter(|b| e.model.lighting.contains_key(b))
                .map(|b| (s.block, b))
        })
        .expect("a lit shape")
}

#[test]
fn open_edit_loads_the_source_and_previews_parsed() {
    let (_d, mut m) = session(false);
    m.open_edit();
    assert_eq!(m.error, None);
    let e = m.edit.as_ref().expect("an edit session");
    assert_eq!(e.source_copy.origin, "A");
    assert!(!e.model.shapes.is_empty());
    assert!(!e.le_mesh);
    assert_eq!(e.source_changed, None);
    assert!(matches!(m.v3.content(), Some(Content::Parsed { .. })));
    assert!(m.v3.nif_view);
    m.close_edit();
    assert!(m.edit.is_none());
    assert!(m.v3.is_mesh());
    assert!(!m.v3.nif_view);
}

#[test]
fn glossiness_edit_reaches_the_viewport_without_a_reload() {
    let (_d, mut m) = session(false);
    let cmds = attach(&mut m);
    m.open_edit();
    m.v3.request_frame(64, 64);
    assert_eq!(loads(&cmds).len(), 1, "the first LoadParsed");
    let e = m.edit.as_ref().unwrap();
    let (shape, shader) = lit_shape(e);
    let cur = e.model.lighting[&shader].lighting.params.glossiness;
    assert_ne!(cur, 12.5);
    let at = nif::edit::anchor_role(&e.model, Some(shape), Role::Shader).unwrap();
    m.edit_push(
        vec![Op::SetField {
            at,
            field: Field::Glossiness,
            value: Value::F32(12.5f32.to_bits()),
            base: Value::F32(cur.to_bits()),
        }],
        None,
    );
    assert_eq!(m.error, None);
    let got: Vec<V3Cmd> = cmds.try_iter().collect();
    assert!(got.iter().any(|c| matches!(
        c,
        V3Cmd::SetMaterial { material, .. } if material.params.glossiness == 12.5
    )));
    assert!(!got.iter().any(|c| matches!(c, V3Cmd::LoadParsed { .. })));
    m.v3.request_frame(64, 64);
    assert!(loads(&cmds).is_empty(), "no reload");
    let e = m.edit.as_ref().unwrap();
    assert_eq!(e.doc.ops.len(), 1);
    assert_eq!(e.doc.ops[0].status, OpStatus::Applied);
    let saved = load(&e.draft_path).unwrap().expect("saved after the push");
    assert_eq!(saved.ops.len(), 1);
    assert!(m.undo_edit());
    assert!(m.edit.as_ref().unwrap().doc.ops.is_empty());
    assert!(m.redo_edit());
    assert_eq!(m.edit.as_ref().unwrap().doc.ops.len(), 1);
}

#[test]
fn new_texture_path_reloads_the_same_view() {
    let (_d, mut m) = session(false);
    let cmds = attach(&mut m);
    m.open_edit();
    m.v3.request_frame(64, 64);
    let first = loads(&cmds);
    let Some(V3Cmd::LoadParsed { textures, .. }) = first.first() else {
        panic!("LoadParsed first")
    };
    assert!(!textures.contains_key("textures/new.dds"));
    let e = m.edit.as_ref().unwrap();
    let (shape, shader) = lit_shape(e);
    let set = e.model.lighting[&shader].texture_set.unwrap();
    let base = String::from_utf8_lossy(&e.model.texsets[&set].raw.slots[0]).into_owned();
    m.edit_push(
        vec![Op::SetTexturePath {
            at: nif::edit::anchor_shape(&e.model, shape),
            slot: 0,
            path: r"textures\new.dds".into(),
            base,
        }],
        None,
    );
    assert_eq!(m.error, None);
    assert_eq!(
        m.edit.as_ref().unwrap().doc.ops[0].status,
        OpStatus::Applied
    );
    m.v3.request_frame(64, 64);
    let again = loads(&cmds);
    let Some(V3Cmd::LoadParsed { textures, .. }) = again.first() else {
        panic!("a new LoadParsed")
    };
    assert!(textures.contains_key("textures/new.dds"));
}

/// Note GP9: a drag tick on a float field names no texture, so it queries no store.
#[test]
fn a_float_edit_makes_no_store_query() {
    let (_d, mut m) = session(false);
    let cmds = attach(&mut m);
    m.open_edit();
    m.v3.request_frame(64, 64);
    assert_eq!(loads(&cmds).len(), 1, "the first LoadParsed");
    let n = sdc_gui::model::meshes::texture_source_calls();
    gloss(&mut m, 12.5);
    assert_eq!(m.error, None);
    assert_eq!(sdc_gui::model::meshes::texture_source_calls() - n, 0);
    let got: Vec<V3Cmd> = cmds.try_iter().collect();
    assert!(got.iter().any(|c| matches!(
        c,
        V3Cmd::SetMaterial { material, .. } if material.params.glossiness == 12.5
    )));
    assert!(!got.iter().any(|c| matches!(c, V3Cmd::LoadParsed { .. })));
}

/// Trace-debug 2026-11-04 #8 (Note FO8): the Auto wetness guess (GN1) is in the texture map a skin shape loads with.
#[test]
fn the_auto_wet_guess_is_in_the_mesh_texture_map() {
    let d = files(false);
    // body_wet derives from slot 7 (`body_s`), head_wet from slot 1 (`head_msn`).
    put(d.path(), "mods/A/textures/a/body_wet.dds", Some(b"dds"));
    put(d.path(), "mods/A/textures/a/head_wet.dds", Some(b"dds"));
    let mut m = MeshesModel::new(Some(d.path().join("scan.db")), MeshesSettings::default());
    m.procs = sdc_gui::model::meshes::no_procs;
    scan_and_select(&mut m, d.path());
    let store = modscan::store::Store::open_read_only(&d.path().join("scan.db")).unwrap();
    let slots = || {
        let mut v = vec![String::new(); 8];
        v[0] = "textures/a/head.dds".into();
        v[1] = "textures/a/head_msn.dds".into();
        v[7] = "textures/a/body_s.dds".into();
        v
    };
    let map = sdc_gui::model::meshes::texture_sources_for_test(&store, &[(4, slots())]).unwrap();
    for wet in ["textures/a/body_wet.dds", "textures/a/head_wet.dds"] {
        assert!(
            matches!(
                map.get(wet),
                Some(sdc_gui::model::viewport3d::FileSource::Optional(_))
            ),
            "{wet}: the guess of the type-4 shape is an optional source"
        );
    }
    // Control: a plain shape (type 0) gets no guess.
    let map = sdc_gui::model::meshes::texture_sources_for_test(&store, &[(0, slots())]).unwrap();
    assert!(!map.contains_key("textures/a/body_wet.dds"));
}

/// Note GP9 (Review Focus 3): a slot path the store has and the loaded map lacks is still looked up and reloaded.
#[test]
fn a_new_slot_path_still_reloads_with_its_texture() {
    let (_d, mut m) = session(false);
    m.open_edit();
    let e = m.edit.as_ref().unwrap();
    let (shape, shader) = lit_shape(e);
    let set = e.model.lighting[&shader].texture_set.unwrap();
    let base = String::from_utf8_lossy(&e.model.texsets[&set].raw.slots[0]).into_owned();
    let op = Op::SetTexturePath {
        at: nif::edit::anchor_shape(&e.model, shape),
        slot: 0,
        path: r"textures\new.dds".into(),
        base,
    };
    let Some(Content::Parsed { textures, .. }) = m.v3.content() else {
        panic!("a Parsed content")
    };
    assert!(!textures.contains_key("textures/new.dds"));
    let n = sdc_gui::model::meshes::texture_source_calls();
    m.edit_push(vec![op], None);
    assert_eq!(m.error, None);
    assert_eq!(sdc_gui::model::meshes::texture_source_calls() - n, 1);
    let Some(Content::Parsed { textures, .. }) = m.v3.content() else {
        panic!("a Parsed content")
    };
    assert!(textures.contains_key("textures/new.dds"));
}

/// Note GP9: a store error in `edit_changed` is retried on the next change, though no slot path changed since.
#[test]
fn a_failed_texture_lookup_is_retried_on_the_next_change() {
    let (d, mut m) = session(false);
    m.open_edit();
    let e = m.edit.as_ref().unwrap();
    let (shape, shader) = lit_shape(e);
    let set = e.model.lighting[&shader].texture_set.unwrap();
    let base = String::from_utf8_lossy(&e.model.texsets[&set].raw.slots[0]).into_owned();
    let op = Op::SetTexturePath {
        at: nif::edit::anchor_shape(&e.model, shape),
        slot: 0,
        path: r"textures\new.dds".into(),
        base,
    };
    let db = rusqlite::Connection::open(d.path().join("scan.db")).unwrap();
    db.execute_batch("ALTER TABLE tex_winner RENAME TO tex_winner_off")
        .unwrap();
    m.edit_push(vec![op], None);
    assert!(m.error.is_some(), "the lookup failed");
    m.error = None;
    db.execute_batch("ALTER TABLE tex_winner_off RENAME TO tex_winner")
        .unwrap();
    let n = sdc_gui::model::meshes::texture_source_calls();
    gloss(&mut m, 12.5);
    assert_eq!(m.error, None);
    assert_eq!(sdc_gui::model::meshes::texture_source_calls() - n, 1);
    let Some(Content::Parsed { textures, .. }) = m.v3.content() else {
        panic!("a Parsed content")
    };
    assert!(textures.contains_key("textures/new.dds"));
}

#[test]
fn le_file_opens_without_preview() {
    let (_d, mut m) = session(true);
    let cmds = attach(&mut m);
    m.open_edit();
    m.v3.request_frame(64, 64);
    let e = m.edit.as_ref().expect("an LE NIF opens read-only");
    assert!(e.le_mesh);
    assert!(!e.model.shapes.is_empty());
    assert!(loads(&cmds).is_empty());
    assert!(!m.v3.nif_view);
}

/// Correction 2026-10-20: `SdcApp::new` replaces `meshes.v3`, so the flag must come from `open_edit`.
#[test]
fn an_edited_nif_keeps_its_failed_shader_message_in_the_app() {
    use eframe::egui;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;
    use render3d::{ShapeInfo, ShapeStatus};
    use sdc_gui::app::SdcApp;
    app_data();
    let d = files(false);
    let model = sdc_gui::model::AppModel::new(
        sdc_gui::settings::GuiSettings::default(),
        None,
        support::session(),
    );
    let mut model = model;
    // Note GD5: the app hands the chosen export folder to the meshes each frame
    model.set_export_folder(d.path().join("mods").join(EDIT_MOD));
    let mut app = SdcApp::new(model);
    app.meshes.procs = sdc_gui::model::meshes::no_procs;
    app.meshes.store_path = Some(d.path().join("scan.db"));
    scan_and_select(&mut app.meshes, d.path());
    app.meshes.open_edit();
    assert_eq!(app.meshes.error, None);
    let (id, material) = app.meshes.edit.as_ref().unwrap().materials()[0].clone();
    app.meshes.v3.report = Some(render3d::LoadReport::default());
    app.meshes.v3.shapes = vec![ShapeInfo {
        id,
        name: "Edited".into(),
        material,
        status: ShapeStatus::Failed {
            error: "boom".into(),
            showing_previous: false,
        },
    }];
    let mut frame = eframe::Frame::_new_kittest();
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1400.0, 900.0))
        .build_ui_state(
            move |ui, app: &mut SdcApp| eframe::App::ui(app, ui, &mut frame),
            app,
        );
    sdc_gui::ui::header::set_meshes_view(&h.ctx, true);
    h.run_steps(4);
    h.get_by_label("Edited: shader failed: boom");
}

// ---- A16 review: a slider drag is one op ----

/// One drag tick on the first lit shape's glossiness, based on the value the panel shows now.
fn tick(m: &mut MeshesModel, v: f32) {
    let e = m.edit.as_ref().unwrap();
    let (shape, shader) = lit_shape(e);
    let cur = e.model.lighting[&shader].lighting.params.glossiness;
    let op = Op::SetField {
        at: nif::edit::anchor_role(&e.model, Some(shape), Role::Shader).unwrap(),
        field: Field::Glossiness,
        value: Value::F32(v.to_bits()),
        base: Value::F32(cur.to_bits()),
    };
    let key = nif::edit::merge_key(&op);
    m.edit_push(vec![op], Some(key));
}

fn op_values(m: &MeshesModel) -> Vec<(f32, f32)> {
    m.edit
        .as_ref()
        .unwrap()
        .doc
        .ops
        .iter()
        .map(|d| match &d.op {
            Op::SetField {
                value: Value::F32(v),
                base: Value::F32(b),
                ..
            } => (f32::from_bits(*b), f32::from_bits(*v)),
            o => panic!("{o:?}"),
        })
        .collect()
}

#[test]
fn a_slider_drag_is_one_op_with_the_original_base() {
    let (_d, mut m) = session(false);
    m.open_edit();
    let e = m.edit.as_ref().unwrap();
    let orig = e.model.lighting[&lit_shape(e).1].lighting.params.glossiness;
    for v in [orig + 1.0, orig + 2.0, 40.5] {
        tick(&mut m, v);
    }
    assert_eq!(op_values(&m), [(orig, 40.5)]);
    // Undo is one step back to before the drag.
    assert!(m.undo_edit());
    assert!(op_values(&m).is_empty());
    assert!(m.redo_edit());
    assert_eq!(op_values(&m), [(orig, 40.5)]);
    // After the drag ends, the next drag on the same field is a new op based on the committed value.
    m.edit.as_mut().unwrap().flush();
    for v in [50.5, 60.5] {
        tick(&mut m, v);
    }
    assert_eq!(op_values(&m), [(orig, 40.5), (40.5, 60.5)]);
    let e = m.edit.as_mut().unwrap();
    assert!(e.doc.ops.iter().all(|d| d.status == OpStatus::Applied));
    e.revert(1);
    assert_eq!(e.doc.ops[0].status, OpStatus::Applied);
    e.revert(0);
    assert_eq!(
        e.patched, e.source,
        "reverting the drag restores the source bytes"
    );
}

#[test]
fn a_failed_reopen_returns_the_view_to_the_mesh() {
    let (_d, mut m) = session(false);
    m.open_edit();
    assert!(matches!(m.v3.content(), Some(Content::Parsed { .. })));
    let p = m.edit.as_ref().unwrap().draft_path.clone();
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, "not = [a document").unwrap();
    m.open_edit();
    assert!(m.edit.is_none());
    assert!(m.error.is_some());
    assert!(m.v3.is_mesh());
}

/// One non-merged SetField on the first lit shape's shader.
fn set_lit(m: &mut MeshesModel, field: Field, v: f32) {
    let e = m.edit.as_ref().unwrap();
    let (shape, shader) = lit_shape(e);
    let p = &e.model.lighting[&shader].lighting.params;
    let cur = match field {
        Field::Glossiness => p.glossiness,
        _ => p.specular_strength,
    };
    let at = nif::edit::anchor_role(&e.model, Some(shape), Role::Shader).unwrap();
    m.edit_push(
        vec![Op::SetField {
            at,
            field,
            value: Value::F32(v.to_bits()),
            base: Value::F32(cur.to_bits()),
        }],
        None,
    );
}

/// v2 B8 (supersedes P13's In game half): In game (simulated) keeps the session and shows every edit.
#[test]
fn in_game_and_back_keeps_the_session_and_shows_every_edit() {
    let (_d, mut m) = session(false);
    assert!(m.pair.is_some(), "x.nif has a PGPatcher copy");
    m.open_edit();
    assert!(m.view_source, "the edit preview is the Source side");
    set_lit(&mut m, Field::Glossiness, 12.5);
    m.set_view_source(false);
    assert!(!m.v3.is_mesh(), "In game is the simulated edit preview");
    assert!(m.v3.nif_view);
    assert!(m.edit.as_ref().unwrap().in_game.is_some());
    set_lit(&mut m, Field::SpecularStrength, 3.5);
    assert_eq!(m.edit.as_ref().unwrap().doc.ops.len(), 2);
    let Some(Content::Parsed { nif, .. }) = m.v3.content() else {
        panic!("In game shows the edit preview")
    };
    let p = &nif.shapes[0].material.params;
    assert_eq!(
        (p.glossiness, p.specular_strength),
        (12.5, 3.5),
        "no JSON: in game too"
    );
    let (shape, _) = lit_shape(m.edit.as_ref().unwrap());
    m.select_edit(Selection::Shape(shape));
    wait_pg(&mut m);
    let mut h = view(m);
    h.run_steps(4);
    h.get_by_label("In game");
    h.get_by_label(NO_MATCH);
    let m = h.state_mut();
    m.set_view_source(true);
    assert!(m.v3.nif_view);
    let Some(Content::Parsed { nif, .. }) = m.v3.content() else {
        panic!("Source shows the edit preview again")
    };
    let p = &nif.shapes[0].material.params;
    assert_eq!((p.glossiness, p.specular_strength), (12.5, 3.5));
    h.run_steps(4);
    assert!(!h.state().edit_preview_hidden());
    assert!(h.query_by_label(EDIT_HIDDEN).is_none());
}

// ---- v2 A17: the edit panel, selection, Shader and Alpha tabs ----

#[path = "../../nif/tests/builder/mod.rs"]
mod builder;
use builder::scenes;
use eframe::egui;
use egui::accesskit::Role as AkRole;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use nif::edit::validate::Target;
use nif::extra::ExtraValue;

/// The Meshes view alone, as `meshes_view.rs` draws it.
fn view(m: MeshesModel) -> Harness<'static, MeshesModel> {
    let h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1400.0, 900.0))
        .build_ui_state(
            |ui, m: &mut MeshesModel| sdc_gui::ui::meshes::show(ui, m),
            m,
        );
    h
}

/// A read-only edit session on `bytes` without a scan (no store, no preview).
fn direct(bytes: Vec<u8>) -> MeshesModel {
    let mut m = MeshesModel::empty();
    m.edit =
        Some(NifEdit::open(bytes, copy("A", 1, false), "meshes/x.nif", None, None, None).unwrap());
    m
}

fn shape_named(e: &NifEdit, name: &str) -> u32 {
    e.model
        .shapes
        .iter()
        .find(|s| s.name.as_deref() == Some(name))
        .unwrap()
        .block
}

fn select(m: &mut MeshesModel, name: &str) -> u32 {
    let b = shape_named(m.edit.as_ref().unwrap(), name);
    m.select_edit(Selection::Shape(b));
    b
}

fn ops(m: &MeshesModel) -> Vec<Op> {
    m.edit.as_ref().unwrap().doc.ops().to_vec()
}

#[test]
fn edit_toggle_opens_the_panel_with_four_tabs() {
    let (_d, m) = session(false);
    let mut h = view(m);
    h.run_steps(4);
    assert!(h.query_by_label("Controllers tab").is_none());
    h.get_by_label("Edit").click();
    h.run_steps(4);
    assert!(h.state().edit.is_some(), "Edit opens a session");
    assert!(
        h.state().shapes_col,
        "Edit shows the shape cards to pick from"
    );
    for l in ["Shader", "Alpha", "Controllers", "Extra data"] {
        h.get_by_label(&format!("{l} tab"));
    }
    h.get_by_label("Source: A");
    h.get_by_label("Edit").click();
    h.run_steps(4);
    assert!(h.state().edit.is_none(), "Edit again closes it");
    assert!(h.query_by_label("Controllers tab").is_none());
}

#[test]
fn the_edit_button_has_a_frame_when_closed() {
    let (_d, m) = session(false);
    let mut h = view(m);
    h.run_steps(4);
    assert_eq!(
        h.get_by_label("Edit").accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::False)
    );
    let r = h.get_by_label("Edit").rect();
    // a fill the size of the button itself, not the header or panel background behind it
    let framed = h.output().shapes.iter().any(|c| {
        matches!(&c.shape,
        egui::epaint::Shape::Rect(s) if s.rect.expand(2.0).contains_rect(r)
            && r.expand(2.0).contains_rect(s.rect) && s.fill.a() > 0)
    });
    assert!(
        framed,
        "Edit must be drawn as a button while the panel is closed"
    );
    h.get_by_label("Edit").click();
    h.run_steps(4);
    assert_eq!(
        h.get_by_label("Edit").accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::True)
    );
}

#[test]
fn clicking_a_card_selects_the_shape_and_root_card_is_first() {
    let (_d, mut m) = session(false);
    m.open_edit();
    m.shapes_col = true;
    let s = m.shapes[0].clone();
    let card = format!(
        "Select shape {} {}",
        s.index3d
            .map_or("#– (not in scene tree)".into(), |i| format!("#{i}")),
        s.name.as_deref().unwrap_or("(unnamed)"),
    );
    let mut h = view(m);
    h.run_steps(4);
    let root = h.get_by_label("Root").rect();
    let first = h.get_by_label(&card).rect();
    assert!(root.max.y <= first.min.y, "Root is the first card");
    h.get_by_label(&card).click();
    h.run_steps(4);
    assert_eq!(
        h.state().edit.as_ref().unwrap().selection,
        Selection::Shape(s.block)
    );
    h.get_by_label("Root").click();
    h.run_steps(4);
    assert_eq!(h.state().edit.as_ref().unwrap().selection, Selection::Root);
}

/// Meshes UI T2: a card picked outside edit mode is where Edit opens; the Shapes button is locked on while editing.
#[test]
fn edit_opens_on_the_card_picked_before_and_locks_the_shapes_column() {
    let (_d, mut m) = session(false);
    m.shapes_col = true;
    let s = m.shapes.last().unwrap().clone();
    let label = format!(
        "Select shape {} {}",
        s.index3d
            .map_or("#– (not in scene tree)".into(), |i| format!("#{i}")),
        s.name.as_deref().unwrap_or("(unnamed)"),
    );
    let mut h = view(m);
    h.run_steps(4);
    h.get_by_role_and_label(AkRole::Button, &label).click();
    h.run_steps(4);
    assert!(
        h.state().edit.is_none(),
        "a pick outside edit mode opens nothing"
    );
    h.get_by_label("Edit").click();
    h.run_steps(4);
    assert_eq!(
        h.state().edit.as_ref().unwrap().selection,
        Selection::Shape(s.block)
    );
    // Note GH4: the toggle names the file while a NIF is selected.
    let shapes = h.get_by_role_and_label(AkRole::Button, "Shapes of x.nif");
    assert!(
        shapes.accesskit_node().is_disabled(),
        "locked while editing"
    );
    shapes.click();
    h.run_steps(4);
    assert!(h.state().shapes_col);
}

/// T2 review Minor 2: a card picked inside Edit is the card outlined after Edit closes and where the next Edit opens.
#[test]
fn a_pick_inside_edit_carries_to_the_next_edit() {
    let (_d, mut m) = session(false);
    m.open_edit();
    let e = m.edit.as_ref().unwrap();
    let last = e.model.shapes.last().unwrap();
    let (block, name) = (last.block, last.name.as_deref().map(str::to_string));
    let mut h = view(m);
    h.run_steps(4);
    let tail = format!(" {}", name.as_deref().unwrap());
    h.query_all_by_role(AkRole::Button)
        .find(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Select shape") && l.ends_with(&tail))
        })
        .expect("the last shape's header")
        .click();
    h.run_steps(4);
    assert_eq!(
        h.state().edit.as_ref().unwrap().selection,
        Selection::Shape(block)
    );
    assert_eq!(h.state().card_sel, Some((block, name.clone())));
    let m = h.state_mut();
    m.close_edit();
    m.open_edit();
    assert_eq!(m.edit.as_ref().unwrap().selection, Selection::Shape(block));
}

/// T2 review Minor 1: Root stays selected across an export-folder switch (the reopen would pick a shape).
#[test]
fn root_stays_selected_across_an_export_folder_switch() {
    let (d, mut m) = export_session();
    m.card_sel = Some((
        m.shapes[0].block,
        m.shapes[0].name.as_deref().map(str::to_string),
    ));
    m.open_edit();
    assert!(matches!(
        m.edit.as_ref().unwrap().selection,
        Selection::Shape(_)
    ));
    m.select_edit(Selection::Root);
    m.set_nif_export_root(Some(d.path().join(r"mods\MyOut")));
    assert!(m.edit.is_some());
    assert_eq!(m.edit.as_ref().unwrap().selection, Selection::Root);
}

/// T2 review Minor 6: a clip-driven controller reads "(clip)", a shape without controllers "none", and a shape whose
/// shader is not a lighting shader keeps its alpha controllers (Minor 3).
#[test]
fn controllers_lines_mark_clips_say_none_and_keep_alpha_on_other_shaders() {
    let mut h = view(direct(scenes::managed_clip()));
    h.run_steps(2);
    h.get_by_label_contains("(clip)");
    let mut h = view(direct(two_named(false)));
    h.run_steps(2);
    assert_eq!(h.query_all_by_label("none").count(), 2);
    let mut n = Nif::new(100);
    let s = n.string("S");
    n.block("BSTriShape", scenes::bstri_named(s, 1, 2));
    n.block(
        "BSWaterShaderProperty",
        W::default().raw(&nif::net::encode_net(-1, &[], -1)).0,
    );
    n.block(
        "NiAlphaProperty",
        W::default()
            .raw(&nif::net::encode_net(-1, &[], 3))
            .u16(0x00ED)
            .u8(128)
            .0,
    );
    n.block(
        "BSNiAlphaPropertyTestRefController",
        scenes::ctrl_body(-1, 0x48, 2, W::default().i32(4)),
    );
    n.block("NiFloatInterpolator", W::default().f(1.0).i32(5).0);
    n.block(
        "NiFloatData",
        W::default().u32(2).u32(1).fs(&[0.0, 0.0, 1.0, 1.0]).0,
    );
    let mut h = view(direct(n.bytes()));
    h.run_steps(2);
    h.get_by_label("1: Alpha test threshold");
}

/// Meshes UI T2: a NIF with one shape opens Edit on that shape, not on Root.
#[test]
fn a_one_shape_nif_opens_edit_on_its_shape() {
    let mut n = builder::Nif::new(100);
    let a = n.string("Only");
    n.block("BSTriShape", one_tri(a, 1));
    n.block("BSLightingShaderProperty", scenes::lsp(0, -1));
    let bytes = n.bytes();
    let d = files_pair(&bytes, &bytes);
    let mut m = MeshesModel::new(Some(d.path().join("scan.db")), MeshesSettings::default());
    m.procs = sdc_gui::model::meshes::no_procs;
    scan_and_select(&mut m, d.path());
    m.open_edit();
    assert_eq!(m.error, None);
    let e = m.edit.as_ref().unwrap();
    assert_eq!(e.selection, Selection::Shape(shape_named(e, "Only")));
}

/// Meshes UI T2: in edit mode a card lists its controllers from the edited model.
#[test]
fn edit_cards_list_their_controllers() {
    let m = direct(ctrl_scene(0x82));
    let mut h = view(m);
    h.run_steps(2);
    h.get_by_label("2: Emissive multiple, U offset");
}

/// A BSTriShape with one triangle (positions only), so the preview draws it.
fn one_tri(name: i32, shader: i32) -> Vec<u8> {
    scenes::avobj(scenes::net(name))
        .fs(&[0.0; 4])
        .i32(-1)
        .i32(shader)
        .i32(-1)
        .u64((1 << 44) | 4) // Vertex, 16-byte records
        .u16(1)
        .u16(3)
        .u32(3 * 16 + 6)
        .fs(&[0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0])
        .u16(0)
        .u16(1)
        .u16(2)
        .u32(0) // particle data
        .0
}

/// BSTriShapes "A" and "B" (0, 1), or "B" and "A" when `swap`, each with its own BSLSP (2, 3).
fn two_named(swap: bool) -> Vec<u8> {
    let mut n = builder::Nif::new(100);
    let (a, b) = (n.string("A"), n.string("B"));
    let (x, y) = if swap { (b, a) } else { (a, b) };
    n.block("BSTriShape", one_tri(x, 2));
    n.block("BSTriShape", one_tri(y, 3));
    n.block("BSLightingShaderProperty", scenes::lsp(0, -1));
    n.block("BSLightingShaderProperty", scenes::lsp(0, -1));
    n.bytes()
}

/// x.nif in mod A and a PGPatcher copy in PGP with different bytes (its blocks numbered differently).
fn files_pair(a: &[u8], pgp: &[u8]) -> tempfile::TempDir {
    let d = instance("+PGP\n+A\n");
    put(d.path(), "mods/A/meshes/x.nif", Some(a));
    put(d.path(), "mods/PGP/meshes/x.nif", Some(pgp));
    put(d.path(), "mods/PGP/PGPatcher.esp", Some(b"e"));
    let diff = format!(
        r#"{{"meshes\\x.nif":{{"crc32original":{},"crc32patched":1}}}}"#,
        crc32fast::hash(a)
    );
    put(
        d.path(),
        "mods/PGP/ParallaxGen_Diff.json",
        Some(diff.as_bytes()),
    );
    d
}

/// Final review I1: the cards, their selection and Solo follow the edited copy, not the PGPatcher copy on show.
#[test]
fn edit_cards_follow_the_session_not_the_shown_pgpatcher_copy() {
    let d = files_pair(&two_named(false), &two_named(true));
    let mut m = MeshesModel::new(Some(d.path().join("scan.db")), MeshesSettings::default());
    m.procs = sdc_gui::model::meshes::no_procs;
    scan_and_select(&mut m, d.path());
    assert!(m.pair.is_some(), "x.nif has a PGPatcher copy");
    m.open_edit();
    assert_eq!(m.error, None);
    m.shapes_col = true;
    let a = shape_named(m.edit.as_ref().unwrap(), "A");
    let shown_a = m
        .shapes
        .iter()
        .find(|s| s.name.as_deref() == Some("A"))
        .unwrap()
        .block;
    assert_ne!(a, shown_a, "the copies number A differently");
    let mut h = view(m);
    h.run_steps(4);
    h.get_by_label_contains("tree) A").click();
    h.run_steps(4);
    assert_eq!(
        h.state().edit.as_ref().unwrap().selection,
        Selection::Shape(a)
    );
    // T4 review Minor 5: the pick is kept in the shown copy's numbering, so the outline finds A after Edit closes.
    assert_eq!(h.state().card_sel, Some((shown_a, Some("A".into()))));
    h.get_by_label("Show only this shape").click();
    h.run_steps(4);
    let mats = h.state().edit.as_ref().unwrap().materials();
    assert_eq!(mats.len(), 2);
    for (id, mat) in mats {
        assert_eq!(mat.hidden, id.0 != a, "shape block {}", id.0);
    }
}

/// Meshes UI T1: the edit-mode cards are built from the edited bytes; untouched, they equal the scan's cards.
#[test]
fn edit_cards_equal_the_scan_before_any_edit() {
    let (d, mut m) = session(false);
    m.open_edit();
    let e = m.edit.as_ref().unwrap();
    let scanned = modscan::store::Store::open_read_only(&d.path().join("scan.db"))
        .unwrap()
        .shapes_of(e.source_copy.copy_id)
        .unwrap();
    assert!(!scanned.is_empty());
    assert_eq!(e.cards, scanned);
}

/// Review Minor 6 (bug 1, rename half): when the source on disk renames its shapes, the cards follow the new names
/// with their full content, not a title only.
#[test]
fn edit_cards_follow_a_source_that_renamed_its_shapes() {
    let mut m = direct(two_named(false));
    let e = m.edit.as_mut().unwrap();
    assert_eq!(e.cards[0].name.as_deref(), Some("A"));
    e.reload_source(two_named(true)).unwrap();
    let names: Vec<_> = e.cards.iter().map(|c| c.name.as_deref()).collect();
    assert_eq!(names, [Some("B"), Some("A")]);
    assert!(
        e.cards.iter().all(|c| c.shader_kind == 1),
        "full cards: {:?}",
        e.cards
    );
}

/// Meshes UI T1 (bug 1): a card shows the edited shader type, flags, alpha and texture path, not the scanned ones.
#[test]
fn edit_cards_show_the_edited_values() {
    use sdc_gui::model::meshes::{alpha_line, shader_type_name};
    let (_d, mut m) = session(false);
    m.open_edit();
    m.shapes_col = true;
    let e = m.edit.as_ref().unwrap();
    let (shape, shader) = lit_shape(e);
    let l = &e.model.lighting[&shader].lighting;
    let ty = l.shader_type;
    let new_ty = if ty == 1 { 0 } else { 1 };
    let at = nif::edit::anchor_role(&e.model, Some(shape), Role::Shader).unwrap();
    let set_block = e.model.lighting[&shader].texture_set.unwrap();
    let alpha = e.model.shape(shape).unwrap().alpha;
    let slot0 = String::from_utf8_lossy(&e.model.texsets[&set_block].raw.slots[0]).into_owned();
    let push = |m: &mut MeshesModel, op: Op| m.edit_push(vec![op], None);
    push(
        &mut m,
        Op::SetField {
            at: at.clone(),
            field: Field::ShaderType,
            value: Value::U32(new_ty),
            base: Value::U32(ty),
        },
    );
    // After the type change: it may have changed the flags too (Note CS).
    let f1 = m.edit.as_ref().unwrap().model.lighting[&shader]
        .lighting
        .slsf1;
    push(
        &mut m,
        Op::SetBits {
            at: at.clone(),
            field: Field::Slsf1,
            mask: 1 << 12,
            value: !f1 & (1 << 12),
            base: f1 & (1 << 12),
        },
    );
    let on_shape = nif::edit::anchor_shape(&m.edit.as_ref().unwrap().model, shape);
    push(
        &mut m,
        Op::SetTexturePath {
            at: on_shape,
            slot: 0,
            path: r"Textures\Edited\Card.dds".into(),
            base: slot0,
        },
    );
    let want_alpha = match alpha {
        Some(a) => {
            let e = m.edit.as_ref().unwrap();
            let p = &e.model.alphas[&a].alpha;
            let at = nif::edit::anchor_role(&e.model, Some(shape), Role::Alpha).unwrap();
            let (flags, thr) = (p.flags, p.threshold);
            push(
                &mut m,
                Op::SetBits {
                    at,
                    field: Field::AlphaFlags,
                    mask: 1,
                    value: !flags as u32 & 1,
                    base: flags as u32 & 1,
                },
            );
            alpha_line(flags ^ 1, thr)
        }
        None => {
            let e = m.edit.as_ref().unwrap();
            let at = nif::edit::anchor_shape(&e.model, shape);
            push(
                &mut m,
                Op::AddAlpha {
                    at,
                    flags: 0x00ED,
                    threshold: 128,
                },
            );
            alpha_line(0x00ED, 128)
        }
    };
    let e = m.edit.as_ref().unwrap();
    assert!(
        e.doc.ops.iter().all(|d| d.status == OpStatus::Applied),
        "{:?}",
        e.outcomes
    );
    let f1_now = e.model.lighting[&shader].lighting.slsf1;
    let mut h = view(m);
    h.run_steps(4);
    h.get_by_label(&shader_type_name(new_ty));
    h.get_by_label("textures/edited/card.dds");
    h.get_by_label(&want_alpha);
    // The flags are no longer on the card; the edit panel's Shader tab shows the edited word.
    h.state_mut().select_edit(Selection::Shape(shape));
    h.run_steps(3);
    h.get_by_label(&format!("Shader flags 1 (0x{f1_now:08X})"));
}

#[test]
fn typing_glossiness_records_one_op_and_one_undo_step() {
    let (_d, mut m) = session(false);
    m.open_edit();
    let (shape, shader) = lit_shape(m.edit.as_ref().unwrap());
    m.select_edit(Selection::Shape(shape));
    let mut h = view(m);
    h.run_steps(4);
    h.get_by_label("Glossiness").focus();
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.run_steps(1);
    h.get_by_label("Glossiness").type_text("12.5");
    h.run_steps(1);
    h.key_press(egui::Key::Enter);
    h.run_steps(4);
    let m = h.state_mut();
    let e = m.edit.as_ref().unwrap();
    assert_eq!(e.model.lighting[&shader].lighting.params.glossiness, 12.5);
    assert_eq!(op_values(m).len(), 1, "typing is one op");
    // A drag: two ticks with the panel's merge key, then the drag ends.
    tick(m, 20.5);
    tick(m, 30.5);
    m.edit.as_mut().unwrap().flush();
    assert_eq!(op_values(m).len(), 2, "the drag is one more op");
    assert!(m.undo_edit());
    assert_eq!(op_values(m).len(), 1, "one undo step undoes the drag");
    assert!(m.undo_edit());
    assert!(op_values(m).is_empty());
    assert!(!m.edit.as_ref().unwrap().history.can_undo());
}

#[test]
fn shared_alpha_banner_and_split_on_edit() {
    let mut m = direct(scenes::shared_alpha_and_set());
    let a = select(&mut m, "A");
    let mut h = view(m);
    h.run_steps(2);
    h.get_by_label("Alpha tab").click();
    h.run_steps(2);
    h.get_by_label("Shared by 2 shapes: editing will split.");
    h.get_by_label("Blend").click();
    h.run_steps(2);
    let m = h.state_mut();
    let got = ops(m);
    assert_eq!(got.len(), 2, "{got:?}");
    assert!(matches!(got[0], Op::SplitAlpha { .. }));
    assert!(matches!(
        got[1],
        Op::SetBits {
            field: Field::AlphaFlags,
            mask: 1,
            ..
        }
    ));
    let e = m.edit.as_ref().unwrap();
    assert!(
        e.doc.ops.iter().all(|d| d.status == OpStatus::Applied),
        "{:?}",
        e.outcomes
    );
    let b = shape_named(e, "B");
    let (aa, ab) = (
        e.model.shape(a).unwrap().alpha.unwrap(),
        e.model.shape(b).unwrap().alpha.unwrap(),
    );
    assert_ne!(aa, ab, "A has its own alpha now");
    assert_eq!(e.model.alphas[&aa].alpha.flags, 0x00EC);
    assert_eq!(e.model.alphas[&ab].alpha.flags, 0x00ED, "B is untouched");
    assert!(m.undo_edit());
    assert!(ops(m).is_empty(), "split and edit are one undo step");
}

/// Note FA10: the Alpha threshold slider's second tick sees an unshared alpha and pushes only the SetField; the
/// merged drag must keep the SplitAlpha that started it, or re-applying hits the still-shared alpha (Conflict).
#[test]
fn shared_alpha_threshold_drag_keeps_split() {
    let mut m = direct(scenes::shared_alpha_and_set());
    let a = select(&mut m, "A");
    let (a_name, on_shape, alpha_end, thr0) = {
        let e = m.edit.as_ref().unwrap();
        let alpha = e.model.shape(a).unwrap().alpha.unwrap();
        (
            e.model.shape(a).unwrap().name.as_deref().map(String::from),
            nif::edit::anchor_shape(&e.model, a),
            nif::header::Header::parse(&e.patched).unwrap().sizes.len() as u32,
            e.model.alphas[&alpha].alpha.threshold,
        )
    };
    let set = |at: Anchor, v: u8, base: u8| Op::SetField {
        at,
        field: Field::AlphaThreshold,
        value: Value::U8(v),
        base: Value::U8(base),
    };
    // Tick 1 as the slider builds it for a shared alpha: the SplitAlpha, and the field on the block the split appends.
    let appended = Anchor {
        block: alpha_end,
        kind: "NiAlphaProperty".into(),
        shape: a_name.clone(),
        role: Role::Alpha,
    };
    let key = nif::edit::merge_key(&set(appended.clone(), 0, 0));
    let tick1 = vec![
        Op::SplitAlpha {
            at: on_shape.clone(),
        },
        set(appended, 100, thr0),
    ];
    m.edit_push(tick1, Some(key.clone()));
    // Tick 2 and 3: the alpha is A's own now, so the slider pushes the SetField alone.
    let push_alone = |m: &mut MeshesModel, v: u8| {
        let e = m.edit.as_ref().unwrap();
        let at = nif::edit::anchor_role(&e.model, Some(a), Role::Alpha).unwrap();
        let cur = e.model.alphas[&at.block].alpha.threshold;
        assert_eq!(nif::edit::merge_key(&set(at.clone(), 0, 0)), key);
        m.edit_push(vec![set(at, v, cur)], Some(key.clone()));
    };
    for (i, v) in [150u8, 200].into_iter().enumerate() {
        push_alone(&mut m, v);
        let e = m.edit.as_ref().unwrap();
        let got = ops(&m);
        assert_eq!(got.len(), 2, "tick {}: {got:?}", i + 2);
        // the carried split is A's, and the drag slice is both ops (Split + Set), not just the tail op
        assert!(
            matches!(&got[0], Op::SplitAlpha { at } if at.shape == a_name && at.block == on_shape.block),
            "{got:?}"
        );
        assert!(
            matches!(&got[1], Op::SetField { value: Value::U8(x), base: Value::U8(b), .. } if *x == v && *b == thr0),
            "tick {} keeps the drag's base: {got:?}",
            i + 2
        );
        assert_eq!(e.drag_ops(&key).len(), 2);
        assert!(
            !e.doc.ops.iter().any(|d| d.status == OpStatus::Conflict),
            "tick {}: {:?}",
            i + 2,
            e.outcomes
        );
        assert!(e.doc.ops.iter().all(|d| d.status == OpStatus::Applied));
    }
    let e = m.edit.as_mut().unwrap();
    e.flush();
    let e = m.edit.as_ref().unwrap();
    let b = shape_named(e, "B");
    let (aa, ab) = (
        e.model.shape(a).unwrap().alpha.unwrap(),
        e.model.shape(b).unwrap().alpha.unwrap(),
    );
    assert_ne!(aa, ab, "A has its own alpha");
    assert_eq!(e.model.alphas[&aa].alpha.threshold, 200, "the last tick");
    assert_eq!(e.model.alphas[&ab].alpha.threshold, thr0, "B is untouched");
    assert!(m.undo_edit());
    assert!(ops(&m).is_empty(), "one undo step undoes the whole drag");
    assert!(!m.edit.as_ref().unwrap().history.can_undo());
}

#[test]
fn shared_set_banner_and_texture_edit_keeps_other_shape() {
    let mut m = direct(scenes::shared_alpha_and_set());
    select(&mut m, "A");
    let mut h = view(m);
    h.run_steps(2);
    h.get_by_label("Shared by 2 shapes: editing makes a copy for this shape.");
    let path = r"textures\a.dds";
    let find = |h: &Harness<'static, MeshesModel>| {
        h.query_all_by_role(AkRole::TextInput)
            .find(|n| n.value().as_deref() == Some(path))
            .unwrap_or_else(|| panic!("no box shows {path}"))
            .focus()
    };
    find(&h);
    h.run_steps(1);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.run_steps(1);
    h.query_all_by_role(AkRole::TextInput)
        .find(|n| n.value().as_deref() == Some(path))
        .unwrap()
        .type_text(r"textures\b.dds");
    h.run_steps(1);
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    let e = h.state().edit.as_ref().unwrap();
    assert_eq!(e.doc.ops.len(), 1);
    assert_eq!(e.doc.ops[0].status, OpStatus::Applied, "{:?}", e.outcomes);
    let slot0 = |name: &str| {
        let s = e.model.shape(shape_named(e, name)).unwrap();
        let set = e.model.lighting[&s.shader.unwrap()].texture_set.unwrap();
        String::from_utf8_lossy(&e.model.texsets[&set].raw.slots[0]).into_owned()
    };
    assert_eq!(slot0("A"), r"textures\b.dds");
    assert_eq!(slot0("B"), r"textures\a.dds");
}

#[test]
fn shared_shader_notice_shows_on_the_shader_tab() {
    let mut m = direct(scenes::two_shapes_one_shader());
    select(&mut m, "A");
    let mut h = view(m);
    h.run_steps(2);
    h.get_by_label("Shared by 2 shapes: shader edits change all of them.");
}

#[test]
fn skinned_and_pbr_bits_are_greyed() {
    let mut m = direct(scenes::shared_alpha_and_set());
    let a = select(&mut m, "A");
    let mut h = view(m);
    h.run_steps(2);
    open_flags(&mut h); // Meshes UI T6: the flag lists start closed
    assert!(h.get_by_label("Skinned").accesskit_node().is_disabled());
    assert!(h
        .get_by_label("Unused01 (TruePBR)")
        .accesskit_node()
        .is_disabled());
    assert!(!h.get_by_label("Specular").accesskit_node().is_disabled());
    assert!(h.query_by_label("Specular level").is_none());
    h.get_by_label("Glossiness");
    // A PBR shape: Glossiness and Specular Strength read as CS uses them.
    let m = h.state_mut();
    let e = m.edit.as_ref().unwrap();
    let at = nif::edit::anchor_role(&e.model, Some(a), Role::Shader).unwrap();
    let shader = e.model.shape(a).unwrap().shader.unwrap();
    let base = e.model.lighting[&shader].lighting.slsf2;
    m.edit_push(
        vec![Op::SetBits {
            at,
            field: Field::Slsf2,
            mask: 1 << 23,
            value: 1 << 23,
            base,
        }],
        None,
    );
    h.run_steps(2);
    h.get_by_label("Specular level");
    h.get_by_label("Roughness scale");
    assert!(h.query_by_label("Glossiness").is_none());
}

#[test]
fn own_emit_badge_shows_for_emissive_without_own_emit() {
    let mut m = direct(scenes::shared_alpha_and_set());
    let a = select(&mut m, "A");
    let mut h = view(m);
    h.run_steps(2);
    assert!(h
        .query_by_label("No effect in game: Own_Emit is off")
        .is_none());
    let m = h.state_mut();
    let e = m.edit.as_ref().unwrap();
    let at = nif::edit::anchor_role(&e.model, Some(a), Role::Shader).unwrap();
    m.edit_push(
        vec![Op::SetField {
            at,
            field: Field::EmissiveColor(0),
            value: Value::F32(1.0f32.to_bits()),
            base: Value::F32(0.0f32.to_bits()),
        }],
        None,
    );
    let e = m.edit.as_ref().unwrap();
    assert!(e.warnings.iter().any(|w| matches!(
        w.target,
        Target::Field {
            field: Field::EmissiveColor(0),
            ..
        }
    )));
    h.run_steps(2);
    h.get_by_label("No effect in game: Own_Emit is off");
}

/// Ruling change C4-D7 (replaces `a_shared_alpha_that_cannot_split_is_not_edited`): a shared alpha with a clip
/// controller splits on edit like any shared alpha.
#[test]
fn a_shared_alpha_with_a_clip_controller_splits_on_edit() {
    let mut m = direct(scenes::shared_alpha_with_managed_ctrl());
    let a = select(&mut m, "A");
    let e = m.edit.as_ref().unwrap();
    let b = shape_named(e, "B");
    let (alpha_a, before) = {
        let ab = e.model.shape(b).unwrap().alpha.unwrap();
        (
            e.model.shape(a).unwrap().alpha.unwrap(),
            e.model.alphas[&ab].alpha.flags,
        )
    };
    let mut h = view(m);
    h.run_steps(2);
    h.get_by_label("Alpha tab").click();
    h.run_steps(2);
    assert!(h
        .query_by_label("Shared alpha with a clip controller: splitting needs Phase C.")
        .is_none());
    h.get_by_label("Shared by 2 shapes: editing will split.");
    h.get_by_label("Blend").click();
    h.run_steps(2);
    let m = h.state();
    let got = ops(m);
    assert!(
        matches!(got.as_slice(), [Op::SplitAlpha { .. }, Op::SetBits { .. }]),
        "{got:?}"
    );
    let e = m.edit.as_ref().unwrap();
    assert!(
        e.outcomes.iter().all(|o| matches!(
            o,
            nif::edit::Outcome::Applied | nif::edit::Outcome::ReAnchored { .. }
        )),
        "{:?}",
        e.outcomes
    );
    let ab = e.model.shape(b).unwrap().alpha.unwrap();
    assert_eq!(e.model.alphas[&ab].alpha.flags, before, "B is untouched");
    let aa = e.model.shape(a).unwrap().alpha.unwrap();
    assert_ne!(aa, alpha_a, "A has its own alpha now");
    assert_ne!(e.model.alphas[&aa].alpha.flags, before);
}

/// C4-D7: the Controllers tab offers Split alpha on a clip-shared alpha, and Add stays enabled.
#[test]
fn controllers_tab_offers_split_on_a_clip_shared_alpha() {
    let mut m = direct(scenes::shared_alpha_with_managed_ctrl());
    let a = select(&mut m, "A");
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Controllers");
    h.get_by_label("Shared by 2 shapes: split the alpha to edit its controllers.");
    assert!(!h
        .get_by_label("Add controller to alpha property")
        .accesskit_node()
        .is_disabled());
    h.get_by_label("Split alpha").click();
    h.run_steps(3);
    let m = h.state();
    let got = ops(m);
    assert!(matches!(got.as_slice(), [Op::SplitAlpha { .. }]), "{got:?}");
    let e = m.edit.as_ref().unwrap();
    assert_eq!(e.outcomes, vec![nif::edit::Outcome::Applied]);
    let aa = e.model.shape(a).unwrap().alpha.unwrap();
    assert_eq!(e.model.alphas[&aa].prop.users.len(), 1);
}

/// `shared_alpha_and_set` with shape A's emissive [0.8, 0.6, −0.0] and specular [0.8, 0.6, 0.4]: colours the egui
/// picker's HSV round trip does not give back bit for bit.
fn odd_colours() -> Vec<u8> {
    let b = scenes::shared_alpha_and_set();
    let m = nif::model::read_model(&b).unwrap();
    let a = m
        .shapes
        .iter()
        .find(|s| s.name.as_deref() == Some("A"))
        .unwrap()
        .block;
    let at = nif::edit::anchor_role(&m, Some(a), Role::Shader).unwrap();
    let lp = &m.lighting[&m.shape(a).unwrap().shader.unwrap()]
        .lighting
        .params;
    let mut ops = vec![];
    for (field, now, new) in [
        (
            Field::EmissiveColor as fn(u8) -> Field,
            lp.emissive_color,
            [0.8f32, 0.6, -0.0],
        ),
        (Field::SpecularColor, lp.specular_color, [0.8, 0.6, 0.4]),
    ] {
        for i in 0..3 {
            ops.push(Op::SetField {
                at: at.clone(),
                field: field(i as u8),
                value: Value::F32(new[i].to_bits()),
                base: Value::F32(now[i].to_bits()),
            });
        }
    }
    let r = nif::edit::apply(&b, &ops).unwrap();
    assert!(r.outcomes.iter().all(|o| *o == nif::edit::Outcome::Applied));
    r.bytes
}

/// A17 review: drawing a colour row never records an edit; a real change keeps the other channels' bits.
#[test]
fn drawing_a_colour_records_nothing_and_a_change_keeps_the_other_channels() {
    let mut m = direct(odd_colours());
    let a = select(&mut m, "A");
    let mut h = view(m);
    h.run_steps(8);
    assert!(ops(h.state()).is_empty(), "{:?}", ops(h.state()));
    assert!(!h.state().edit.as_ref().unwrap().history.can_undo());
    h.get_by_label("Specular colour B").focus();
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.run_steps(1);
    h.get_by_label("Specular colour B").type_text("0.25");
    h.run_steps(1);
    h.key_press(egui::Key::Enter);
    h.run_steps(4);
    let e = h.state().edit.as_ref().unwrap();
    assert!(!e.doc.ops.is_empty());
    let l = &e.model.lighting[&e.model.shape(a).unwrap().shader.unwrap()]
        .lighting
        .params;
    let bits = |c: [f32; 3]| c.map(f32::to_bits);
    assert_eq!(bits(l.specular_color), bits([0.8, 0.6, 0.25]));
    assert_eq!(
        bits(l.emissive_color),
        bits([0.8, 0.6, -0.0]),
        "emissive untouched"
    );
}

/// A17 review: every control carries its name and its own role (the row names are painted).
#[test]
fn edit_controls_have_names_and_roles() {
    let mut m = direct(scenes::shared_alpha_and_set());
    select(&mut m, "A");
    let mut h = view(m);
    h.run_steps(2);
    h.get_by_role_and_label(AkRole::ComboBox, "Shader type");
    h.get_by_role_and_label(AkRole::ColorWell, "Specular colour");
    h.get_by_role_and_label(AkRole::TextInput, "Diffuse path (slot 1)");
    h.get_by_label("Alpha tab").click();
    h.run_steps(2);
    for l in ["Source", "Destination", "Function"] {
        h.get_by_role_and_label(AkRole::ComboBox, l);
    }
    h.get_by_role_and_label(AkRole::Slider, "Threshold");
}

/// Review jargon list: the clamp mode names follow nif.xml's TexClampMode (S is U, T is V).
#[test]
fn clamp_mode_labels_follow_the_nif_order() {
    assert_eq!(
        CLAMP_MODE_LABELS,
        [
            "Clamp both",
            "Clamp U, wrap V",
            "Wrap U, clamp V",
            "Wrap both"
        ]
    );
    assert_eq!(clamp_mode_label(3), "Wrap both");
    assert_eq!(clamp_mode_label(7), "Other (7)");
}

/// One BSTriShape "S" with a BSLightingShaderProperty whose clamp mode is `clamp`.
fn one_shape_clamp(clamp: u32) -> Vec<u8> {
    let mut n = Nif::new(100);
    let s = n.string("S");
    n.block("BSTriShape", one_tri(s, 1));
    let mut lsp = scenes::lsp(0, -1);
    lsp[60..64].copy_from_slice(&clamp.to_le_bytes()); // after the shader type, the net, 2 flag words, uv, texture set, emissive
    n.block("BSLightingShaderProperty", lsp);
    n.bytes()
}

/// Review jargon list: a clamp mode outside 0-3 is shown as "Other (7)", drawing never changes it, and a pick of one
/// of the four named values is the usual SetField op.
#[test]
fn an_unknown_clamp_mode_is_shown_and_never_changed() {
    let mut m = direct(one_shape_clamp(7));
    select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(5);
    let combo = h.get_by_role_and_label(AkRole::ComboBox, "Texture wrap");
    assert_eq!(combo.accesskit_node().value().as_deref(), Some("Other (7)"));
    assert!(h.query_by_label("Clamp mode").is_none());
    {
        let e = h.state().edit.as_ref().unwrap();
        assert!(e.doc.ops().is_empty());
        assert!(!e.touched());
    }
    h.get_by_role_and_label(AkRole::ComboBox, "Texture wrap")
        .click();
    h.run_steps(4);
    let other = h.get_by_label("Other (7)");
    assert!(other.accesskit_node().is_disabled());
    h.get_by_label("Wrap both").click();
    h.run_steps(4);
    let o = ops(h.state());
    assert_eq!(o.len(), 1, "{o:?}");
    match &o[0] {
        Op::SetField {
            field, value, base, ..
        } => {
            assert_eq!(*field, Field::ClampMode);
            assert_eq!(*value, Value::U32(3));
            assert_eq!(*base, Value::U32(7));
        }
        other => panic!("{other:?}"),
    }
}

/// Review jargon list: a face-tint shape (type 4) and a block on the NIF panel read in plain words; the class names
/// are on hover only.
#[test]
fn the_nif_panel_uses_plain_labels_and_keeps_the_class_on_hover() {
    let mut n = builder::Nif::new(100);
    let (a, b) = (n.string("A"), n.string("B"));
    n.block("BSTriShape", one_tri(a, 2));
    n.block("BSTriShape", one_tri(b, 3));
    n.block("BSLightingShaderProperty", scenes::lsp(0, -1));
    n.block("BSLightingShaderProperty", scenes::lsp(4, -1));
    let mut m = direct(n.bytes());
    select(&mut m, "B");
    let mut h = view(m);
    h.run_steps(5);
    h.get_by_label("Editing: B").hover();
    h.run_steps(30);
    h.get_by_label_contains("Block type: BSTriShape");
    assert_eq!(
        h.get_by_role_and_label(AkRole::ComboBox, "Shader type")
            .accesskit_node()
            .value()
            .as_deref(),
        Some("Face Tint")
    );
    h.get_by_role_and_label(AkRole::ComboBox, "Texture wrap");
    assert!(h.query_by_label("Clamp mode").is_none());
    h.get_by_label_contains("Shader flags 1 (0x");
    h.get_by_label_contains("Shader flags 2 (0x");
    for bad in ["SLSF1", "SLSF2", "BSDynamicTriShape", "(4)"] {
        assert_eq!(h.query_all_by_label_contains(bad).count(), 0, "{bad}");
    }
}

/// The AccessKit description of the node `h` finds by `label`, read before any hover.
fn described(h: &Harness<'static, MeshesModel>, label: &str) -> Option<String> {
    h.get_by_label(label)
        .accesskit_node()
        .description()
        .map(|d| d.to_string())
}

/// Face-tint shape "B" selected, shown on the Shader tab.
fn face_tint_panel() -> Harness<'static, MeshesModel> {
    let mut n = builder::Nif::new(100);
    let (a, b) = (n.string("A"), n.string("B"));
    n.block("BSTriShape", one_tri(a, 2));
    n.block("BSTriShape", one_tri(b, 3));
    n.block("BSLightingShaderProperty", scenes::lsp(0, -1));
    n.block("BSLightingShaderProperty", scenes::lsp(4, -1));
    let mut m = direct(n.bytes());
    select(&mut m, "B");
    let mut h = view(m);
    h.run_steps(5);
    h
}

/// Round 1: every hover sentence of the jargon list is also the description of its widget, read on the original
/// node before any hover (a screen-reader user gets the class names and the field notes too).
#[test]
fn the_editing_line_describes_its_block_class() {
    let h = face_tint_panel();
    assert_eq!(
        described(&h, "Editing: B").as_deref(),
        Some("Block type: BSTriShape.")
    );
}

#[test]
fn the_shader_type_combo_describes_its_number_and_nothing_more() {
    let h = face_tint_panel();
    assert_eq!(
        h.get_by_role_and_label(AkRole::ComboBox, "Shader type")
            .accesskit_node()
            .description()
            .as_deref(),
        Some("Shader type 4 in the NIF.")
    );
}

#[test]
fn the_texture_wrap_combo_describes_the_clamp_mode() {
    let h = face_tint_panel();
    let wrap = h.get_by_role_and_label(AkRole::ComboBox, "Texture wrap");
    assert_eq!(
        wrap.accesskit_node().description().as_deref(),
        Some(
            "Clamp mode 3 in the NIF. Wrap repeats the texture past its edge; clamp stretches the edge pixel."
        )
    );
}

#[test]
fn the_flag_headers_describe_their_nifskope_names() {
    let h = face_tint_panel();
    let f1 = h.get_by_label_contains("Shader flags 1 (0x");
    assert_eq!(
        f1.accesskit_node().description().as_deref(),
        Some("SLSF1 in NifSkope.")
    );
    let f2 = h.get_by_label_contains("Shader flags 2 (0x");
    assert_eq!(
        f2.accesskit_node().description().as_deref(),
        Some("SLSF2 in NifSkope.")
    );
}

#[test]
fn the_no_alpha_line_describes_the_missing_block() {
    let mut h = face_tint_panel();
    h.get_by_label("Alpha tab").click();
    h.run_steps(4);
    assert_eq!(
        described(&h, "This shape has no alpha settings.").as_deref(),
        Some("The NIF has no NiAlphaProperty block for this shape.")
    );
}
/// Round 1: the root's class reads as a description too (a class the panel knows by sentence, and one it does not).
#[test]
fn the_editing_line_describes_the_root_and_the_dynamic_shape() {
    let mut m = direct(ctrl_scene(0x82));
    m.select_edit(Selection::Root);
    let mut h = view(m);
    h.run_steps(3);
    assert_eq!(
        described(&h, "Editing: Root").as_deref(),
        Some("Block type: NiNode.")
    );
    let mut n = Nif::new(100);
    let s = n.string("D");
    n.block("BSDynamicTriShape", one_tri(s, -1));
    let mut m = direct(n.bytes());
    select(&mut m, "D");
    let mut h = view(m);
    h.run_steps(3);
    assert_eq!(
        described(&h, "Editing: D").as_deref(),
        Some("Block type: BSDynamicTriShape, a shape whose vertices the game can move while it plays, as on heads and faces.")
    );
}

/// Round 2: a cut editing line (a long name in a narrow panel) keeps its class: the description holds the whole line
/// and then the class sentence, set on the original node before any hover.
#[test]
fn a_cut_editing_line_still_describes_its_block_class() {
    let name = "A_very_long_shape_name_".repeat(12);
    let mut n = Nif::new(100);
    let s = n.string(&name);
    n.block("BSTriShape", one_tri(s, -1));
    let mut m = direct(n.bytes());
    select(&mut m, &name);
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(700.0, 900.0))
        .build_ui_state(
            |ui, m: &mut MeshesModel| sdc_gui::ui::meshes::show(ui, m),
            m,
        );
    h.run_steps(5);
    let line = format!("Editing: {name}");
    let node = h.get_by_label(&line);
    assert!(
        node.accesskit_node()
            .bounding_box()
            .is_some_and(|b| b.width() < 500.0),
        "the line is cut by the narrow panel"
    );
    assert_eq!(
        node.accesskit_node().description().as_deref(),
        Some(format!("{line} Block type: BSTriShape.").as_str())
    );
}
/// Round 1: the Undo step names of the relabelled fields, as the shared Undo returns them after a real edit.
#[test]
fn the_undo_steps_of_the_relabelled_fields_read_in_plain_words() {
    let mut m = direct(scenes::shared_alpha_and_set());
    let a = select(&mut m, "A");
    let mut h = view(m);
    sdc_gui::ui::header::set_meshes_view(&h.ctx, true);
    h.run_steps(3);
    let at = {
        let e = h.state().edit.as_ref().unwrap();
        nif::edit::anchor_role(&e.model, Some(a), Role::Shader).unwrap()
    };
    let bits = |field, base| Op::SetBits {
        at: at.clone(),
        field,
        mask: 1,
        value: 1,
        base,
    };
    let wrap = Op::SetField {
        at: at.clone(),
        field: Field::ClampMode,
        value: Value::U32(1),
        base: Value::U32(3),
    };
    let (f1, f2) = {
        let e = h.state().edit.as_ref().unwrap();
        let l = &e.model.lighting[&e.model.shape(a).unwrap().shader.unwrap()].lighting;
        (l.slsf1, l.slsf2)
    };
    for (op, want) in [
        (bits(Field::Slsf1, f1 & !1), "Set shader flags 1"),
        (bits(Field::Slsf2, f2 & !1), "Set shader flags 2"),
        (wrap, "Set texture wrap"),
    ] {
        h.state_mut().edit_push(vec![op], None);
        h.run_steps(4);
        let u = sdc_gui::ui::meshes::undo_target(&h.ctx).expect("the NIF history");
        assert_eq!(u.undo_label.as_deref(), Some(want));
    }
}
/// A colour change is three channel ops; a drag of the picker merges them as three ops with the first bases.
#[test]
fn a_colour_drag_is_three_ops_with_the_original_bases() {
    let mut m = direct(scenes::shared_alpha_and_set());
    let a = select(&mut m, "A");
    let e = m.edit.as_ref().unwrap();
    let at = nif::edit::anchor_role(&e.model, Some(a), Role::Shader).unwrap();
    let push = |m: &mut MeshesModel, v: f32, base: f32| {
        let ops = (0..3)
            .map(|i| Op::SetField {
                at: at.clone(),
                field: Field::SpecularColor(i),
                value: Value::F32(v.to_bits()),
                base: Value::F32(base.to_bits()),
            })
            .collect();
        m.edit_push(ops, Some("Color|x|Specular colour".into()));
    };
    push(&mut m, 0.5, 1.0);
    push(&mut m, 0.25, 0.5);
    let got = ops(&m);
    assert_eq!(got.len(), 3, "{got:?}");
    for op in &got {
        let Op::SetField { value, base, .. } = op else {
            panic!("{op:?}")
        };
        assert_eq!(*value, Value::F32(0.25f32.to_bits()));
        assert_eq!(*base, Value::F32(1.0f32.to_bits()), "the drag's first base");
    }
    assert!(m.undo_edit());
    assert!(ops(&m).is_empty(), "one undo step");
}

/// Layout v2: the edit panel sits beside the 3D view under the shared header, above the open bottom panel.
#[test]
fn the_edit_panel_fits_beside_the_3d_view_with_the_header_and_bottom_panel() {
    use sdc_gui::app::SdcApp;
    app_data();
    let d = files(false);
    let project = tempfile::tempdir().unwrap();
    let model = support::opened_as_the_app(
        project.path().to_owned(),
        d.path().join("mods").join(EDIT_MOD),
    );
    let mut app = SdcApp::new(model);
    app.meshes.procs = sdc_gui::model::meshes::no_procs;
    app.meshes.store_path = Some(d.path().join("scan.db"));
    scan_and_select(&mut app.meshes, d.path());
    app.meshes.open_edit();
    let (shape, _) = lit_shape(app.meshes.edit.as_ref().unwrap());
    app.meshes.select_edit(Selection::Shape(shape));
    app.model.show_tab(sdc_gui::model::bottom::BottomTab::Files);
    let mut frame = eframe::Frame::_new_kittest();
    let (w, hgt) = (1280.0, 800.0);
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(w, hgt))
        .build_ui_state(
            move |ui, app: &mut SdcApp| eframe::App::ui(app, ui, &mut frame),
            app,
        );
    sdc_gui::ui::header::set_meshes_view(&h.ctx, true);
    h.run_steps(6);
    // The shared header and the open bottom panel are still there.
    for l in ["Save", "Files"] {
        h.get_by_label(l);
    }
    support::header_export_label(&h);
    let panel = |id: &str| support::panel(&h.ctx, id);
    let (edit, tree, bottom) = (panel("meshes-edit"), panel("meshes-tree"), panel("bottom"));
    assert!(
        edit.max.x <= w + 0.5,
        "edit panel past the window: {edit:?}"
    );
    assert!(
        tree.max.x <= edit.min.x,
        "tree {tree:?} under edit {edit:?}"
    );
    assert!(
        edit.max.y <= bottom.min.y + 0.5,
        "edit {edit:?} over bottom {bottom:?}"
    );
    let frame3d = h
        .get_by_label(sdc_gui::model::viewport3d::FRAME_LABEL)
        .rect();
    assert!(
        frame3d.max.x <= edit.min.x,
        "3D {frame3d:?} under edit {edit:?}"
    );
    assert!(frame3d.min.x >= tree.max.x);
    assert!(
        frame3d.width() > 100.0 && frame3d.height() > 100.0,
        "{frame3d:?}"
    );
    let folder = h.get_by_label(sdc_gui::ui::viewport3d::VIEW_MENU).rect();
    assert!(
        folder.max.x <= edit.min.x,
        "3D toolbar pushed under the panel"
    );
    for l in [
        "Shader tab",
        "Alpha tab",
        "Controllers tab",
        "Extra data tab",
    ] {
        let r = h.get_by_label(l).rect();
        assert!(
            edit.contains(r.center()),
            "{l} {r:?} outside the panel {edit:?}"
        );
    }
    // Note GH2: the panel has no Undo / Redo of its own; the header names whose history it acts on (Note FC12).
    for l in ["Undo NIF edit", "Redo NIF edit"] {
        assert!(h.query_by_label(l).is_none(), "{l} is back in the panel");
    }
    for verb in ["Undo", "Redo"] {
        let l = support::header_undo(&h, verb)
            .accesskit_node()
            .label()
            .unwrap();
        // No NIF edit yet, so no step follows the history's name.
        assert_eq!(l, format!("{verb} (NIF edit history)"));
    }
    // Meshes UI T3: the footer sits under the tabs, inside the panel, with its blocked reason; Note GD2: its button
    // is Review…, not Export NIF.
    let footer = panel("meshes-edit-export");
    assert!(
        h.query(export_nif()).is_none(),
        "the footer has no Export NIF"
    );
    let export = h.get(review_button()).rect();
    let tabs = h.get_by_label("Extra data tab").rect();
    assert!(
        edit.contains(footer.min) && edit.contains(footer.max - egui::vec2(0.5, 0.5)),
        "footer {footer:?} outside {edit:?}"
    );
    assert!(
        footer.contains(export.center()),
        "{export:?} not in {footer:?}"
    );
    assert!(
        export.min.y > tabs.max.y,
        "Review… {export:?} above the tabs {tabs:?}"
    );
    // Minor 5: an untouched session is idle, not blocked: weak text, no red.
    h.get_by_label("No edits to export.");
    assert!(h.query_by_label("Blocked: No edits to export.").is_none());
    support::reveal_header_control(&mut h, "MO2…");
}

// ---- v2 A18: Controllers (scalars) and Extra data tabs ----

use builder::{Nif, W};

/// Rulings B-P28, B-P29: an authored TBC key, decreasing or non-finite key times block export (the `validate_ops`
/// wiring).
#[test]
fn authored_key_errors_block_export() {
    let b = scenes::keyed_pair(scenes::Share::No, [1, 2]);
    let base = nif::keys::read_data(&nif::header::Header::parse(&b).unwrap(), &b, 6).unwrap();
    for (key_type, times, want) in [
        (
            3,
            [0.0, 1.0],
            "Only linear and quadratic keys can be authored",
        ),
        (1, [1.0, 0.0], "Key times must not decrease"),
        (1, [0.0, f32::NAN], "Key values must be finite numbers"),
    ] {
        let d = tempfile::tempdir().unwrap();
        let edit_mod = d.path().join("mods").join(EDIT_MOD);
        let mut e = NifEdit::open(
            b.clone(),
            copy("A", 1, false),
            "meshes/x.nif",
            None,
            Some(&edit_mod),
            None,
        )
        .unwrap();
        let role = Role::Ctrl {
            owner: nif::edit::Owner::Shader,
            class: "BSLightingShaderPropertyFloatController".into(),
            var: Some(11),
            nth: 0,
        };
        let tbc = (key_type == 3).then_some([0.5, -0.25, 1.0]);
        let op = Op::SetKeys {
            at: nif::edit::anchor_role(&e.model, Some(0), role).unwrap(),
            clip: None,
            clip_name: None,
            keys: KeyData::Float(KeyGroup {
                key_type: Some(key_type),
                keys: times.iter().map(|&t| fkey(t, 1.0, tbc)).collect(),
            }),
            base: base.clone(),
        };
        e.push(vec![op], None);
        assert_eq!(e.export_block().as_deref(), Some(want));
    }
}

/// Shape "S" (0) -> BSLSP (1) -> free float controllers var 11 (2, interp 4) and var 20 (3, interp 5); a root NiNode
/// (12) listing BSXFlags `bsx` (6), a BSInvMarker, a multi-line string, an int, a bool and a negative-zero float.
fn ctrl_scene(bsx: u32) -> Vec<u8> {
    ctrl_scene_vars(bsx, [11, 20])
}

fn ctrl_scene_vars(bsx: u32, vars: [u32; 2]) -> Vec<u8> {
    let mut n = Nif::new(100);
    let (s, r) = (n.string("S"), n.string("Root"));
    n.block("BSTriShape", scenes::bstri_named(s, 1, -1));
    n.block("BSLightingShaderProperty", scenes::lsp_body(0, &[], 2, &[]));
    for (next, var, interp) in [(3, vars[0], 4), (-1, vars[1], 5)] {
        let tail = W::default().i32(interp).u32(var);
        n.block(
            "BSLightingShaderPropertyFloatController",
            scenes::ctrl_body(next, 0x48, 1, tail),
        );
    }
    n.block("NiFloatInterpolator", W::default().f(0.5).i32(-1).0);
    n.block("NiFloatInterpolator", W::default().f(0.25).i32(-1).0);
    let names = ["BSX", "INV", "Tag", "Count", "PG_IGNORE", "Scale"].map(|x| n.string(x));
    let val = n.string("a\r\nb");
    n.block("BSXFlags", W::default().i32(names[0]).u32(bsx).0);
    n.block(
        "BSInvMarker",
        W::default()
            .i32(names[1])
            .u16(4712)
            .u16(6283)
            .u16(0)
            .f(1.0)
            .0,
    );
    n.block("NiStringExtraData", W::default().i32(names[2]).i32(val).0);
    n.block("NiIntegerExtraData", W::default().i32(names[3]).u32(7).0);
    n.block("NiBooleanExtraData", W::default().i32(names[4]).u8(1).0);
    n.block("NiFloatExtraData", W::default().i32(names[5]).f(-0.0).0);
    let mut w = W::default().i32(r).u32(6);
    for e in 6..12 {
        w = w.i32(e);
    }
    n.block("NiNode", scenes::avobj(w.i32(-1)).u32(0).u32(0).0);
    n.roots = vec![12];
    n.bytes()
}

fn tab(h: &mut Harness<'static, MeshesModel>, name: &str) {
    h.get_by_label(&format!("{name} tab")).click();
    h.run_steps(3);
}

fn edit_number(h: &mut Harness<'static, MeshesModel>, label: &str, text: &str) {
    h.get_by_label(label).focus();
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.run_steps(1);
    h.get_by_label(label).type_text(text);
    h.run_steps(1);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
}

#[test]
fn controller_rows_show_variable_cycle_and_edit_frequency() {
    let mut m = direct(ctrl_scene(0x82));
    select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Controllers");
    h.get_by_label("Emissive multiple");
    h.get_by_label("U offset");
    h.get_by_role_and_label(AkRole::ComboBox, "Emissive multiple cycle");
    h.get_by_role_and_label(AkRole::ComboBox, "Emissive multiple variable");
    h.get_by_role_and_label(AkRole::CheckBox, "Emissive multiple active");
    assert!(ops(h.state()).is_empty(), "drawing records nothing");
    edit_number(&mut h, "Emissive multiple frequency", "2");
    let got = ops(h.state());
    assert_eq!(got.len(), 1, "{got:?}");
    let Op::SetField {
        at,
        field,
        value,
        base,
    } = &got[0]
    else {
        panic!("{got:?}")
    };
    assert_eq!(*field, Field::Frequency);
    assert_eq!(*value, Value::F32(2.0f32.to_bits()));
    assert_eq!(*base, Value::F32(1.0f32.to_bits()));
    assert!(
        matches!(
            &at.role,
            Role::Ctrl {
                var: Some(11),
                nth: 0,
                ..
            }
        ),
        "{at:?}"
    );
    let e = h.state().edit.as_ref().unwrap();
    assert_eq!(e.model.ctrls[&2].frequency, 2.0);
    assert_eq!(e.model.ctrls[&3].frequency, 1.0, "the other row untouched");
    // The interpolator value goes through the controller's anchor.
    edit_number(&mut h, "U offset value", "0.75");
    let e = h.state().edit.as_ref().unwrap();
    assert_eq!(e.model.interps[&5].value, vec![0.75], "{:?}", e.outcomes);
}

/// The managed-clip scene in a direct session with its shape selected, on the Controllers tab.
fn clip_view() -> Harness<'static, MeshesModel> {
    let mut m = direct(scenes::managed_clip());
    let b = m.edit.as_ref().unwrap().model.shapes[0].block;
    m.select_edit(Selection::Shape(b));
    let mut h = view(m);
    h.run_steps(2);
    h.get_by_label("Controllers tab").click();
    h.run_steps(3);
    h
}

/// Note GR1: the Clips section holds only the Preview picker; no clip editing control is offered, the picker lists
/// both clips, and a managed controller's "In clip" key table still edits that clip.
#[test]
fn the_clips_section_has_no_editing_controls() {
    let mut h = clip_view();
    // The section is open by default: its picker is on screen, so the absences below are not just a closed section.
    h.get_by_label("Clips (2 · showing free chains)");
    h.get_by_role_and_label(AkRole::ComboBox, "Preview animation");
    for l in [
        "Add clip",
        "Remove clip",
        "Convert to clips",
        "Convert shader controllers to clips",
        "Clip name",
        "New clip name",
        "Clip cycle",
        "Clip frequency",
        "Clip start",
        "Clip stop",
    ] {
        assert!(h.query_by_label(l).is_none(), "{l} is offered");
    }
    assert!(h.query_by_label_contains("Remove clip").is_none());
    assert!(h.query_by_label_contains("Add clip").is_none());
    h.get_by_role_and_label(AkRole::ComboBox, "Preview animation")
        .click();
    h.run_steps(3);
    h.get_by_role_and_label(AkRole::Button, "Free chains");
    let names = {
        let e = h.state().edit.as_ref().unwrap();
        e.model
            .sequences
            .iter()
            .map(|s| s.name.clone().unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(names.len(), 2);
    for n in &names {
        h.get_by_role_and_label(AkRole::Button, n);
    }
    // Pick the first clip: its key table edits that clip.
    h.get_by_role_and_label(AkRole::Button, &names[0]).click();
    h.run_steps(3);
    let seq = h.state().edit.as_ref().unwrap().model.sequences[0].block;
    assert_eq!(h.state().edit.as_ref().unwrap().playing, Playing::Clip(seq));
    assert!(h.query_all_by_label_contains("In clip ").count() >= 1);
    let add = format!("Add key to Emissive multiple in {}", names[0]);
    reveal(&mut h, &add);
    h.get_by_label(&add).click();
    h.run_steps(3);
    let last = ops(h.state()).pop();
    assert!(
        matches!(last, Some(Op::SetKeys { clip: Some(s), .. }) if s == seq),
        "{last:?}"
    );
}

/// T4 review Minor 2: on a shape with no shader and no alpha the Clips section (the Preview picker) is still there.
#[test]
fn the_clips_section_shows_on_a_shape_without_properties() {
    let mut m = direct(scenes::managed_clip_and_bare_shape());
    m.select_edit(Selection::Shape(12));
    let mut h = view(m);
    h.run_steps(2);
    h.get_by_label("Controllers tab").click();
    h.run_steps(3);
    h.get_by_label("This shape has no shader or alpha property.");
    h.get_by_label("Clips (2 · showing free chains)");
    h.get_by_role_and_label(AkRole::ComboBox, "Preview animation");
}

/// R8 lifted (v2 B7, E9: replaces `managed_variable_combo_is_disabled`): the Variable combo of a managed controller
/// is enabled; a pick rewrites the clip's ID.
#[test]
fn managed_variable_change_rewrites_clip_ids() {
    let mut m = MeshesModel::empty();
    m.edit = Some(
        NifEdit::open(
            scenes::managed_clip(),
            copy("A", 1, false),
            "meshes/x.nif",
            None,
            None,
            None,
        )
        .unwrap(),
    );
    let b = m.edit.as_ref().unwrap().model.shapes[0].block;
    m.select_edit(Selection::Shape(b));
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Controllers");
    h.get_by_label("Managed by a clip");
    let var = h.get_by_role_and_label(AkRole::ComboBox, "Emissive multiple variable");
    assert!(!var.accesskit_node().is_disabled());
    var.click();
    h.run_steps(3);
    h.get_by_role_and_label(AkRole::Button, "U offset (20)")
        .click();
    h.run_steps(3);
    let got = ops(h.state());
    assert!(
        matches!(
            &got[..],
            [Op::SetField {
                field: Field::Variable,
                value: Value::U32(20),
                base: Value::U32(11),
                ..
            }]
        ),
        "{got:?}"
    );
    let e = h.state().edit.as_ref().unwrap();
    assert_eq!(e.outcomes, vec![nif::edit::Outcome::Applied]);
    assert_eq!(
        e.model.sequences[0].blocks[0].ctrl_id.as_deref(),
        Some(&b"20"[..])
    );
    assert_eq!(e.model.sequences[0].blocks[1].ctrl_id, None);
}

#[test]
fn bsx_bits_toggle_and_inv_marker_round_trips() {
    let mut m = direct(ctrl_scene(0x82));
    select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Extra data");
    // Animated is ticked and locked: a free controller animates this model (D9).
    let animated = h.get_by_role_and_label(AkRole::CheckBox, "Animated");
    assert!(animated.accesskit_node().is_disabled());
    assert_eq!(
        animated.accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::True)
    );
    h.get_by_role_and_label(AkRole::CheckBox, "Havok").click();
    h.run_steps(3);
    let got = ops(h.state());
    assert_eq!(got.len(), 1, "{got:?}");
    let Op::EditExtra { value, base, at } = &got[0] else {
        panic!("{got:?}")
    };
    assert_eq!(*value, ExtraValue::Bsx(0x82 & !2));
    assert_eq!(*base, ExtraValue::Bsx(0x82));
    assert!(
        matches!(&at.role, Role::Extra { on_root: true, .. }),
        "{at:?}"
    );
    edit_number(&mut h, "Rotation X", "90");
    let e = h.state().edit.as_ref().unwrap();
    let inv = e.model.extras.values().find(|x| &*x.kind == "BSInvMarker");
    let Some(ExtraValue::Inv { rot, zoom }) = inv.and_then(|x| x.value.clone()) else {
        panic!("{inv:?}")
    };
    assert_eq!(rot, [1571, 6283, 0]);
    assert_eq!(zoom, 1.0f32.to_bits());
}

/// Animated is not locked when no property has a free controller, and ticking it writes `old | 1`.
#[test]
fn animated_ticks_when_nothing_is_animated() {
    let mut m = direct(scenes::emit_scene("BSX", 0x82, true));
    m.select_edit(Selection::Root);
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Extra data");
    h.get_by_role_and_label(AkRole::CheckBox, "Animated")
        .click();
    h.run_steps(3);
    let got = ops(h.state());
    let Some(Op::EditExtra { value, .. }) = got.first() else {
        panic!("{got:?}")
    };
    assert_eq!(*value, ExtraValue::Bsx(0x83));
}

#[test]
fn add_and_remove_string_extra_on_a_shape() {
    let mut m = direct(ctrl_scene(0x82));
    let s = select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Extra data");
    h.get_by_role_and_label(AkRole::ComboBox, "Add type (shape)")
        .click();
    h.run_steps(3);
    h.get_by_role_and_label(AkRole::Button, "NiStringExtraData")
        .click();
    h.run_steps(3);
    h.get_by_role_and_label(AkRole::TextInput, "Name (shape)")
        .focus();
    h.run_steps(2);
    h.get_by_role_and_label(AkRole::TextInput, "Name (shape)")
        .type_text("Note");
    h.run_steps(2);
    h.get_by_label("Add to shape").click();
    h.run_steps(4);
    let e = h.state().edit.as_ref().unwrap();
    let net = &e.model.shape(s).unwrap().net;
    let added = e.model.extras_of(net);
    assert_eq!(added.len(), 1, "{:?}", e.outcomes);
    assert_eq!(&*added[0].kind, "NiStringExtraData");
    assert_eq!(added[0].name.as_deref(), Some(&b"Note"[..]));
    assert!(matches!(ops(h.state())[0], Op::AddExtra { .. }));
    h.get_by_label("Remove Note (shape)").click();
    h.run_steps(4);
    let e = h.state().edit.as_ref().unwrap();
    assert!(e.model.extras_of(&e.model.shape(s).unwrap().net).is_empty());
    assert!(matches!(
        ops(h.state()).last(),
        Some(Op::RemoveExtra { .. })
    ));
    // The root's extras were never touched.
    assert_eq!(
        e.model.extras_of(e.model.root_net.as_ref().unwrap()).len(),
        6
    );
}

/// Opens the Known picker of the shape (`root` false) or root list and clicks `entry`.
fn pick_known(h: &mut Harness<'static, MeshesModel>, root: bool, entry: &str) {
    let combo = if root {
        "Known extra data"
    } else {
        "Known extra data (shape)"
    };
    h.get_by_role_and_label(AkRole::ComboBox, combo)
        .scroll_to_me();
    h.run_steps(12);
    h.get_by_role_and_label(AkRole::ComboBox, combo).click();
    h.run_steps(3);
    h.get_by_role_and_label(AkRole::Button, entry).click();
    h.run_steps(3);
}

fn known_entry_disabled(h: &mut Harness<'static, MeshesModel>, root: bool, entry: &str) -> bool {
    let combo = if root {
        "Known extra data"
    } else {
        "Known extra data (shape)"
    };
    h.get_by_role_and_label(AkRole::ComboBox, combo)
        .scroll_to_me();
    h.run_steps(12);
    h.get_by_role_and_label(AkRole::ComboBox, combo).click();
    h.run_steps(3);
    let off = h
        .get_by_role_and_label(AkRole::Button, entry)
        .accesskit_node()
        .is_disabled();
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    off
}

/// Meshes UI T5: the shape's Known picker adds AnisotropicAlphaMaterial as NiIntegerExtraData 3; root-only entries
/// are off there ("Goes on the root").
#[test]
fn known_picker_adds_anisotropic_alpha_on_a_shape() {
    let mut m = direct(ctrl_scene(0x82));
    let s = select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Extra data");
    assert!(
        known_entry_disabled(&mut h, false, "BSX"),
        "BSX goes on the root"
    );
    // Owner, 2026-10-22: BODYTRI is not offered (BodySlide / Outfit Studio writes it).
    h.get_by_role_and_label(AkRole::ComboBox, "Known extra data (shape)")
        .click();
    h.run_steps(3);
    assert!(h
        .query_by_role_and_label(AkRole::Button, "BODYTRI")
        .is_none());
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    pick_known(&mut h, false, "AnisotropicAlphaMaterial");
    h.get_by_label("Add to shape").click();
    h.run_steps(4);
    let e = h.state().edit.as_ref().unwrap();
    let added = e.model.extras_of(&e.model.shape(s).unwrap().net);
    assert_eq!(added.len(), 1, "{:?}", e.outcomes);
    assert!(
        matches!(
            &ops(h.state())[0],
            Op::AddExtra { kind, name, value: ExtraValue::Int(3), .. }
                if kind == "NiIntegerExtraData" && name == "AnisotropicAlphaMaterial"
        ),
        "{:?}",
        ops(h.state())
    );
}

/// Meshes UI T5: on the root, BSX and INV are off when present ("Already on this root"); HDT SMP's Add waits for a
/// path; Prn is offered marked unconfirmed (owner decision 1).
#[test]
fn known_picker_on_the_root_offers_hdt_with_a_path_and_marks_prn() {
    let mut m = direct(ctrl_scene(0x82));
    select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Extra data");
    assert!(
        known_entry_disabled(&mut h, true, "BSX"),
        "BSX is already there"
    );
    assert!(
        known_entry_disabled(&mut h, true, "AnisotropicAlphaMaterial"),
        "goes on a shape"
    );
    assert!(!known_entry_disabled(&mut h, true, "Prn (unconfirmed)"));
    pick_known(&mut h, true, "HDT Skinned Mesh Physics Object");
    assert!(
        h.get_by_label("Add to root").accesskit_node().is_disabled(),
        "needs a path"
    );
    h.get_by_role_and_label(AkRole::TextInput, "Value").focus();
    h.run_steps(2);
    h.get_by_role_and_label(AkRole::TextInput, "Value")
        .type_text(r"SKSE\Plugins\hdtSkinnedMeshConfigs\x.xml");
    h.run_steps(2);
    h.get_by_label("Add to root").click();
    h.run_steps(4);
    let got = ops(h.state());
    assert!(
        matches!(
            &got[..],
            [Op::AddExtra { kind, name, value: ExtraValue::Str(p), .. }]
                if kind == "NiStringExtraData"
                    && name == "HDT Skinned Mesh Physics Object"
                    && &**p == r"SKSE\Plugins\hdtSkinnedMeshConfigs\x.xml"
        ),
        "{got:?}"
    );
}

/// Meshes UI T5 (bug 3): an extra-data row shows its own warnings, here "BSXFlags must be named BSX".
#[test]
fn an_extra_row_shows_its_warnings() {
    let mut m = direct(ctrl_scene(0x82));
    let at = nif::edit::anchor_role(&m.edit.as_ref().unwrap().model, None, Role::Root).unwrap();
    m.edit_push(
        vec![Op::AddExtra {
            at,
            kind: "BSXFlags".into(),
            name: "BSX2".into(),
            value: ExtraValue::Bsx(0),
        }],
        None,
    );
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Extra data");
    // T5 review Minor 6: under its own row (after BSX2, before the root's Add row), once.
    let w = h.get_by_label("BSXFlags must be named BSX").rect();
    let row = h.get_by_label("BSX2").rect();
    let add = h
        .get_by_role_and_label(AkRole::ComboBox, "Known extra data")
        .rect();
    assert!(
        row.max.y <= w.min.y && w.max.y <= add.min.y,
        "{row:?} {w:?} {add:?}"
    );
}

/// T5 review Minors 2-4: Prn needs a value, a typed value is trimmed, and a duplicate is found ignoring ASCII case
/// (the SMP reader compares the HDT name that way).
#[test]
fn known_values_are_required_trimmed_and_duplicates_ignore_case() {
    let mut m = direct(ctrl_scene(0x82));
    select(&mut m, "S");
    let at = nif::edit::anchor_role(&m.edit.as_ref().unwrap().model, None, Role::Root).unwrap();
    m.edit_push(
        vec![Op::AddExtra {
            at,
            kind: "NiStringExtraData".into(),
            name: "hdt skinned mesh physics object".into(),
            value: ExtraValue::Str("x.xml".into()),
        }],
        None,
    );
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Extra data");
    assert!(
        known_entry_disabled(&mut h, true, "HDT Skinned Mesh Physics Object"),
        "the same name in lower case is already there"
    );
    pick_known(&mut h, true, "Prn (unconfirmed)");
    assert!(
        h.get_by_label("Add to root").accesskit_node().is_disabled(),
        "Prn needs a node name"
    );
    h.get_by_role_and_label(AkRole::TextInput, "Value").focus();
    h.run_steps(2);
    h.get_by_role_and_label(AkRole::TextInput, "Value")
        .type_text("  WeaponBack  ");
    h.run_steps(2);
    h.get_by_label("Add to root").click();
    h.run_steps(4);
    let got = ops(h.state());
    assert!(
        matches!(got.last(), Some(Op::AddExtra { name, value: ExtraValue::Str(v), .. }) if name == "Prn" && &**v == "WeaponBack"),
        "{got:?}"
    );
}

/// T5 review Minor 6: after a Known Add the picker is back on Custom… and the entry is off ("Already on this
/// shape"); an existing row of a Known entry shows its tooltip on the name.
#[test]
fn a_known_add_falls_back_to_custom_and_rows_show_the_tip() {
    let mut m = direct(ctrl_scene(0x82));
    select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Extra data");
    pick_known(&mut h, false, "AnisotropicAlphaMaterial");
    h.get_by_label("Add to shape").click();
    h.run_steps(4);
    assert_eq!(
        h.get_by_role_and_label(AkRole::ComboBox, "Known extra data (shape)")
            .accesskit_node()
            .value()
            .as_deref(),
        Some("Custom…")
    );
    assert!(known_entry_disabled(
        &mut h,
        false,
        "AnisotropicAlphaMaterial"
    ));
    h.get_by_label("AnisotropicAlphaMaterial").hover();
    h.run_steps(30);
    h.get_by_label_contains("Community Shaders see-through look");
}

/// Drawing both tabs records nothing, whatever the values' bits (negative zero, CRLF).
#[test]
fn drawing_the_controller_and_extra_tabs_records_nothing() {
    let mut m = direct(ctrl_scene(0x83));
    select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(2);
    for t in ["Controllers", "Extra data", "Controllers", "Extra data"] {
        tab(&mut h, t);
        h.run_steps(6);
    }
    assert!(ops(h.state()).is_empty(), "{:?}", ops(h.state()));
    assert!(!h.state().edit.as_ref().unwrap().history.can_undo());
}

/// P9: the value of an interpolator that two controllers share is greyed, and no edit reaches it.
#[test]
fn a_shared_interpolator_value_is_greyed() {
    let mut m = direct(scenes::shader_ctrl(true));
    select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Controllers");
    let v = h.get_by_label("Emissive multiple value");
    assert!(v.accesskit_node().is_disabled());
    assert!(ops(h.state()).is_empty());
    // The controller's own fields stay editable.
    let f = h.get_by_label("Emissive multiple frequency");
    assert!(!f.accesskit_node().is_disabled());
}

/// The name being typed in the Add row belongs to the shape it was typed for.
#[test]
fn the_add_row_name_does_not_follow_to_another_shape() {
    let mut m = direct(scenes::shared_alpha_and_set());
    select(&mut m, "A");
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Extra data");
    let name = |h: &Harness<'static, MeshesModel>| {
        h.get_by_role_and_label(AkRole::TextInput, "Name (shape)")
            .accesskit_node()
            .value()
            .map(|s| s.to_string())
    };
    h.get_by_role_and_label(AkRole::TextInput, "Name (shape)")
        .focus();
    h.run_steps(2);
    h.get_by_role_and_label(AkRole::TextInput, "Name (shape)")
        .type_text("Foo");
    h.run_steps(2);
    assert_eq!(name(&h).as_deref(), Some("BSXFoo"));
    let b = shape_named(h.state().edit.as_ref().unwrap(), "B");
    h.state_mut().select_edit(Selection::Shape(b));
    h.run_steps(3);
    assert_eq!(name(&h).as_deref(), Some("BSX"));
}

// ---- v2 A19: export, placement check, guard and pending list ----

use sdc_gui::model::meshes::export::*;

fn mod_row(name: &str, prio: i32, derived: bool) -> modscan::store::ModRow {
    modscan::store::ModRow {
        id: prio as i64,
        name: name.into(),
        kind: modscan::store::KIND_MOD,
        priority: prio,
        derived,
        nifs: 1,
    }
}

#[test]
fn placement_rules() {
    let mods = [
        mod_row("PGP", 9, true),
        mod_row("B", 7, false),
        mod_row(EDIT_MOD, 5, false),
        mod_row("A", 3, false),
    ];
    let ok = [
        copy("PGP", 9, true),
        copy(EDIT_MOD, 5, false),
        copy("A", 3, false),
    ];
    assert_eq!(placement(&ok, &mods, EDIT_MOD), Placement::Ok);
    let below = [
        copy("PGP", 9, true),
        copy("B", 7, false),
        copy("A", 3, false),
    ];
    assert_eq!(
        placement(&below, &mods, EDIT_MOD),
        Placement::Below("B".into())
    );
    let low_pg = [mod_row("PGP", 4, true), mod_row(EDIT_MOD, 5, false)];
    let c = [copy("PGP", 4, true), copy("A", 3, false)];
    assert_eq!(
        placement(&c, &low_pg, EDIT_MOD),
        Placement::AbovePg("PGP".into())
    );
    assert_eq!(placement(&ok, &mods[..2], EDIT_MOD), Placement::NotEnabled);
    assert_eq!(Placement::Ok.text(EDIT_MOD), "Placement OK.");
    assert_eq!(
        Placement::NotEnabled.text(EDIT_MOD),
        "The edit mod is not enabled: enable 'sdc NIF Edits' in MO2."
    );
    assert_eq!(
        Placement::NotScanned.text(EDIT_MOD),
        "Rescan to check where 'sdc NIF Edits' sits."
    );
    assert_eq!(
        Placement::Below("B".into()).text(EDIT_MOD),
        "The edit mod is below B: your edit loses. Drag 'sdc NIF Edits' below B in MO2."
    );
    assert_eq!(
        Placement::AbovePg("PGP".into()).text(EDIT_MOD),
        "The edit mod is above PGP: PGPatcher output will not include your edit until you drag 'sdc NIF Edits' above PGP in MO2."
    );
}

/// `files(false)` plus A's y.nif, with `sdc NIF Edits` (an empty folder) between PGP and A; scanned, x.nif selected.
fn export_session() -> (tempfile::TempDir, MeshesModel) {
    let d = files_with(false, "+PGP\n+sdc NIF Edits\n+A\n");
    std::fs::create_dir_all(d.path().join("mods").join(EDIT_MOD)).unwrap();
    put(d.path(), "mods/A/meshes/y.nif", None);
    let mut m = MeshesModel::new(Some(d.path().join("scan.db")), MeshesSettings::default());
    m.procs = sdc_gui::model::meshes::no_procs;
    scan_and_select(&mut m, d.path());
    let x = m.nifs.iter().find(|n| n.path.ends_with("x.nif")).unwrap();
    m.select_nif(x.copy_id);
    (d, m)
}

fn exported(d: &tempfile::TempDir) -> std::path::PathBuf {
    d.path()
        .join("mods")
        .join(EDIT_MOD)
        .join("meshes")
        .join("x.nif")
}

/// Review's Export of the NIF rows: the only export path (Note GP10).
fn export(m: &mut MeshesModel) -> Result<usize, String> {
    m.export_nif_edits()
}

/// The export skips, naming `why`.
fn export_fails(m: &mut MeshesModel, why: &str) {
    let e = export(m).unwrap_err();
    assert!(e.contains(why), "{e}");
}

/// Answers the one Review row's foreign file with `choice`, for the bytes on disk now.
fn answer(m: &mut MeshesModel, choice: GuardChoice) {
    let r = m.nif_review_rows().remove(0);
    m.set_nif_answer(&r.path, choice, r.foreign.unwrap());
}

/// Sets the first lit shape's glossiness to `v` (one op).
fn gloss(m: &mut MeshesModel, v: f32) {
    let e = m.edit.as_ref().unwrap();
    let (shape, shader) = lit_shape(e);
    let cur = e.model.lighting[&shader].lighting.params.glossiness;
    let op = Op::SetField {
        at: nif::edit::anchor_role(&e.model, Some(shape), Role::Shader).unwrap(),
        field: Field::Glossiness,
        value: Value::F32(v.to_bits()),
        base: Value::F32(cur.to_bits()),
    };
    m.edit_push(vec![op], None);
}

#[test]
fn export_writes_the_patched_nif_and_records_its_hash() {
    let (d, mut m) = export_session();
    m.open_edit();
    assert_eq!(m.error, None);
    assert_eq!(
        m.edit.as_ref().unwrap().placement,
        Some(Placement::Ok),
        "the edit mod sits between A and PGP"
    );
    gloss(&mut m, 12.5);
    assert_eq!(export(&mut m), Ok(1));
    let e = m.edit.as_ref().unwrap();
    assert_eq!(
        e.export_note,
        Some((RERUN_PGPATCHER.into(), false)),
        "PGP is a derived copy"
    );
    let want = nif::edit::apply(&e.source, &e.doc.ops()).unwrap().bytes;
    assert_ne!(want, *e.source);
    assert_eq!(std::fs::read(exported(&d)).unwrap(), want);
    let saved = load(e.record_path.as_ref().unwrap()).unwrap().unwrap();
    assert_eq!(
        saved.export.sha256.as_deref(),
        Some(sha256_hex(&want).as_str())
    );
    assert!(saved.export.at.is_some());
    assert!(!saved.pending());
    assert!(!e.doc.pending());
}

/// Adds a float controller on the first lit shape's shader (`remove`: and removes it again, leaving three orphans).
fn add_then_remove_ctrl(m: &mut MeshesModel, remove: bool) {
    const LSCF: &str = "BSLightingShaderPropertyFloatController";
    let e = m.edit.as_ref().unwrap();
    let (shape, _) = lit_shape(e);
    let key = |time, value| Key {
        time,
        value,
        forward: None,
        backward: None,
        tbc: None,
    };
    let add = Op::AddController {
        at: nif::edit::anchor_role(&e.model, Some(shape), Role::Shader).unwrap(),
        class: LSCF.into(),
        var: Some(11),
        keys: KeyData::Float(KeyGroup {
            key_type: Some(1),
            keys: vec![key(0.0, 1.0), key(1.0, 2.0)],
        }),
        flags: 0x48,
        freq: 1.0,
        start: 0.0,
        stop: 1.0,
    };
    m.edit_push(vec![add], None);
    if remove {
        let e = m.edit.as_ref().unwrap();
        let (shape, shader) = lit_shape(e);
        let nth = e.model.lighting[&shader]
            .prop
            .ctrls
            .iter()
            .filter(|c| &*e.model.ctrls[c].class == LSCF && e.model.ctrls[c].var == Some(11))
            .count() as u32
            - 1;
        let role = Role::Ctrl {
            owner: nif::edit::Owner::Shader,
            class: LSCF.into(),
            var: Some(11),
            nth,
        };
        let at = nif::edit::anchor_role(&e.model, Some(shape), role).unwrap();
        m.edit_push(vec![Op::RemoveController { at }], None);
    }
}

#[test]
fn export_drops_orphan_blocks_and_records_the_written_hash() {
    let (d, mut m) = export_session();
    m.open_edit();
    add_then_remove_ctrl(&mut m, true);
    assert_eq!(m.edit.as_ref().unwrap().blocked, None);
    assert_eq!(export(&mut m), Ok(1));
    let e = m.edit.as_ref().unwrap();
    let n = nif::header::Header::parse(&e.source).unwrap().sizes.len();
    let blocks = |b: &[u8]| nif::header::Header::parse(b).unwrap().sizes.len();
    let written = std::fs::read(exported(&d)).unwrap();
    assert!(blocks(&e.patched) >= n + 3, "the session keeps the orphans");
    assert!(
        blocks(&written) <= n + 1,
        "at most the root's new BSXFlags stays"
    );
    assert_eq!(
        written,
        nif::edit::compact(&e.patched, n as u32).unwrap(),
        "the export is the compacted patch"
    );
    assert_eq!(
        load(e.record_path.as_ref().unwrap())
            .unwrap()
            .unwrap()
            .export
            .sha256
            .as_deref(),
        Some(sha256_hex(&written).as_str())
    );
    assert_eq!(e.cleanup_note, None);
    // the recorded hash is the file's: the next export replaces it without asking (and clears an old cleanup line)
    add_then_remove_ctrl(&mut m, false);
    m.edit.as_mut().unwrap().cleanup_note = Some("Exported without cleanup: x.".into());
    assert_eq!(export(&mut m), Ok(1));
    assert_eq!(m.edit.as_ref().unwrap().cleanup_note, None);
    let e = m.edit.as_ref().unwrap();
    let written = std::fs::read(exported(&d)).unwrap();
    assert_eq!(written, nif::edit::compact(&e.patched, n as u32).unwrap());
    assert!(blocks(&written) < blocks(&e.patched));
}

/// Note CQ5: when the cleanup cannot read the file, the export writes the patch as it is and says why.
#[test]
fn export_without_cleanup_writes_the_patch_and_says_why() {
    // scenes::animated(Some(1), 0) plus a source NiFloatData (4) nothing names, whose key count runs past it
    let mut b = builder::Nif::new(100);
    let (r, x, s) = (b.string("Root"), b.string("BSX"), b.string("S"));
    let net = builder::W::default().i32(r).u32(1).i32(1).i32(-1);
    b.block("NiNode", scenes::avobj(net).u32(1).i32(2).u32(0).0);
    b.block("BSXFlags", builder::W::default().i32(x).u32(1).0);
    b.block("BSTriShape", scenes::bstri_named(s, 3, -1));
    b.block(
        "BSLightingShaderProperty",
        scenes::lsp_body(0, &[], -1, &[]),
    );
    b.block("NiFloatData", builder::W::default().u32(u32::MAX).u32(1).0);
    let src = b.bytes();
    let d = files_with(false, "+PGP\n+sdc NIF Edits\n+A\n");
    std::fs::create_dir_all(d.path().join("mods").join(EDIT_MOD)).unwrap();
    put(d.path(), "mods/A/meshes/x.nif", Some(&src));
    let mut m = MeshesModel::new(Some(d.path().join("scan.db")), MeshesSettings::default());
    m.procs = sdc_gui::model::meshes::no_procs;
    scan_and_select(&mut m, d.path());
    m.open_edit();
    assert_eq!(m.error, None);
    add_then_remove_ctrl(&mut m, true);
    // Note GD2: known before the export, an Info row in plain words; the Review row has no count
    use sdc_gui::model::meshes::{NifLevel, NifTab};
    let info: Vec<_> = m
        .nif_problems()
        .iter()
        .filter(|p| p.level == NifLevel::Info)
        .map(|p| (p.text.clone(), p.tab))
        .collect();
    assert_eq!(info, [(NO_CLEANUP.to_string(), NifTab::Controllers)]);
    assert_eq!(review_row(&m).compaction, None);
    assert_eq!(export(&mut m), Ok(1));
    let e = m.edit.as_ref().unwrap();
    let why = nif::edit::compact(&e.patched, 5).unwrap_err();
    assert_eq!(std::fs::read(exported(&d)).unwrap(), *e.patched);
    assert_eq!(
        load(e.record_path.as_ref().unwrap())
            .unwrap()
            .unwrap()
            .export
            .sha256
            .as_deref(),
        Some(sha256_hex(&e.patched).as_str())
    );
    let line = format!("Exported without cleanup: {why}.");
    assert_eq!(e.cleanup_note.as_deref(), Some(line.as_str()));
    // Note GD2: CQ5's under-button line is gone; the Info row above says it
    let mut h = view(m);
    h.run_steps(4);
    assert!(h.query_by_label(&line).is_none());
}

#[test]
fn hand_edited_export_asks_before_overwrite() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    assert_eq!(export(&mut m), Ok(1));
    let p = exported(&d);
    std::fs::write(&p, b"junk").unwrap();
    // ruling E6-S (was "nothing pending: no row", Ok(0)): Export reads the installed NIF then; it is not the recorded
    // one (E6-N drift), so the row is pending and asks before anything is written
    export_fails(&mut m, NIF_UNANSWERED);
    assert_eq!(std::fs::read(&p).unwrap(), b"junk");
    gloss(&mut m, 13.5);
    // Unanswered, a hand-edited file is not written; its row has the block diff.
    export_fails(&mut m, NIF_UNANSWERED);
    let diff = m
        .nif_diff("meshes/x.nif")
        .expect("a foreign file has a diff");
    assert!(!diff.is_empty());
    assert_eq!(std::fs::read(&p).unwrap(), b"junk");
    answer(&mut m, GuardChoice::KeepTheirs);
    export_fails(&mut m, NIF_KEPT);
    assert_eq!(std::fs::read(&p).unwrap(), b"junk");
    export_fails(&mut m, NIF_UNANSWERED);
    answer(&mut m, GuardChoice::Overwrite);
    assert_eq!(export(&mut m), Ok(1));
    let e = m.edit.as_ref().unwrap();
    assert_eq!(std::fs::read(&p).unwrap(), *e.patched);
}

/// Review Focus 1: an Overwrite answered for bytes that changed on disk since is not applied.
#[test]
fn an_overwrite_answer_for_older_bytes_does_not_write() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    let p = exported(&d);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, b"junk").unwrap();
    answer(&mut m, GuardChoice::Overwrite);
    std::fs::write(&p, b"changed").unwrap();
    export_fails(&mut m, NIF_CHANGED_SINCE);
    assert_eq!(std::fs::read(&p).unwrap(), b"changed");
}

/// Note GP10, ruling E6-S2 (was: a file equal to the export when the rows were read is not foreign, and the write
/// found it foreign once a source change re-applied the ops): a present NIF the record does not name is foreign even
/// when its bytes equal the export's, so the row asks before anything is written, whatever the source does.
#[test]
fn a_file_that_turns_foreign_at_the_write_is_not_written() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    let e = m.edit.as_ref().unwrap();
    let theirs = e.out.to_vec();
    let p = exported(&d);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, &theirs).unwrap();
    // the source's specular strength changes on disk: no conflict with the glossiness edit, but other bytes
    let (shape, shader) = lit_shape(e);
    let cur = e.model.lighting[&shader].lighting.params.specular_strength;
    let src = nif::edit::apply(
        &e.source,
        &[Op::SetField {
            at: nif::edit::anchor_role(&e.model, Some(shape), Role::Shader).unwrap(),
            field: Field::SpecularStrength,
            value: Value::F32((cur + 1.0).to_bits()),
            base: Value::F32(cur.to_bits()),
        }],
    )
    .unwrap()
    .bytes;
    std::fs::write(d.path().join("mods/A/meshes/x.nif"), &src).unwrap();
    assert_eq!(
        review_row(&m).foreign,
        Some(sha(&theirs)),
        "the row asks for the unrecorded file, equal bytes or not"
    );
    export_fails(&mut m, NIF_UNANSWERED);
    assert_eq!(std::fs::read(&p).unwrap(), theirs);
}

/// Note GD2: an answer counts only while the file has the hash it was given for: Keep for a file that is then
/// deleted no longer applies, and the export writes.
#[test]
fn a_keep_answer_for_a_deleted_file_no_longer_applies() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    let p = exported(&d);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, b"junk").unwrap();
    answer(&mut m, GuardChoice::KeepTheirs);
    std::fs::remove_file(&p).unwrap();
    assert_eq!(export(&mut m), Ok(1));
    assert_eq!(std::fs::read(&p).unwrap(), *m.edit.as_ref().unwrap().out);
}

#[test]
fn conflicting_doc_blocks_export() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    // The source's own glossiness changes on disk to another value.
    let e = m.edit.as_ref().unwrap();
    let (shape, shader) = lit_shape(e);
    let src = nif::model::read_model(&e.source).unwrap();
    let cur = src.lighting[&shader].lighting.params.glossiness;
    assert_ne!(cur, 7.0);
    let theirs = nif::edit::apply(
        &e.source,
        &[Op::SetField {
            at: nif::edit::anchor_role(&e.model, Some(shape), Role::Shader).unwrap(),
            field: Field::Glossiness,
            value: Value::F32(7.0f32.to_bits()),
            base: Value::F32(cur.to_bits()),
        }],
    )
    .unwrap()
    .bytes;
    assert_ne!(theirs, *e.source);
    std::fs::write(d.path().join("mods/A/meshes/x.nif"), &theirs).unwrap();
    export_fails(&mut m, "Resolve the conflicts first.");
    assert!(!exported(&d).exists());
    let e = m.edit.as_ref().unwrap();
    assert_eq!(*e.source, theirs);
    assert!(e.doc.has_conflicts());
}

#[test]
fn a_refused_edit_blocks_export() {
    let (d, mut m) = export_session();
    m.open_edit();
    let e = m.edit.as_ref().unwrap();
    let (shape, shader) = lit_shape(e);
    let set = e.model.lighting[&shader].texture_set.unwrap();
    let base = String::from_utf8_lossy(&e.model.texsets[&set].raw.slots[0]).into_owned();
    let op = Op::SetTexturePath {
        at: nif::edit::anchor_shape(&e.model, shape),
        slot: 0,
        path: "textures\\\u{4e2d}.dds".into(),
        base,
    };
    m.edit_push(vec![op.clone()], None);
    assert!(matches!(
        m.edit.as_ref().unwrap().outcomes[0],
        nif::edit::Outcome::Refused(_)
    ));
    export_fails(&mut m, "1 edit cannot be applied.");
    m.edit_push(vec![op], None);
    export_fails(&mut m, "2 edits cannot be applied.");
    assert!(!exported(&d).exists());
}

/// An edited session whose patched bytes do not scan. No edit makes such bytes (`apply` refuses a source that does
/// not add up, and the scan fails on nothing else), so the test seam sets them as `reapply` would (Note GP8).
fn unreadable_patch(m: &mut MeshesModel) {
    m.open_edit();
    gloss(m, 12.5);
    let e = m.edit.as_mut().unwrap();
    let mut bad = (*e.patched).clone();
    bad.truncate(bad.len() - 3);
    e.set_patched(bad);
}

#[test]
fn export_block_still_names_bytes_that_do_not_read_back() {
    let (_d, mut m) = export_session();
    unreadable_patch(&mut m);
    let why = m.edit.as_ref().unwrap().export_block().unwrap_or_default();
    assert!(
        why.starts_with("The written file would not read back: "),
        "{why}"
    );
}

#[test]
fn one_push_scans_the_patched_bytes_once() {
    let (_d, mut m) = export_session();
    m.open_edit();
    let n = scan_count();
    gloss(&mut m, 12.5);
    assert_eq!(scan_count() - n, 1);
}

#[test]
fn one_push_checks_the_export_block_once() {
    let (_d, mut m) = export_session();
    m.open_edit();
    assert!(!m.edit.as_ref().unwrap().check_exists);
    let n = block_check_count();
    gloss(&mut m, 12.5);
    assert_eq!(block_check_count() - n, 1);
}

/// Note GP8: turning "Check that files exist" off changes the warnings without a new `rev`: its rows still go.
#[test]
fn turning_the_file_check_off_drops_its_rows() {
    let (_d, mut m) = export_session();
    m.open_edit();
    let rows = |m: &MeshesModel| m.edit.as_ref().unwrap().problems.len();
    let n = rows(&m);
    m.set_edit_check_exists(true);
    assert!(rows(&m) > n, "a missing file is a row");
    m.set_edit_check_exists(false);
    assert_eq!(rows(&m), n);
}

#[test]
fn export_refuses_bytes_that_do_not_read_back() {
    let (d, mut m) = export_session();
    unreadable_patch(&mut m);
    export_fails(&mut m, ": The written file would not read back: ");
    assert!(!exported(&d).exists());
}

#[test]
fn an_unreadable_export_file_blocks_and_writes_nothing() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    std::fs::create_dir_all(exported(&d)).unwrap();
    export_fails(&mut m, ": Cannot read ");
    assert!(exported(&d).is_dir());
    let doc = load(&m.edit.as_ref().unwrap().draft_path).unwrap().unwrap();
    assert!(doc.export.sha256.is_none());
    assert!(
        !nif_doc(&export_root(&d)).exists(),
        "no record is written either"
    );
    assert_eq!(m.edit.as_ref().unwrap().doc.export.sha256, None);
}

/// The chip the export folder's records give a NIF (`pending_docs` walks `.sdc\nif-edits`), as a fresh start sees it.
fn badge_of(m: &MeshesModel, name: &str) -> Option<DocBadge> {
    m.pending_docs()
        .into_iter()
        .find(|(k, _)| k.ends_with(name))
        .map(|(_, b)| b)
}

/// The chip the session keeps current in memory (`doc_badges`): it follows the draft, which is not in the export folder.
fn chip_of(m: &MeshesModel, name: &str) -> Option<DocBadge> {
    m.doc_badges
        .iter()
        .find(|(k, _)| k.ends_with(name))
        .map(|(_, b)| *b)
}

#[test]
fn pending_list_and_chips() {
    let (d, mut m) = export_session();
    let y = m.nifs.iter().find(|n| n.path.ends_with("y.nif")).unwrap();
    m.select_nif(y.copy_id);
    m.open_edit();
    gloss(&mut m, 30.5);
    let x = m.nifs.iter().find(|n| n.path.ends_with("x.nif")).unwrap();
    m.select_nif(x.copy_id);
    m.open_edit();
    gloss(&mut m, 12.5);
    assert_eq!(export(&mut m), Ok(1));
    assert_eq!(badge_of(&m, "x.nif"), Some(DocBadge::Exported));
    assert_eq!(chip_of(&m, "y.nif"), Some(DocBadge::Pending));
    assert_eq!(
        badge_of(&m, "y.nif"),
        None,
        "Review E6: an edit not exported has no record in the export folder"
    );
    // A new session on the same store reads the chips from the records, without a scan or an open edit.
    let mut fresh = MeshesModel::new(
        Some(d.path().join("scan.db")),
        MeshesSettings {
            instance: Some(d.path().to_path_buf()),
            profile: Some("Default".into()),
            folder: None,
        },
    );
    fresh.procs = sdc_gui::model::meshes::no_procs;
    fresh.activate_with(None);
    // the app sets the chosen export folder on its first frame (Note GD5)
    fresh.set_nif_export_root(Some(d.path().join("mods").join(EDIT_MOD)));
    let chips = |k: &str| {
        fresh
            .doc_badges
            .iter()
            .find(|(p, _)| p.ends_with(k))
            .map(|(_, b)| *b)
    };
    assert_eq!(chips("x.nif"), Some(DocBadge::Exported));
    assert_eq!(chips("y.nif"), None, "a draft is read when its NIF opens");
    // Meshes UI T2: the column is locked on the shapes while Edit is open; the chips are in the NIF list.
    m.close_edit();
    let mut h = view(m);
    h.run_steps(4);
    h.get_by_label("Edited");
    h.get_by_label("Edit pending");
    assert!(h.query_by_label("Edit conflict").is_none());
}

#[test]
fn undo_after_export_makes_it_pending_again() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    assert_eq!(export(&mut m), Ok(1));
    assert_eq!(badge_of(&m, "x.nif"), Some(DocBadge::Exported));
    assert!(m.undo_edit());
    let e = m.edit.as_ref().unwrap();
    assert!(e.doc.pending(), "the exported file still has the edit");
    assert!(
        e.doc.export.sha256.is_some(),
        "undo keeps the export record"
    );
    assert_eq!(chip_of(&m, "x.nif"), Some(DocBadge::Pending));
    assert_eq!(
        badge_of(&m, "x.nif"),
        Some(DocBadge::Exported),
        "the record in the export folder is what the last export wrote"
    );
    // Our own file is replaced without asking.
    assert_eq!(export(&mut m), Ok(1));
    let e = m.edit.as_ref().unwrap();
    assert_eq!(std::fs::read(exported(&d)).unwrap(), *e.source);
}

/// The open session's source with its specular strength raised: an unrelated change that leaves the glossiness edit
/// applied and the block count the same.
fn changed_source(m: &MeshesModel) -> Vec<u8> {
    let e = m.edit.as_ref().unwrap();
    let (shape, shader) = lit_shape(e);
    let cur = e.model.lighting[&shader].lighting.params.specular_strength;
    nif::edit::apply(
        &e.source,
        &[Op::SetField {
            at: nif::edit::anchor_role(&e.model, Some(shape), Role::Shader).unwrap(),
            field: Field::SpecularStrength,
            value: Value::F32((cur + 1.0).to_bits()),
            base: Value::F32(cur.to_bits()),
        }],
    )
    .unwrap()
    .bytes
}

/// Note GQ1: the exported file was built from the old source, so a source change makes the edit pending again.
#[test]
fn a_source_change_after_export_makes_it_pending_again() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    assert_eq!(export(&mut m), Ok(1));
    assert_eq!(badge_of(&m, "x.nif"), Some(DocBadge::Exported));
    let src = changed_source(&m);
    let record = std::fs::read(nif_doc(&export_root(&d))).unwrap();
    // the export re-reads the source copy: the change is on disk too
    std::fs::write(d.path().join("mods/A/meshes/x.nif"), &src).unwrap();
    m.edit_with(|e| e.reload_source(src.clone()).unwrap());
    let e = m.edit.as_ref().unwrap();
    assert!(e.doc.ops.iter().all(|o| o.status == OpStatus::Applied));
    assert!(e.pending());
    assert!(e.doc.export.ops_hash.is_some(), "not None: the PGP warning");
    // ruling E6-A: the forced re-export is the session's (its chip), the record is what the last export wrote
    assert_eq!(chip_of(&m, "x.nif"), Some(DocBadge::Pending));
    assert_eq!(std::fs::read(nif_doc(&export_root(&d))).unwrap(), record);
    assert_eq!(export(&mut m), Ok(1));
    let e = m.edit.as_ref().unwrap();
    assert_eq!(*e.source, src);
    assert_eq!(std::fs::read(exported(&d)).unwrap(), *e.out);
    assert!(!e.pending(), "the export records a real hash again");
}

/// Review round 2 (B3 display, ruling E6-J): a draft from source S1 opened and exported against S2 is current: the
/// record names S2, so reopening (the draft still names S1) does not make it pending again.
#[test]
fn an_export_on_a_changed_source_is_current_after_a_reopen() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    let src = changed_source(&m);
    m.close_edit();
    std::fs::write(d.path().join("mods/A/meshes/x.nif"), &src).unwrap();
    m.open_edit();
    assert_eq!(export(&mut m), Ok(1));
    assert!(!m.edit.as_ref().unwrap().pending(), "premise: exported");
    m.close_edit();
    m.open_edit();
    let e = m.edit.as_ref().unwrap();
    assert_eq!(*e.source, src);
    assert!(!e.pending(), "the record was exported from this source");
    assert_ne!(chip_of(&m, "x.nif"), Some(DocBadge::Pending));
}

/// Review round 2 (B3, ruling E6-J): an op that open re-anchors leaves the session unsaved, so Export saves the draft
/// (with the block it applies to now) before the NIF.
#[test]
fn an_op_reanchored_at_open_is_saved_by_the_export() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    m.close_edit();
    let mut doc = load(&autosave(&d)).unwrap().unwrap();
    let Op::SetField { at, .. } = &mut doc.ops[0].op else {
        panic!("{:?}", doc.ops[0].op)
    };
    let right = at.block;
    at.block = 9999; // the recorded index is gone: the one block of that role is found again
    save(&autosave(&d), &doc).unwrap();
    m.open_edit();
    assert_eq!(op_values(&m)[0].1, 12.5, "premise: the op applies");
    assert_eq!(export(&mut m), Ok(1));
    let Op::SetField { at, .. } = &load(&autosave(&d)).unwrap().unwrap().ops[0].op else {
        unreachable!()
    };
    assert_eq!(
        at.block, right,
        "the draft holds the block the NIF was written with"
    );
}

/// The same when the source changed while the edit was closed.
#[test]
fn reopening_an_exported_edit_with_a_changed_source_is_pending() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    assert_eq!(export(&mut m), Ok(1));
    let src = changed_source(&m);
    m.close_edit();
    let record = std::fs::read(nif_doc(&export_root(&d))).unwrap();
    std::fs::write(d.path().join("mods/A/meshes/x.nif"), &src).unwrap();
    m.open_edit();
    let e = m.edit.as_ref().unwrap();
    assert_eq!(*e.source, src);
    assert!(e.doc.ops.iter().all(|o| o.status == OpStatus::Applied));
    assert!(e.pending());
    assert!(e.doc.export.ops_hash.is_some(), "not None: the PGP warning");
    assert_eq!(chip_of(&m, "x.nif"), Some(DocBadge::Pending));
    // ruling E6-A: the open wrote nothing in the export folder; the record's source is what makes it pending
    assert_eq!(std::fs::read(nif_doc(&export_root(&d))).unwrap(), record);
    // closed again before any export: the next open works the forced re-export out again
    m.close_edit();
    assert_eq!(std::fs::read(nif_doc(&export_root(&d))).unwrap(), record);
    m.open_edit();
    assert!(m.edit.as_ref().unwrap().pending());
    assert_eq!(export(&mut m), Ok(1));
    assert_eq!(
        std::fs::read(exported(&d)).unwrap(),
        *m.edit.as_ref().unwrap().out
    );
}

/// Note GQ1: only an exported session is saved at open; a never-exported one keeps its document until the next save.
#[test]
fn reopening_a_never_exported_edit_with_a_changed_source_writes_no_document() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    let doc = m.edit.as_ref().unwrap().draft_path.clone();
    let before = std::fs::read_to_string(&doc).unwrap();
    let src = changed_source(&m);
    m.close_edit();
    std::fs::write(d.path().join("mods/A/meshes/x.nif"), &src).unwrap();
    m.open_edit();
    assert_eq!(*m.edit.as_ref().unwrap().source, src);
    assert_eq!(
        std::fs::read_to_string(&doc).unwrap(),
        before,
        "no write at open"
    );
}

/// Review Focus 1: only a changed source forces the re-export.
#[test]
fn reopening_an_exported_edit_with_an_unchanged_source_is_not_pending() {
    let (_d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    assert_eq!(export(&mut m), Ok(1));
    m.close_edit();
    m.open_edit();
    assert!(!m.edit.as_ref().unwrap().pending());
    assert_eq!(badge_of(&m, "x.nif"), Some(DocBadge::Exported));
}

/// Note GD2: the footer keeps its status and destination lines; its button opens Review (the NIF edits export there,
/// with the textures) and writes nothing. The placement and Re-run lines are Review's and the worklist's now.
#[test]
fn the_footer_opens_review_and_writes_nothing() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    let Op::SetField { at, .. } = &ops(&m)[0] else {
        unreachable!()
    };
    let revert = format!("Revert 1: {}: Glossiness", at.shape.as_deref().unwrap());
    let mut h = view(m);
    h.run_steps(3);
    h.get_by_label_contains(&writes_line(&d));
    h.get_by_label("Ready");
    assert!(h.query(export_nif()).is_none(), "no Export NIF button");
    assert!(
        h.query_by_label("Placement OK.").is_none(),
        "placement is on the Review row"
    );
    // Meshes UI T3: the pending list is closed while no row is red.
    assert!(h.query_by_label(&revert).is_none());
    h.get_by_label("Pending edits (1)").click();
    h.run_steps(3);
    h.get_by_role_and_label(AkRole::Button, &revert);
    assert!(!h.state().open_review_request);
    h.get(review_button()).click();
    h.run_steps(3);
    assert!(
        h.state().open_review_request,
        "Review… asks the app to open Review"
    );
    assert!(!exported(&d).exists(), "the click writes nothing");
}

/// Meshes UI T3: the retired Export NIF button (its name was its text); Note GD2 checks it is gone.
fn export_nif() -> egui_kittest::kittest::By<'static> {
    egui_kittest::kittest::By::new()
        .role(AkRole::Button)
        .label_contains("Export NIF")
}

/// Note GD2: the footer's Review… button.
fn review_button() -> egui_kittest::kittest::By<'static> {
    egui_kittest::kittest::By::new()
        .role(AkRole::Button)
        .label("Review…")
}

/// Meshes UI T3: the export block's destination line ("Writes … to …"), whole in its accessible name.
fn writes_line(d: &tempfile::TempDir) -> String {
    format!("Writes meshes\\x.nif to {}", exported(d).display())
}

/// A pending-list row (a label; its Revert button carries the same words).
fn row(text: &str) -> egui_kittest::kittest::By<'_> {
    egui_kittest::kittest::By::new()
        .role(AkRole::Label)
        .label_contains(text)
}

/// Meshes UI T3: a red row opens "Pending edits (N)" by itself, the block says why it is blocked, and the row is
/// named in words, not as a Rust field (`Slsf1`).
#[test]
fn a_red_row_opens_the_pending_list_and_rows_read_as_words() {
    let (_d, mut m) = export_session();
    m.open_edit();
    let e = m.edit.as_ref().unwrap();
    let (shape, shader) = lit_shape(e);
    let f1 = e.model.lighting[&shader].lighting.slsf1;
    let at = nif::edit::anchor_role(&e.model, Some(shape), Role::Shader).unwrap();
    let set = e.model.lighting[&shader].texture_set.unwrap();
    let base = String::from_utf8_lossy(&e.model.texsets[&set].raw.slots[0]).into_owned();
    let on_shape = nif::edit::anchor_shape(&e.model, shape);
    m.edit_push(
        vec![Op::SetBits {
            at,
            field: Field::Slsf1,
            mask: 1 << 12,
            value: !f1 & (1 << 12),
            base: f1 & (1 << 12),
        }],
        None,
    );
    // A path outside Windows-1252 is refused: a red row.
    m.edit_push(
        vec![Op::SetTexturePath {
            at: on_shape,
            slot: 0,
            path: "textures\\\u{4e2d}.dds".into(),
            base,
        }],
        None,
    );
    let mut h = view(m);
    h.run_steps(3);
    h.get_by_label("Pending edits (2)");
    h.get_by_label("Blocked: 1 edit cannot be applied.");
    h.get(row(": Shader flags 1"));
    assert!(h.query_by_label_contains("Slsf1").is_none());
    h.get_by_label_contains(": Texture slot 1: ");
    // Closed by hand, Show opens it again.
    h.get_by_label("Pending edits (2)").click();
    h.run_steps(3);
    assert!(h.query(row(": Shader flags 1")).is_none());
    h.get_by_role_and_label(AkRole::Button, "Show pending edits")
        .click();
    h.run_steps(3);
    h.get(row(": Shader flags 1"));
}

/// Meshes UI T3: without an export folder Review… stays on. Note GH5: an untouched session reads the idle "No edits" line,
/// not a red block (an edit with no folder reads "Skipped until an export folder is chosen", pinned below).
#[test]
fn a_read_only_session_shows_the_no_export_folder_reason() {
    let m = direct(scenes::shared_alpha_and_set());
    let mut h = view(m);
    h.run_steps(3);
    h.get_by_label(NO_EDITS);
    assert!(h.query_by_label_contains("Blocked").is_none());
    // Note GD2: Review… stays on, so Review can show the reason on the NIF's row.
    assert!(!h.get(review_button()).accesskit_node().is_disabled());
}

// ---- v2 A19 review ----

/// Note GD2: the guard dialog is gone; a foreign file is answered on its Review row. After an unanswered export the
/// panel draws no dialog, and Enter writes nothing.
#[test]
fn a_foreign_file_opens_no_guard_dialog() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    let p = exported(&d);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, b"theirs").unwrap();
    export_fails(&mut m, NIF_UNANSWERED);
    let mut h = view(m);
    h.run_steps(3);
    assert!(h
        .query_by_label("A file sdc did not write is already at this path.")
        .is_none());
    assert!(h
        .query_by_role_and_label(AkRole::Button, "Overwrite")
        .is_none());
    assert!(h
        .query_by_role_and_label(AkRole::Button, "Keep theirs")
        .is_none());
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert_eq!(std::fs::read(&p).unwrap(), b"theirs");
}

#[test]
fn pending_walk_does_not_follow_directory_links() {
    let d = tempfile::tempdir().unwrap();
    let edits = d.path().join(".sdc").join("nif-edits");
    let sub = edits.join("meshes");
    std::fs::create_dir_all(&sub).unwrap();
    let mut doc = doc();
    doc.ops.push(DocOp {
        op: sample_op(),
        status: OpStatus::Applied,
    });
    save(&sub.join("x.nif.sdc-nifedit.toml"), &doc).unwrap();
    // A junction back to its own parent: a walk that follows it never ends (or lists the doc twice).
    let link = sub.join("loop");
    let ok = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(&edits)
        .output()
        .unwrap()
        .status
        .success();
    assert!(ok && link.is_dir(), "the junction was made");
    let found = pending_docs(d.path());
    assert_eq!(found, vec![("meshes/x.nif".to_string(), DocBadge::Pending)]);
    std::fs::remove_dir(&link).unwrap(); // the junction only, never its target
}

#[test]
fn a_path_typed_without_enter_is_committed_by_the_review_click() {
    let (d, mut m) = export_session();
    m.open_edit();
    let (shape, _) = lit_shape(m.edit.as_ref().unwrap());
    m.select_edit(Selection::Shape(shape));
    gloss(&mut m, 12.5);
    let mut h = view(m);
    h.run_steps(3);
    let field = "Diffuse path (slot 1)";
    h.get_by_role_and_label(AkRole::TextInput, field).focus();
    h.run_steps(1);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.run_steps(1);
    h.get_by_role_and_label(AkRole::TextInput, field)
        .type_text("textures\\typed.dds");
    h.run_steps(1);
    // Note GD2: the click that leaves the field is Review…'s; the export is Review's (here, the model call it makes).
    h.get(review_button()).click();
    h.run_steps(4);
    assert!(h.state().open_review_request);
    assert_eq!(h.state_mut().export_nif_edits(), Ok(1));
    let bytes = std::fs::read(exported(&d)).expect("exported");
    let needle = b"textures\\typed.dds";
    assert!(
        bytes.windows(needle.len()).any(|w| w == needle),
        "the typed path is in the exported file"
    );
    let e = h.state().edit.as_ref().unwrap();
    assert!(!e.doc.pending(), "the export includes the typed edit");
}

/// Types `path` into slot 1 of the open edit, without Enter: the field keeps the focus.
fn type_path(h: &mut Harness<'static, MeshesModel>, path: &str) {
    let field = "Diffuse path (slot 1)";
    h.get_by_role_and_label(AkRole::TextInput, field).focus();
    h.run_steps(1);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.run_steps(1);
    h.get_by_role_and_label(AkRole::TextInput, field)
        .type_text(path);
    h.run_steps(1);
    assert!(
        !ops(h.state())
            .iter()
            .any(|o| matches!(o, Op::SetTexturePath { path: p, .. } if p == path)),
        "the path is not committed before the click"
    );
}

fn has_path(m: &MeshesModel, path: &str) -> bool {
    ops(m)
        .iter()
        .any(|o| matches!(o, Op::SetTexturePath { path: p, .. } if p == path))
}

/// Note HS15: a tab click ends the typing too; the old tab is drawn once more, so its field commits.
#[test]
fn a_path_typed_without_enter_survives_a_tab_click() {
    let (_d, mut m) = export_session();
    m.open_edit();
    let (shape, _) = lit_shape(m.edit.as_ref().unwrap());
    m.select_edit(Selection::Shape(shape));
    gloss(&mut m, 12.5);
    let mut h = view(m);
    h.run_steps(3);
    type_path(&mut h, "textures\\typed.dds");
    h.get_by_label("Alpha tab").click();
    h.run_steps(4);
    assert!(
        has_path(h.state(), "textures\\typed.dds"),
        "{:?}",
        ops(h.state())
    );
}

/// Note HS15: a later control's click (here a flag's Fix) keeps the typed path as well as its own edit.
#[test]
fn a_path_typed_without_enter_survives_a_click_on_a_later_control() {
    let (_d, mut m) = export_session();
    m.open_edit();
    let (shape, shader) = lit_shape(m.edit.as_ref().unwrap());
    m.select_edit(Selection::Shape(shape));
    gloss(&mut m, 12.5);
    let e = m.edit.as_ref().unwrap();
    let lt = &e.model.lighting[&shader].lighting;
    assert_ne!(
        lt.shader_type, 5,
        "precondition: bit 21 is an error off Skin Tint"
    );
    let f1 = lt.slsf1;
    assert_eq!(f1 & (1 << 21), 0, "precondition: the skin tint flag is off");
    let at = nif::edit::anchor_role(&e.model, Some(shape), Role::Shader).unwrap();
    m.edit_push(
        vec![Op::SetBits {
            at,
            field: Field::Slsf1,
            mask: 1 << 21,
            value: 1 << 21,
            base: f1,
        }],
        None,
    );
    let mut h = view(m);
    h.run_steps(3);
    let clear = format!("Clear the flag: {SKIN_TINT} (Shader flags 1)");
    h.get_by_role_and_label(AkRole::Button, &clear)
        .scroll_to_me();
    h.run_steps(3);
    type_path(&mut h, "textures\\typed.dds");
    h.get_by_role_and_label(AkRole::Button, &clear).click();
    h.run_steps(4);
    let m = h.state();
    assert!(has_path(m, "textures\\typed.dds"), "{:?}", ops(m));
    assert!(
        ops(m).iter().any(|o| matches!(
            o,
            Op::SetBits { field: Field::Slsf1, mask, value: 0, .. } if *mask & (1 << 21) != 0
        )),
        "the flag's Fix is kept too: {:?}",
        ops(m)
    );
}

/// Note HS15 (pin: it passed before the fix too, and fails if the `same_slot` drop is removed): the same slot's Fix
/// clicked while a path is typed is dropped (the typed path wins), never a Conflict.
#[test]
fn a_same_slot_fix_while_a_path_is_typed_is_dropped() {
    let mut m = direct(scenes::shared_alpha_and_set());
    let a = select(&mut m, "A");
    let e = m.edit.as_ref().unwrap();
    let set = e.model.lighting[&e.model.shape(a).unwrap().shader.unwrap()]
        .texture_set
        .unwrap();
    let base = String::from_utf8_lossy(&e.model.texsets[&set].raw.slots[0]).into_owned();
    let at = nif::edit::anchor_shape(&e.model, a);
    m.edit_push(
        vec![Op::SetTexturePath {
            at,
            slot: 0,
            path: r"C:\Game\Data\textures\a.dds".into(),
            base,
        }],
        None,
    );
    let mut h = view(m);
    h.run_steps(2);
    type_path(&mut h, "textures\\typed.dds");
    h.get(
        egui_kittest::kittest::By::new()
            .role(AkRole::Button)
            .label_contains("Fix slot 1: "),
    )
    .click();
    h.run_steps(4);
    let m = h.state();
    assert!(has_path(m, "textures\\typed.dds"), "{:?}", ops(m));
    let slot0 = ops(m)
        .iter()
        .filter(|o| matches!(o, Op::SetTexturePath { slot: 0, .. }))
        .count();
    assert_eq!(
        slot0,
        2,
        "the setup edit and the typed path only: {:?}",
        ops(m)
    );
    assert!(!m.edit.as_ref().unwrap().doc.has_conflicts());
}

// ---- v2 A20: the shared Undo / Redo act on the NIF history while an edit is open in the Meshes view ----

/// One app-data redirect for every `SdcApp` test of this binary (as meshes_header.rs): the variables are process-wide,
/// so a test's own temp dir could be deleted under another test.
fn app_data() {
    static D: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    D.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("appdata");
        std::fs::create_dir_all(&a).unwrap();
        std::env::set_var("LOCALAPPDATA", &a);
        std::env::set_var("APPDATA", &a);
        d
    });
}

type AppH = Harness<'static, sdc_gui::app::SdcApp>;

/// The app in the Meshes view with a Textures history step and an open NIF edit with `ops` NIF undo steps.
fn both_histories(ops: usize) -> (tempfile::TempDir, tempfile::TempDir, AppH) {
    app_data();
    let t = tempfile::tempdir().unwrap();
    let model = support::pbr_opened(&t.path().join("mod"));
    assert!(model.can_undo() && !model.can_redo());
    let (d, h) = in_the_meshes_view(model, ops);
    (t, d, h)
}

fn in_the_meshes_view(model: sdc_gui::model::AppModel, ops: usize) -> (tempfile::TempDir, AppH) {
    in_the_meshes_view_sized(model, ops, egui::vec2(1400.0, 900.0))
}

fn in_the_meshes_view_sized(
    model: sdc_gui::model::AppModel,
    ops: usize,
    size: egui::Vec2,
) -> (tempfile::TempDir, AppH) {
    let (d, mut h) = in_the_meshes_view_steps(model, ops, size);
    h.run_steps(4);
    (d, h)
}

/// As `in_the_meshes_view_sized`, but no frame has run since the Meshes view was switched on.
fn in_the_meshes_view_steps(
    model: sdc_gui::model::AppModel,
    ops: usize,
    size: egui::Vec2,
) -> (tempfile::TempDir, AppH) {
    let d = files(false);
    let mut app = sdc_gui::app::SdcApp::new(model);
    app.meshes.procs = sdc_gui::model::meshes::no_procs;
    app.meshes.store_path = Some(d.path().join("scan.db"));
    scan_and_select(&mut app.meshes, d.path());
    // As the app's first frame sets it: the edit opens in the Textures export folder (Note CI29).
    let root = app.model.nif_export_root();
    app.meshes.set_nif_export_root(root);
    app.meshes.open_edit();
    assert_eq!(app.meshes.error, None);
    for i in 0..ops {
        tick(&mut app.meshes, 40.5 + i as f32);
        app.meshes.edit.as_mut().unwrap().flush();
    }
    let mut frame = eframe::Frame::_new_kittest();
    let h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(size)
        .build_ui_state(
            move |ui, app: &mut sdc_gui::app::SdcApp| eframe::App::ui(app, ui, &mut frame),
            app,
        );
    sdc_gui::ui::header::set_meshes_view(&h.ctx, true);
    (d, h)
}

/// Beyond the brief's list: with no Textures history, Ctrl+Z and the header still undo the NIF step.
#[test]
fn ctrl_z_undoes_a_nif_edit_with_an_empty_textures_history() {
    app_data();
    let t = tempfile::tempdir().unwrap();
    let model = support::opened(&t.path().join("mod"));
    assert!(!model.can_undo());
    let (_d, mut h) = in_the_meshes_view(model, 2);
    assert!(!support::header_undo(&h, "Undo")
        .accesskit_node()
        .is_disabled());
    press(&mut h, egui::Modifiers::COMMAND, egui::Key::Z);
    assert_eq!(nif(&h), (1, true));
    support::header_undo(&h, "Undo").click();
    h.run_steps(4);
    assert_eq!(nif(&h), (0, true));
    assert!(!h.state().model.can_undo() && !h.state().model.can_redo());
}

/// (NIF ops, NIF can redo).
fn nif(h: &AppH) -> (usize, bool) {
    let e = h.state().meshes.edit.as_ref().unwrap();
    (e.doc.ops.len(), e.history.can_redo())
}

/// (can undo, can redo, the undo label) of the Textures history.
fn textures(h: &AppH) -> (bool, bool, Option<String>) {
    let m = &h.state().model;
    (m.can_undo(), m.can_redo(), m.undo_label().map(String::from))
}

fn press(h: &mut AppH, mods: egui::Modifiers, key: egui::Key) {
    h.key_press_modifiers(mods, key);
    h.run_steps(4);
}

/// Closes the edit: the shared Undo / Redo go back to the Textures history.
fn close(h: &mut AppH) {
    h.state_mut().meshes.close_edit();
    h.run_steps(4);
}

#[test]
fn header_undo_follows_the_nif_history_while_editing() {
    let (_t, _d, mut h) = both_histories(1);
    let tex = textures(&h);
    support::header_undo(&h, "Undo").click();
    h.run_steps(4);
    assert_eq!(nif(&h), (0, true), "the NIF op is undone");
    assert_eq!(textures(&h), tex, "the Textures history is untouched");
    support::header_undo(&h, "Redo").click();
    h.run_steps(4);
    assert_eq!(nif(&h), (1, false));
    assert_eq!(textures(&h), tex);
    close(&mut h);
    support::header_undo(&h, "Undo").click();
    h.run_steps(4);
    assert!(
        h.state().model.can_redo(),
        "without an edit Undo is the Textures one"
    );
}

#[test]
fn ctrl_z_and_ctrl_y_follow_the_nif_history_while_editing() {
    use egui::{Key, Modifiers};
    let (_t, _d, mut h) = both_histories(2);
    let tex = textures(&h);
    press(&mut h, Modifiers::COMMAND, Key::Z);
    assert_eq!(nif(&h), (1, true));
    press(&mut h, Modifiers::COMMAND, Key::Z);
    assert_eq!(nif(&h), (0, true));
    press(&mut h, Modifiers::COMMAND, Key::Y);
    assert_eq!(nif(&h), (1, true));
    press(&mut h, Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
    assert_eq!(nif(&h), (2, false));
    assert_eq!(textures(&h), tex, "the Textures history is untouched");
    close(&mut h);
    press(&mut h, Modifiers::COMMAND, Key::Z);
    assert!(
        h.state().model.can_redo(),
        "without an edit Ctrl+Z is the Textures one"
    );
}

/// Ctrl+K, "Undo", Enter.
fn palette_undo(h: &mut AppH) {
    press(h, egui::Modifiers::COMMAND, egui::Key::K);
    h.event(egui::Event::Text("Undo".into()));
    h.run_steps(2);
    h.key_press(egui::Key::Enter);
    h.run_steps(4);
    assert!(
        h.state().model.nav.palette.is_none(),
        "the palette ran a pick"
    );
}

#[test]
fn the_palette_undo_follows_the_nif_history_while_editing() {
    let (_t, _d, mut h) = both_histories(1);
    let tex = textures(&h);
    palette_undo(&mut h);
    assert!(
        sdc_gui::ui::header::meshes_view_active(&h.ctx),
        "the pick was the action, not a texture"
    );
    assert_eq!(nif(&h), (0, true));
    assert_eq!(textures(&h), tex);
    close(&mut h);
    palette_undo(&mut h);
    assert!(h.state().model.can_redo());
}

#[test]
fn header_undo_enabled_state_and_hover_follow_the_nif_history() {
    let (_t, _d, mut h) = both_histories(0);
    assert!(
        support::header_undo(&h, "Undo")
            .accesskit_node()
            .is_disabled(),
        "no NIF step to undo, though the Textures history has one"
    );
    support::header_undo(&h, "Undo").hover();
    h.run_steps(30);
    // Note FC12: the hover and the button name the NIF history.
    h.get_by_label("Nothing to undo in the NIF edit history");
    tick(&mut h.state_mut().meshes, 40.5);
    h.state_mut().meshes.edit.as_mut().unwrap().flush();
    h.run_steps(4);
    assert!(!support::header_undo(&h, "Undo")
        .accesskit_node()
        .is_disabled());
    assert!(support::header_undo(&h, "Redo")
        .accesskit_node()
        .is_disabled());
    support::header_undo(&h, "Undo").hover();
    h.run_steps(30);
    h.get_by_label_contains("(Ctrl+Z). Acts on the NIF edit history");
    support::header_undo(&h, "Undo").click();
    h.run_steps(4);
    support::header_undo(&h, "Redo").hover();
    h.run_steps(30);
    h.get_by_label_contains("Shift+Z). Acts on the NIF edit history");
}

#[test]
fn overlay_history_commands_leave_the_hidden_nif_edit_alone() {
    let (_t, _d, mut h) = both_histories(1);
    h.get_by_label("Settings…").click();
    h.run_steps(4);
    h.get_by_label("Back to meshes");
    let tex = textures(&h);
    support::header_undo(&h, "Undo").click();
    h.run_steps(4);
    assert!(
        h.state().model.can_redo(),
        "the overlay header undoes Textures"
    );
    press(&mut h, egui::Modifiers::COMMAND, egui::Key::Y);
    assert_eq!(textures(&h), tex, "the overlay shortcut redoes Textures");
    palette_undo(&mut h);
    assert!(
        h.state().model.can_redo(),
        "the overlay palette undoes Textures"
    );
    support::header_undo(&h, "Redo").click();
    h.run_steps(4);
    assert_eq!(textures(&h), tex);
    press(&mut h, egui::Modifiers::COMMAND, egui::Key::Z);
    assert!(h.state().model.can_redo());
    h.get_by_label("Back to meshes").click();
    h.run_steps(4);
    assert!(!h.state().model.centre_over_meshes);
    assert_eq!(
        nif(&h),
        (1, false),
        "Back must never apply a queued NIF undo"
    );
}

/// Review: in the Textures view the header Undo and Ctrl+Z act on Textures although a NIF edit is open.
#[test]
fn the_textures_view_undoes_textures_with_a_nif_edit_open() {
    let (_t, _d, mut h) = both_histories(1);
    sdc_gui::ui::header::set_meshes_view(&h.ctx, false);
    h.run_steps(4);
    support::header_undo(&h, "Undo").click();
    h.run_steps(4);
    assert!(h.state().model.can_redo(), "the header undid Textures");
    assert_eq!(nif(&h), (1, false), "the NIF history is untouched");
    h.state_mut().model.redo();
    h.run_steps(4);
    assert!(!h.state().model.can_redo());
    press(&mut h, egui::Modifiers::COMMAND, egui::Key::Z);
    assert!(h.state().model.can_redo(), "Ctrl+Z undid Textures");
    assert_eq!(nif(&h), (1, false));
}

#[test]
fn ctrl_z_does_nothing_in_a_text_field_a_popup_or_a_dialog() {
    use egui::{Key, Modifiers};
    let (_t, _d, mut h) = both_histories(1);
    // Meshes UI T2: with Edit open the mod filter is hidden (Shapes locked): a texture path field instead.
    let (shape, _) = lit_shape(h.state().meshes.edit.as_ref().unwrap());
    h.state_mut().meshes.select_edit(Selection::Shape(shape));
    h.run_steps(2);
    let tex = textures(&h);
    // A focused text field.
    h.query_all_by_role(AkRole::TextInput)
        .next()
        .expect("a text field")
        .focus();
    h.run_steps(2);
    press(&mut h, Modifiers::COMMAND, Key::Z);
    assert_eq!(nif(&h), (1, false), "not with a text field focused");
    assert_eq!(textures(&h), tex);
    h.key_press(Key::Escape);
    h.run_steps(4);
    // An open popup (the header's Open menu).
    support::header_undo(&h, "Open").click();
    h.run_steps(4);
    assert!(egui::Popup::is_any_open(&h.ctx), "the menu is open");
    press(&mut h, Modifiers::COMMAND, Key::Z);
    assert_eq!(nif(&h), (1, false), "not with a popup open");
    assert_eq!(textures(&h), tex);
    h.key_press(Key::Escape);
    h.run_steps(4);
    // A shared dialog.
    h.state_mut().model.dialog = Some(sdc_gui::model::Dialog::Error("x".into()));
    h.run_steps(4);
    press(&mut h, Modifiers::COMMAND, Key::Z);
    assert_eq!(nif(&h), (1, false), "not with a dialog open");
    assert_eq!(textures(&h), tex);
    // The guard: with nothing in the way, Ctrl+Z undoes the NIF step.
    h.state_mut().model.dialog = None;
    h.run_steps(4);
    assert!(!egui::Popup::is_any_open(&h.ctx));
    press(&mut h, Modifiers::COMMAND, Key::Z);
    assert_eq!(nif(&h), (0, true));
    assert_eq!(textures(&h), tex);
}

/// Sets `mask` bits of SLSF2 on shape A's shader (the edit panel redraws on the next run).
fn set_slsf2(h: &mut Harness<'static, MeshesModel>, a: u32, mask: u32) {
    let m = h.state_mut();
    let e = m.edit.as_ref().unwrap();
    let at = nif::edit::anchor_role(&e.model, Some(a), Role::Shader).unwrap();
    let shader = e.model.shape(a).unwrap().shader.unwrap();
    let base = e.model.lighting[&shader].lighting.slsf2;
    m.edit_push(
        vec![Op::SetBits {
            at,
            field: Field::Slsf2,
            mask,
            value: mask,
            base,
        }],
        None,
    );
    h.run_steps(2);
}

#[test]
fn pbr_shape_labels_slsf2_bits_and_non_pbr_does_not() {
    let mut m = direct(scenes::shared_alpha_and_set());
    let a = select(&mut m, "A");
    let mut h = view(m);
    h.run_steps(2);
    open_flags(&mut h); // Meshes UI T6: the flag lists start closed
    assert!(h.query_all_by_label_contains("PBR:").next().is_none());
    set_slsf2(&mut h, a, (1 << 23) | (1 << 24) | (1 << 25));
    h.get_by_label("PBR: Two-layer coat");
    h.get_by_label("PBR: Inner-layer parallax");
    // The checkbox keeps its name and adds the PBR meaning.
    h.get_by_role_and_label(
        egui::accesskit::Role::CheckBox,
        "Multi_Layer_Parallax (PBR: Two-layer coat)",
    );
}

#[test]
fn slsf2_warning_shows_under_the_flags() {
    let mut m = direct(scenes::shared_alpha_and_set());
    let a = select(&mut m, "A");
    let mut h = view(m);
    h.run_steps(2);
    open_flags(&mut h); // Meshes UI T6: the flag lists start closed
    set_slsf2(&mut h, a, 1 << 12);
    h.get_by_label("Fit_Slope (SLSF2 12) does nothing without the PBR flag.");
}

// ---- v2 P14 (owner, 2026-10-20): the NIF export follows the Textures export folder ----

/// `files_with(false, modlist)` with `mods\MyOut` made before the scan (so a listed MyOut gets its mod row); scanned,
/// x.nif selected, the export root set to `root(instance)`.
fn session_into(
    modlist: &str,
    root: impl Fn(&Path) -> std::path::PathBuf,
) -> (tempfile::TempDir, MeshesModel) {
    let d = files_with(false, modlist);
    std::fs::create_dir_all(d.path().join("mods").join("MyOut")).unwrap();
    let mut m = MeshesModel::new(Some(d.path().join("scan.db")), MeshesSettings::default());
    m.procs = sdc_gui::model::meshes::no_procs;
    scan_and_select(&mut m, d.path());
    m.set_nif_export_root(Some(root(d.path())));
    (d, m)
}

fn nif_doc(root: &Path) -> std::path::PathBuf {
    root.join(r".sdc\nif-edits\meshes\x.nif.sdc-nifedit.toml")
}

#[test]
fn export_goes_to_the_textures_export_folder_and_checks_that_mod() {
    let (d, mut m) = session_into("+PGP\n+MyOut\n+A\n", |r| r.join(r"mods\MyOut"));
    let out = d.path().join(r"mods\MyOut");
    m.open_edit();
    assert_eq!(m.error, None);
    assert_eq!(m.edit.as_ref().unwrap().placement, Some(Placement::Ok));
    gloss(&mut m, 12.5);
    assert_eq!(export(&mut m), Ok(1));
    assert!(out.join(r"meshes\x.nif").is_file());
    assert!(nif_doc(&out).is_file());
    assert!(!d.path().join("mods").join(EDIT_MOD).exists());
    assert_eq!(badge_of(&m, "x.nif"), Some(DocBadge::Exported));
}

#[test]
fn placement_names_the_export_folder_mod() {
    let (_d, mut m) = session_into("+PGP\n+A\n+MyOut\n", |r| r.join(r"mods\MyOut"));
    m.open_edit();
    let p = m.edit.as_ref().unwrap().placement.clone().unwrap();
    assert_eq!(p, Placement::Below("A".into()));
    assert_eq!(
        p.text("MyOut"),
        "The edit mod is below A: your edit loses. Drag 'MyOut' below A in MO2."
    );
}

/// Where a session with no project open saves its draft (Review E6; Note GD5 (c)):
/// `<scan.db's folder>\nif-edits\<hash>.toml`. The export folder plays no part.
fn autosave(d: &tempfile::TempDir) -> std::path::PathBuf {
    d.path()
        .join("nif-edits")
        .join(format!("{}.toml", sha256_hex(b"meshes\\x.nif")))
}

// ---- Review E6: the draft lives with the project, the export folder keeps only a record ----

/// Where a session in `project` saves its draft of `meshes\x.nif`.
fn draft(p: &Path) -> std::path::PathBuf {
    p.join(r"_sdc\nif-edits\meshes\x.nif.sdc-nifedit.toml")
}

/// `export_session`'s export folder (`mods\sdc NIF Edits`).
fn export_root(d: &tempfile::TempDir) -> std::path::PathBuf {
    d.path().join("mods").join(EDIT_MOD)
}

/// A folder `name` in the session's temp dir, made.
fn project_folder(d: &tempfile::TempDir, name: &str) -> std::path::PathBuf {
    let p = d.path().join(name);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// `export_session` with the project `name` opened (a folder in its temp dir) and `export` chosen as the export
/// folder (None: `mods\sdc NIF Edits`, as every session of this binary has).
fn session_in_project(
    name: &str,
    export: Option<&Path>,
) -> (tempfile::TempDir, MeshesModel, std::path::PathBuf) {
    let (d, mut m) = export_session();
    if let Some(e) = export {
        m.set_nif_export_root(Some(e.to_path_buf()));
    }
    let p = project_folder(&d, name);
    m.set_project_root(Some(&p));
    (d, m, p)
}

/// Every `*.toml` under `dir`, oldest name first.
fn tomls_under(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = vec![];
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    for f in rd {
        let p = f.unwrap().path();
        if p.is_dir() {
            out.extend(tomls_under(&p));
        } else if p.extension().is_some_and(|x| x == "toml") {
            out.push(p);
        }
    }
    out.sort();
    out
}

/// The values the document's ops set, as `op_values` reads them from a session.
fn doc_values(p: &Path) -> Vec<f32> {
    load(p)
        .unwrap()
        .unwrap_or_else(|| panic!("{} holds no document", p.display()))
        .ops
        .iter()
        .map(|d| match &d.op {
            Op::SetField {
                value: Value::F32(v),
                ..
            } => f32::from_bits(*v),
            o => panic!("{o:?}"),
        })
        .collect()
}

/// Project B with its own draft of `meshes\x.nif` (one op, 7.5), made by a session of its own that is then closed.
/// The model is left with project B open and no session.
fn project_with_a_draft(m: &mut MeshesModel, b: &Path) -> Vec<u8> {
    m.set_project_root(Some(b));
    m.open_edit();
    gloss(m, 7.5);
    m.close_edit();
    assert!(draft(b).exists(), "B holds its own draft");
    std::fs::read(draft(b)).unwrap()
}

/// The kept autosave of the NIF's draft `id`.
fn kept(d: &tempfile::TempDir, id: &str) -> std::path::PathBuf {
    kept_autosave_path(d.path(), "meshes\\x.nif", id)
}

/// The id the draft in `p` carries.
fn id_of(p: &Path) -> String {
    load(&draft(p))
        .unwrap()
        .unwrap()
        .id
        .expect("a saved draft has its id")
}

fn export_review(d: &tempfile::TempDir) -> sdc_gui::model::AppModel {
    app_data();
    let settings = sdc_gui::settings::GuiSettings {
        export_folder: Some(export_root(d)),
        ..Default::default()
    };
    sdc_gui::model::AppModel::new(settings, None, support::session())
}

#[test]
fn the_record_names_its_draft_by_id_not_by_path() {
    let (d, mut m, p) = saved_in_p();
    assert_eq!(export(&mut m), Ok(1));
    let rec = nif_doc(&export_root(&d));
    let id = id_of(&p);
    assert!(valid_id(&id));
    assert_eq!(load(&rec).unwrap().unwrap().export.owner, Some(id));
    let text = std::fs::read_to_string(rec).unwrap().replace(r"\\", r"\");
    assert!(!text.contains(p.to_str().unwrap()));
}

#[test]
fn the_owner_survives_the_autosave_moving_into_a_project() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    assert_eq!(export(&mut m), Ok(1));
    let id = load(&autosave(&d)).unwrap().unwrap().id.unwrap();
    let p = project_folder(&d, "p");
    m.set_project_root(Some(&p));
    assert_eq!(id_of(&p), id, "the autosave's id moves with its ops");
    gloss(&mut m, 13.5);
    assert_eq!(review_row(&m).foreign, None);
    assert_eq!(export(&mut m), Ok(1));
    let rec = load(&nif_doc(&export_root(&d))).unwrap().unwrap();
    assert_eq!(rec.export.owner, Some(id));
    assert_eq!(
        rec.export.sha256,
        Some(sha256_hex(&std::fs::read(exported(&d)).unwrap()))
    );
}

#[test]
fn a_carry_checks_the_kept_autosave_for_the_incoming_id() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    let plain = autosave(&d);
    let before = std::fs::read(&plain).unwrap();
    let doc = load(&plain).unwrap().unwrap();
    let k = kept(&d, doc.id.as_deref().unwrap());
    save(&k, &with_value(doc, 8.5)).unwrap();
    ahead(&k, 3600);
    let p = project_folder(&d, "p");
    m.set_project_root(Some(&p));
    assert!(
        !draft(&p).exists(),
        "carry created a draft behind the newer kept autosave"
    );
    assert_eq!(std::fs::read(&plain).unwrap(), before);
    assert_eq!(doc_values(&k), [8.5]);
    assert!(review_row(&m).blocked.unwrap().contains("autosave"));
}

#[test]
fn a_retried_carry_checks_the_kept_autosave_before_saving() {
    let (d, mut m) = export_session();
    let p = project_folder(&d, "p");
    m.open_edit();
    gloss(&mut m, 12.5);
    let plain = autosave(&d);
    let before = std::fs::read(&plain).unwrap();
    let doc = load(&plain).unwrap().unwrap();
    let k = kept(&d, doc.id.as_deref().unwrap());
    std::fs::write(p.join("_sdc"), b"blocked").unwrap();
    m.set_project_root(Some(&p));
    assert!(review_row(&m).blocked.is_some());
    std::fs::remove_file(p.join("_sdc")).unwrap();
    save(&k, &with_value(doc, 8.5)).unwrap();
    ahead(&k, 3600);
    gloss(&mut m, 13.5);
    assert!(
        !draft(&p).exists(),
        "the retry wrote behind a new kept autosave"
    );
    assert_eq!(std::fs::read(&plain).unwrap(), before);
    assert_eq!(doc_values(&k), [8.5]);
    assert!(review_row(&m).blocked.is_some());
}

#[test]
fn the_owner_survives_a_renamed_project_folder() {
    let (d, mut m, p) = saved_in_p();
    assert_eq!(export(&mut m), Ok(1));
    let id = id_of(&p);
    let p2 = d.path().join("renamed");
    std::fs::rename(&p, &p2).unwrap();
    m.set_project_root(Some(&p2));
    assert!(!p.exists());
    assert_eq!(id_of(&p2), id);
    gloss(&mut m, 13.5);
    assert_eq!(review_row(&m).foreign, None);
    assert_eq!(export(&mut m), Ok(1));
    let rec = load(&nif_doc(&export_root(&d))).unwrap().unwrap();
    assert_eq!(rec.export.owner, Some(id));
    assert_eq!(
        rec.export.sha256,
        Some(sha256_hex(&std::fs::read(exported(&d)).unwrap()))
    );
}

#[test]
fn a_legacy_draft_exported_with_no_new_edit_is_listed_and_saved_first() {
    use sdc_gui::model::review::NIF_DRAFT_HOVER;
    let (d, mut m, p) = saved_in_p();
    m.close_edit();
    let path = draft(&p);
    let mut old = load(&path).unwrap().unwrap();
    old.id = None;
    save(&path, &old).unwrap();
    m.open_edit();
    let mut model = export_review(&d);
    model.set_nif_review_rows(m.nif_review_rows());
    assert!(
        model
            .review_writes()
            .also_updated
            .iter()
            .any(|w| w.path == path && w.hover == NIF_DRAFT_HOVER),
        "Review lists the legacy draft before Export assigns its id"
    );
    assert_eq!(export(&mut m), Ok(1));
    assert_eq!(
        load(&nif_doc(&export_root(&d)))
            .unwrap()
            .unwrap()
            .export
            .owner,
        Some(id_of(&p))
    );
}

#[test]
fn a_failed_pre_record_write_stops_before_the_nif() {
    let (d, mut m, _p) = saved_in_p();
    std::fs::write(export_root(&d).join(".sdc"), b"plain file").unwrap();
    let why = export(&mut m).unwrap_err();
    assert!(
        !exported(&d).exists(),
        "a failed pre-record wrote the NIF: {why}"
    );
    assert!(why.contains("x.nif"), "{why}");
    assert!(m.edit.as_ref().unwrap().pending());
}

#[test]
fn a_failed_record_write_after_the_nif_leaves_the_pre_record() {
    let (d, mut m, p) = saved_in_p();
    let e = d.path().join(r"mods\MyOut");
    m.set_nif_export_root(Some(e.clone()));
    let copies = [copy_at(&e, "out", 9)];
    let source = || pick_source(&copies, Path::new(r"C:\elsewhere"), "meshes/x.nif");
    assert!(source().is_some());
    m.fail_record_write_after_nif_for_test();
    export_fails(&mut m, "is in place, but its record");
    assert!(e.join(r"meshes\x.nif").is_file());
    let pre = load(&nif_doc(&e))
        .unwrap()
        .expect("the pre-record survives the final failure");
    assert_eq!(pre.export.owner, Some(id_of(&p)));
    assert_eq!(
        (pre.export.sha256, pre.export.at, pre.export.ops_hash),
        (None, None, None)
    );
    assert!(pre.ops.is_empty());
    assert!(
        source().is_none(),
        "the partial export cannot be its own source"
    );
    assert_eq!(export(&mut m), Ok(1), "one-shot fault");
    let old = load(&nif_doc(&e)).unwrap().unwrap();
    gloss(&mut m, 13.5);
    m.fail_record_write_after_nif_for_test();
    export_fails(&mut m, "is in place, but its record");
    let pre = load(&nif_doc(&e)).unwrap().unwrap();
    assert_eq!(pre.export, old.export);
    assert_eq!(pre.ops, old.ops);
}

#[test]
fn an_export_from_another_project_asks_before_overwriting() {
    let (d, mut m, _p) = saved_in_p();
    assert_eq!(export(&mut m), Ok(1));
    let original = std::fs::read(exported(&d)).unwrap();
    let rec = nif_doc(&export_root(&d));
    let before = std::fs::read(&rec).unwrap();
    let b = project_folder(&d, "b");
    m.set_project_root(Some(&b));
    gloss(&mut m, 13.5);
    assert_eq!(review_row(&m).foreign, Some(sha(&original)));
    export_fails(&mut m, NIF_UNANSWERED);
    assert_eq!(std::fs::read(exported(&d)).unwrap(), original);
    assert_eq!(std::fs::read(&rec).unwrap(), before);
    answer(&mut m, GuardChoice::Overwrite);
    assert_eq!(export(&mut m), Ok(1));
    assert_ne!(std::fs::read(exported(&d)).unwrap(), original);
    assert_eq!(load(&rec).unwrap().unwrap().export.owner, Some(id_of(&b)));
}

#[test]
fn a_restarted_cross_project_export_asks_again_after_the_final_write_failed() {
    for (value, old_pre_record) in [(13.5, false), (12.5, false), (12.5, true)] {
        let (d, mut m, a) = saved_in_p();
        assert_eq!(export(&mut m), Ok(1));
        let path = nif_doc(&export_root(&d));
        let previous = load(&path).unwrap().unwrap();
        let b = project_folder(&d, "b");
        m.set_project_root(Some(&b));
        gloss(&mut m, value);
        answer(&mut m, GuardChoice::Overwrite);
        m.fail_record_write_after_nif_for_test();
        export_fails(&mut m, "is in place, but its record");
        let mut pre = load(&path).unwrap().unwrap();
        assert_ne!(id_of(&a), id_of(&b));
        assert_eq!(pre.export.owner, Some(id_of(&b)));
        assert_eq!(pre.export.sha256, previous.export.sha256);
        assert_eq!(pre.ops, previous.ops);
        let installed = std::fs::read(exported(&d)).unwrap();
        let live_row = m.nif_review_rows();
        drop(m);
        if old_pre_record {
            // The first owner-aware writer left A's document ID in B's pre-record.
            pre.id.clone_from(&previous.id);
            save(&path, &pre).unwrap();
        }
        let mut m = restart(&d, &b);
        m.open_edit();
        assert_eq!(
            (live_row.len(), m.nif_review_rows().len()),
            (1, 1),
            "missing repair row for {value}"
        );
        assert_eq!(live_row[0].blocked, None);
        if old_pre_record {
            gloss(&mut m, 19.5);
            m.set_nif_export_root(Some(d.path().join("other export")));
            m.set_nif_export_root(Some(export_root(&d)));
            assert!(m.undo_edit());
            assert_eq!(
                m.nif_review_rows().len(),
                1,
                "undo hid the old pre-record's repair"
            );
        }
        assert_eq!(review_row(&m).foreign, Some(sha(&installed)));
        export_fails(&mut m, NIF_UNANSWERED);
        assert_eq!(load(&path).unwrap().unwrap(), pre);
        answer(&mut m, GuardChoice::Overwrite);
        assert_eq!(export(&mut m), Ok(1));
        assert_eq!(doc_values(&path), [value]);
        assert_eq!(load(&path).unwrap().unwrap().id, Some(id_of(&b)));
    }
}

#[test]
fn another_owners_record_with_unexported_ops_stays_protected() {
    let (d, mut m, _a) = saved_in_p();
    assert_eq!(export(&mut m), Ok(1));
    let path = nif_doc(&export_root(&d));
    let edited = with_value(load(&path).unwrap().unwrap(), 8.5);
    save(&path, &edited).unwrap();
    let nif = std::fs::read(exported(&d)).unwrap();
    let b = project_folder(&d, "b");
    m.set_project_root(Some(&b));
    gloss(&mut m, 13.5);
    m.set_nif_answer("meshes/x.nif", GuardChoice::Overwrite, sha(&nif));
    export_fails(&mut m, "holds other edits");
    assert_eq!(load(&path).unwrap().unwrap(), edited);
    assert_eq!(std::fs::read(exported(&d)).unwrap(), nif);
}

#[test]
fn another_owner_is_foreign_even_when_the_draft_knows_the_same_nif_hash() {
    let (d, mut m, _a) = saved_in_p();
    assert_eq!(export(&mut m), Ok(1));
    let nif = std::fs::read(exported(&d)).unwrap();
    m.set_nif_export_root(Some(d.path().join("other")));
    let b = project_folder(&d, "b");
    m.set_project_root(Some(&b));
    gloss(&mut m, 12.5);
    assert_eq!(export(&mut m), Ok(1));
    gloss(&mut m, 13.5);
    m.set_nif_export_root(Some(export_root(&d)));
    assert_eq!(
        m.edit.as_ref().unwrap().doc.export.sha256,
        Some(sha256_hex(&nif))
    );
    assert_eq!(review_row(&m).foreign, Some(sha(&nif)));
    export_fails(&mut m, NIF_UNANSWERED);
    assert_eq!(std::fs::read(exported(&d)).unwrap(), nif);
}

#[test]
fn undo_to_zero_cannot_replace_a_drifted_nif_in_another_owners_folder() {
    let (d, mut m, _a) = saved_in_p();
    assert_eq!(export(&mut m), Ok(1));
    let installed = exported(&d);
    let record = nif_doc(&export_root(&d));
    let original = std::fs::read(&record).unwrap();
    std::fs::write(&installed, b"replacement").unwrap();
    let b = project_folder(&d, "b");
    m.set_project_root(Some(&b));
    assert!(m.nif_review_rows().is_empty(), "untouched B stays idle");
    m.set_nif_export_root(Some(d.path().join("B exports")));
    gloss(&mut m, 14.5);
    assert_eq!(export(&mut m), Ok(1));
    assert!(m.undo_edit());
    m.set_nif_export_root(Some(export_root(&d)));
    m.set_nif_answer("meshes/x.nif", GuardChoice::Overwrite, sha(b"replacement"));
    export_fails(&mut m, NIF_UNDONE);
    assert_eq!(std::fs::read(installed).unwrap(), b"replacement");
    assert_eq!(std::fs::read(record).unwrap(), original);
}

#[test]
fn an_open_session_sees_the_new_owner_when_the_record_mtime_changes() {
    let (d, mut a, _p) = saved_in_p();
    assert_eq!(export(&mut a), Ok(1));
    let original = std::fs::read(exported(&d)).unwrap();
    let mut b = restart(&d, &project_folder(&d, "b"));
    b.open_edit();
    gloss(&mut b, 12.5);
    answer(&mut b, GuardChoice::Overwrite);
    assert_eq!(export(&mut b), Ok(1));
    ahead(&nif_doc(&export_root(&d)), 3600);
    assert_eq!(std::fs::read(exported(&d)).unwrap(), original);
    gloss(&mut a, 13.5);
    a.set_nif_export_root(Some(export_root(&d))); // the app's next frame
    assert_eq!(review_row(&a).foreign, Some(sha(&original)));
    export_fails(&mut a, NIF_UNANSWERED);
}

#[test]
fn changing_the_edit_refreshes_the_cached_foreign_hash() {
    let (d, mut m, _p) = saved_in_p();
    let path = exported(&d);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"first").unwrap();
    let time = std::fs::metadata(&path).unwrap().modified().unwrap();
    answer(&mut m, GuardChoice::KeepTheirs);
    std::fs::write(&path, b"other").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(time)
        .unwrap();
    gloss(&mut m, 13.5);
    assert_eq!(review_row(&m).foreign, Some(sha(b"other")));
    assert_eq!(review_row(&m).answer, None);
}

#[test]
fn a_first_pre_record_still_identifies_an_undone_partial_after_restart() {
    let (d, mut m, p) = saved_in_p();
    m.fail_record_write_after_nif_for_test();
    export_fails(&mut m, "is in place, but its record");
    assert!(m.undo_edit());
    drop(m);
    let mut m = restart(&d, &p);
    m.open_edit();
    assert!(
        m.edit.as_ref().unwrap().pending(),
        "the installed NIF has a pre-record, but no recorded hash"
    );
    assert_eq!(review_row(&m).blocked.as_deref(), Some(NIF_UNDONE));
    export_fails(&mut m, NIF_UNDONE);
}

#[test]
fn another_projects_record_does_not_make_this_session_pending() {
    let (d, mut m, _p) = saved_in_p();
    assert_eq!(export(&mut m), Ok(1));
    let b = project_folder(&d, "b");
    m.set_project_root(Some(&b));
    assert!(!m.edit.as_ref().unwrap().pending(), "B has no draft");
    assert!(m.nif_review_rows().is_empty());
    let mut h = view(m);
    h.run_steps(10);
    h.state_mut().set_project_root(None);
    assert!(!b.join("_sdc").exists());
}

#[test]
fn export_lists_and_writes_the_draft_only_when_it_changed() {
    use sdc_gui::model::review::NIF_DRAFT_HOVER;
    let (d, mut m, p) = saved_in_p();
    let mut model = export_review(&d);
    let path = draft(&p);
    let old = std::time::UNIX_EPOCH + Duration::from_secs(1);
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(old)
        .unwrap();
    model.set_nif_review_rows(m.nif_review_rows());
    assert!(!model
        .review_writes()
        .also_updated
        .iter()
        .any(|w| w.hover == NIF_DRAFT_HOVER));
    assert_eq!(export(&mut m), Ok(1));
    assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), old);
    tick(&mut m, 40.5);
    assert!(
        review_row(&m).draft_unsaved,
        "an unflushed slider drag needs a draft write"
    );
    model.set_nif_review_rows(m.nif_review_rows());
    assert!(model
        .review_writes()
        .also_updated
        .iter()
        .any(|w| w.path == path && w.hover == NIF_DRAFT_HOVER));
    assert_eq!(export(&mut m), Ok(1));
    assert_eq!(doc_values(&path), [12.5, 40.5]);
}

#[test]
fn a_source_change_at_export_waits_for_review_to_list_the_new_draft_write() {
    use sdc_gui::model::review::NIF_DRAFT_HOVER;
    let (d, mut m, p) = saved_in_p();
    let mut model = export_review(&d);
    let out = export_root(&d);
    let roots = [p.as_path(), out.as_path()];
    model.set_nif_review_rows(m.nif_review_rows());
    assert!(!model
        .review_writes()
        .also_updated
        .iter()
        .any(|w| w.hover == NIF_DRAFT_HOVER));
    let before = support::tree_hashes(&roots);
    let source = changed_source(&m);
    std::fs::write(d.path().join(r"mods\A\meshes\x.nif"), &source).unwrap();
    let first = export(&mut m);
    assert_eq!(
        before,
        support::tree_hashes(&roots),
        "the newly needed draft write was absent from Review"
    );
    assert!(first.unwrap_err().contains("source NIF changed"));
    model.set_nif_review_rows(m.nif_review_rows());
    assert!(model
        .review_writes()
        .also_updated
        .iter()
        .any(|w| w.path == draft(&p) && w.hover == NIF_DRAFT_HOVER));
    assert_eq!(export(&mut m), Ok(1));
    assert_eq!(
        load(&draft(&p)).unwrap().unwrap().source.sha256,
        sha256_hex(&source)
    );
}

#[test]
fn review_lists_the_files_changed_by_the_nif_export() {
    use std::collections::BTreeSet;
    let (d, mut m, p) = saved_in_p();
    let mut model = export_review(&d);
    let root = export_root(&d);
    let roots = [p.as_path(), root.as_path()];
    tick(&mut m, 40.5);
    let before = support::tree_hashes(&roots);
    model.set_nif_review_rows(m.nif_review_rows());
    let listed: BTreeSet<_> = support::listed(&model.review_writes())
        .into_iter()
        .map(|w| w.path)
        .collect();
    assert_eq!(export(&mut m), Ok(1));
    let after = support::tree_hashes(&roots);
    let changed = after
        .keys()
        .chain(before.keys())
        .filter(|p| before.get(*p) != after.get(*p))
        .cloned()
        .collect();
    assert_eq!(
        listed, changed,
        "Review and Export agree on every changed file"
    );

    // Keep applies even if an edit subsequently makes the output equal the installed, unrecorded NIF (E6-S2).
    std::fs::remove_file(nif_doc(&root)).unwrap();
    m.set_nif_export_root(None);
    m.set_nif_export_root(Some(root.clone()));
    assert!(m.undo_edit());
    answer(&mut m, GuardChoice::KeepTheirs);
    model.set_nif_review_rows(m.nif_review_rows());
    assert!(model.review_writes().also_updated.is_empty());
    assert!(m.redo_edit());
    model.set_nif_review_rows(m.nif_review_rows());
    assert!(model.review_writes().also_updated.is_empty());
    let before = support::tree_hashes(&roots);
    export_fails(&mut m, NIF_KEPT);
    assert_eq!(before, support::tree_hashes(&roots));
}

/// The files a save moved aside from the kept autosave `k` (`<stem>.<seconds>.toml` in its folder), oldest name first.
fn moved_aside_from(k: &Path) -> Vec<std::path::PathBuf> {
    kept_beside(k)
}

/// Sets the modified time of `p` an hour back, so a newer file beside it wins a GD11 comparison.
fn make_old(p: &Path) {
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    std::fs::File::options()
        .write(true)
        .open(p)
        .unwrap()
        .set_modified(old)
        .unwrap();
}

#[test]
fn an_edit_is_saved_with_the_project_not_in_the_export_folder() {
    let (d, mut m, p) = session_in_project("proj", None);
    m.open_edit();
    gloss(&mut m, 12.5);
    assert!(draft(&p).exists(), "the draft is saved under the project");
    assert!(
        !nif_doc(&export_root(&d)).exists(),
        "nothing is written to the export folder before an export"
    );
}

#[test]
fn with_no_project_the_edit_is_saved_in_local_app_data() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    assert!(autosave(&d).exists(), "the autosave beside the store");
    assert!(!nif_doc(&export_root(&d)).exists());
}

#[test]
fn changing_the_export_folder_writes_nothing() {
    let (d, mut m, p) = session_in_project("proj", None);
    m.open_edit();
    gloss(&mut m, 12.5);
    assert!(
        draft(&p).exists(),
        "premise: the draft is under the project"
    );
    let before = std::fs::read(draft(&p)).unwrap();
    let new = d.path().join(r"mods\MyOut");
    m.set_nif_export_root(Some(new.clone()));
    assert_eq!(m.nif_export_root.as_deref(), Some(new.as_path()));
    assert_eq!(std::fs::read(draft(&p)).unwrap(), before);
    assert!(!nif_doc(&new).exists());
    assert!(
        std::fs::read_dir(&new).map_or(true, |mut r| r.next().is_none()),
        "the new folder is still empty"
    );
    assert_eq!(op_values(&m).len(), 1, "the edit stays open as it was");
}

#[test]
fn a_project_switch_saves_to_the_old_project_and_ends_the_edit() {
    let (d, mut m) = export_session();
    let (a, b) = (project_folder(&d, "a"), project_folder(&d, "b"));
    let b_bytes = project_with_a_draft(&mut m, &b);
    m.set_project_root(Some(&a));
    m.open_edit();
    gloss(&mut m, 12.5);
    m.set_project_root(Some(&b));
    assert_eq!(doc_values(&draft(&a)), [12.5], "A's draft holds A's edit");
    assert_eq!(
        std::fs::read(draft(&b)).unwrap(),
        b_bytes,
        "B's is untouched"
    );
    assert_eq!(
        op_values(&m).iter().map(|v| v.1).collect::<Vec<_>>(),
        [7.5],
        "the open session holds B's ops"
    );
    assert_eq!(
        tomls_under(&b.join(r"_sdc\nif-edits")),
        [draft(&b)],
        "nothing of A's reached B"
    );
}

#[test]
fn closing_the_project_saves_the_draft_and_ends_the_edit() {
    let (d, mut m, a) = session_in_project("a", None);
    m.open_edit();
    gloss(&mut m, 12.5);
    m.set_project_root(None);
    assert_eq!(doc_values(&draft(&a)), [12.5]);
    assert!(!autosave(&d).exists(), "nothing goes to the autosave");
    assert!(
        m.edit.as_ref().is_some_and(|e| e.doc.ops.is_empty()),
        "the reopened session has no ops"
    );
}

#[test]
fn opening_a_project_carries_the_autosave_into_it() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    assert!(
        autosave(&d).exists(),
        "premise: the edit is in the autosave"
    );
    let p = project_folder(&d, "p");
    m.set_project_root(Some(&p));
    // ruling E6-H1: the carry writes the project's draft itself, then removes the autosave
    assert_eq!(doc_values(&draft(&p)), [12.5]);
    assert!(!autosave(&d).exists(), "the autosave is deleted (GD5 (c))");
    assert!(m.edit.as_ref().unwrap().touched());
    assert_eq!(op_values(&m).len(), 1, "the same session goes on");
    assert_eq!(m.edit.as_ref().unwrap().draft_path, draft(&p));
}

/// Ruling r3-1: a session nothing was done in writes nothing and makes no folder.
#[test]
fn an_untouched_session_writes_nothing_on_a_switch() {
    let (d, mut m, a) = session_in_project("a", None);
    m.open_edit();
    let b = project_folder(&d, "b");
    m.set_project_root(Some(&b));
    assert!(m.edit.is_some(), "premise: the NIF reopens in B");
    m.set_project_root(None);
    assert!(m.edit.is_some(), "premise: and again with no project");
    assert!(!a.join("_sdc").exists());
    assert!(!b.join("_sdc").exists());
    assert!(
        !d.path().join("nif-edits").exists(),
        "no file and no folder in the autosave place"
    );
}

#[test]
fn a_removed_project_folder_keeps_the_edit_on_a_switch() {
    let (d, mut m) = export_session();
    let (a, b) = (project_folder(&d, "a"), project_folder(&d, "b"));
    let b_bytes = project_with_a_draft(&mut m, &b);
    m.set_project_root(Some(&a));
    m.open_edit();
    gloss(&mut m, 12.5);
    let id = id_of(&a);
    std::fs::remove_dir_all(&a).unwrap();
    m.messages.clear();
    m.set_project_root(Some(&b));
    m.set_nif_export_root(m.nif_export_root.clone()); // the next frame moves the session's lines to Messages
    assert!(!a.exists(), "the removed folder is not made again");
    assert_eq!(
        doc_values(&kept(&d, &id)),
        [12.5],
        "the kept autosave holds the edit"
    );
    assert!(
        m.messages
            .iter()
            .any(|l| l.starts_with("Could not save the edits of meshes\\x.nif to ")),
        "{:?}",
        m.messages
    );
    assert_eq!(std::fs::read(draft(&b)).unwrap(), b_bytes);
    // ruling E6-K (replaces "the session is B's"): no open finds a kept autosave of a removed project, so the
    // session stays, its edit kept on disk and open here
    assert_eq!(vals(&m), [12.5], "the session stays");
}

/// Ruling r3-2: the fallback is keyed on the draft's id; another draft's kept autosave and the plain autosave stay.
#[test]
fn a_fallback_never_overwrites_another_kept_autosave() {
    let (d, mut m) = export_session();
    let (a, b) = (project_folder(&d, "a"), project_folder(&d, "b"));
    project_with_a_draft(&mut m, &b);
    m.set_project_root(Some(&a));
    m.open_edit();
    gloss(&mut m, 12.5);
    let id = id_of(&a);
    // another project's kept autosave of this NIF, and the plain no-project autosave (older than B's draft)
    let other = kept(&d, "0123456789abcdef");
    save(&other, &doc()).unwrap();
    save(&autosave(&d), &doc()).unwrap();
    make_old(&autosave(&d));
    let (other_bytes, plain_bytes) = (
        std::fs::read(&other).unwrap(),
        std::fs::read(autosave(&d)).unwrap(),
    );
    std::fs::remove_dir_all(&a).unwrap();
    m.set_project_root(Some(&b));
    assert_eq!(
        doc_values(&kept(&d, &id)),
        [12.5],
        "its own file is written"
    );
    assert_eq!(std::fs::read(&other).unwrap(), other_bytes);
    assert_eq!(std::fs::read(autosave(&d)).unwrap(), plain_bytes);
}

/// Ruling r3-2: a renamed project keeps its draft id, so opening it again brings back the edit the old path could not
/// save (a drag that was not flushed).
#[test]
fn a_renamed_project_gets_its_unflushed_edit_back() {
    let (d, mut m, p) = session_in_project("p", None);
    m.open_edit();
    gloss(&mut m, 12.5);
    make_old(&draft(&p));
    tick(&mut m, 40.5); // a drag, not flushed
    assert_eq!(op_values(&m).len(), 2);
    let id = id_of(&p);
    let p2 = d.path().join("p2");
    std::fs::rename(&p, &p2).unwrap();
    m.set_project_root(Some(&p2));
    assert!(!p.exists(), "the old path is not made again");
    assert!(kept(&d, &id).exists(), "the drag was kept at the switch");
    let v: Vec<f32> = op_values(&m).iter().map(|v| v.1).collect();
    assert_eq!(v, [12.5, 40.5], "the kept autosave is newer than the draft");
    gloss(&mut m, 13.5);
    assert!(!kept(&d, &id).exists(), "its next save removes it");
    assert_eq!(doc_values(&draft(&p2)), [12.5, 40.5, 13.5]);
}

#[test]
fn another_project_never_loads_a_kept_autosave_that_is_not_its_own() {
    let (d, mut m) = export_session();
    let (a, b) = (project_folder(&d, "a"), project_folder(&d, "b"));
    project_with_a_draft(&mut m, &b);
    m.set_project_root(Some(&a));
    m.open_edit();
    gloss(&mut m, 12.5);
    let id = id_of(&a);
    std::fs::remove_dir_all(&a).unwrap();
    m.set_project_root(Some(&b));
    // ruling E6-K: the switch waits (no open finds a removed project's kept autosave). Ruling E6-P2 (was: undone to no
    // ops the switch finishes): a home that is gone cannot tell, so undone it still waits; once the user puts A's
    // folder back (empty) the session matches it and the switch finishes
    assert_eq!(vals(&m), [12.5], "premise: the switch waits");
    assert!(m.undo_edit());
    m.set_project_root(Some(&b));
    assert!(
        m.edit.as_ref().unwrap().draft_path.starts_with(&a),
        "undone, the switch still waits: A is gone"
    );
    std::fs::create_dir(&a).unwrap();
    m.set_project_root(None);
    m.set_project_root(Some(&b));
    let a_kept = std::fs::read(kept(&d, &id)).unwrap();
    let values = |m: &MeshesModel| op_values(m).iter().map(|v| v.1).collect::<Vec<_>>();
    assert_eq!(values(&m), [7.5], "B's session holds B's ops only");
    m.set_project_root(None);
    assert!(
        m.edit.as_ref().unwrap().doc.ops.is_empty(),
        "no project: only the plain autosave is read"
    );
    assert_eq!(std::fs::read(kept(&d, &id)).unwrap(), a_kept);
    // B opened afresh reads its own draft again
    m.set_project_root(Some(&b));
    assert_eq!(values(&m), [7.5]);
    assert_eq!(std::fs::read(kept(&d, &id)).unwrap(), a_kept);
}

/// Ruling r3-3, r4-2: both saves fail, so the edit stays open and blocked; nothing is retried per frame, only by the
/// next edit.
#[test]
fn when_both_saves_fail_the_edit_stays_open_and_export_is_blocked() {
    let (d, mut m) = export_session();
    let (a, b) = (project_folder(&d, "a"), project_folder(&d, "b"));
    project_with_a_draft(&mut m, &b);
    m.set_project_root(Some(&a));
    m.open_edit();
    gloss(&mut m, 12.5);
    let id = id_of(&a);
    let blocker = d.path().join("nif-edits");
    std::fs::write(&blocker, b"a file where the autosave folder goes").unwrap();
    std::fs::remove_dir_all(&a).unwrap();
    m.set_project_root(Some(&b));
    assert!(m.edit.is_some(), "the session is not dropped");
    assert_eq!(
        op_values(&m).iter().map(|v| v.1).collect::<Vec<_>>(),
        [12.5],
        "it still holds A's edit, not B's"
    );
    let why = m.edit.as_ref().unwrap().export_block().unwrap();
    assert!(
        why.starts_with("Could not save the edits of meshes\\x.nif while changing the project: "),
        "{why}"
    );
    // what the modder sees: Export this NIF's greyed hover and the Review row
    assert_eq!(m.quick_export_block().as_deref(), Some(why.as_str()));
    assert_eq!(review_row(&m).blocked.as_deref(), Some(why.as_str()));
    let (tries, lines, stamp) = (
        m.switch_save_attempts_for_test(),
        m.messages.len(),
        std::fs::metadata(&blocker).unwrap().modified().unwrap(),
    );
    assert_eq!(tries, 1, "one attempt of the two saves");
    for _ in 0..10 {
        m.set_project_root(Some(&b));
        assert_eq!(m.quick_export_block().as_deref(), Some(why.as_str()));
        assert_eq!(review_row(&m).blocked.as_deref(), Some(why.as_str()));
    }
    assert_eq!(
        m.switch_save_attempts_for_test(),
        tries,
        "no per-frame retry"
    );
    assert_eq!(m.messages.len(), lines);
    assert_eq!(
        std::fs::metadata(&blocker).unwrap().modified().unwrap(),
        stamp
    );
    // the autosave folder works again: the next edit tries again and keeps the edits. Ruling E6-K (replaces "B
    // opens"): no open finds a removed project's kept autosave, so the session stays and the switch waits
    std::fs::remove_file(&blocker).unwrap();
    gloss(&mut m, 13.5);
    assert_eq!(m.switch_save_attempts_for_test(), tries + 1);
    assert_eq!(doc_values(&kept(&d, &id)), [12.5, 13.5]);
    assert_eq!(vals(&m), [12.5, 13.5], "the session stays");
}

/// Recursively copies `from` to `to` (a project folder).
fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for f in std::fs::read_dir(from).unwrap() {
        let p = f.unwrap().path();
        let t = to.join(p.file_name().unwrap());
        if p.is_dir() {
            copy_dir(&p, &t);
        } else {
            std::fs::copy(&p, &t).unwrap();
        }
    }
}

/// Ruling r4-3: a copy of a project holds the same draft id; its fallback moves the original's kept autosave aside.
#[test]
fn a_copied_project_never_replaces_the_kept_autosave_of_its_original() {
    let (d, mut m) = export_session();
    let (a, b) = (project_folder(&d, "a"), project_folder(&d, "b"));
    m.set_project_root(Some(&a));
    m.open_edit();
    gloss(&mut m, 7.5);
    let id = id_of(&a);
    let a2 = d.path().join("a2");
    copy_dir(&a, &a2);
    assert_eq!(id_of(&a2), id, "the copy keeps the draft's id");
    gloss(&mut m, 12.5);
    std::fs::remove_dir_all(&a).unwrap();
    m.set_project_root(Some(&b));
    let k = kept(&d, &id);
    assert_eq!(doc_values(&k), [7.5, 12.5], "A's edit is kept");
    // ruling E6-K: the switch waits (no open finds a removed project's kept autosave). Ruling E6-P2 (was: undone to no
    // ops the session matches the removed project and ends): a home that is gone cannot tell, so undone it still
    // waits; once the user puts A's folder back (empty) the session matches it and ends, its last state in the kept
    // autosave
    assert!(m.undo_edit() && m.undo_edit());
    m.set_project_root(Some(&b));
    assert!(
        m.edit.as_ref().unwrap().draft_path.starts_with(&a),
        "undone, the switch still waits: A is gone"
    );
    std::fs::create_dir(&a).unwrap();
    m.set_project_root(None);
    m.set_project_root(Some(&b));
    assert!(
        m.edit.as_ref().unwrap().draft_path.starts_with(&b),
        "premise: the NIF opened in B"
    );
    assert!(
        load(&k).unwrap().unwrap().ops.is_empty(),
        "A's last state is kept"
    );
    let a_edit = std::fs::read(&k).unwrap();
    // A2's own draft is the newer file, so its session does not take the original's kept autosave
    make_old(&k);
    let now = std::time::SystemTime::now();
    std::fs::File::options()
        .write(true)
        .open(draft(&a2))
        .unwrap()
        .set_modified(now)
        .unwrap();
    m.set_project_root(Some(&a2));
    assert_eq!(op_values(&m).len(), 1, "A2's session holds A2's draft");
    gloss(&mut m, 9.5);
    std::fs::remove_dir_all(&a2).unwrap();
    m.set_project_root(Some(&b));
    assert_eq!(
        doc_values(&k),
        [7.5, 9.5],
        "the kept autosave now holds A2's edit"
    );
    let aside = moved_aside_from(&k);
    assert_eq!(aside.len(), 1, "{aside:?}");
    assert_eq!(std::fs::read(&aside[0]).unwrap(), a_edit);
}

/// Ruling r4-5: back in its own project, a session whose switch failed saves there and goes on.
#[test]
fn returning_to_the_own_project_after_both_saves_failed_keeps_the_session() {
    let (d, mut m) = export_session();
    let (a, b) = (project_folder(&d, "a"), project_folder(&d, "b"));
    project_with_a_draft(&mut m, &b);
    m.set_project_root(Some(&a));
    m.open_edit();
    let glossiness = |m: &MeshesModel| {
        let e = m.edit.as_ref().unwrap();
        e.model.lighting[&lit_shape(e).1].lighting.params.glossiness
    };
    let source_value = glossiness(&m);
    gloss(&mut m, 12.5);
    gloss(&mut m, 13.5);
    let shape = lit_shape(m.edit.as_ref().unwrap()).0;
    m.select_edit(Selection::Shape(shape));
    // A's folder stays, but `_sdc` is a plain file: no draft can be written there; and no autosave folder either
    std::fs::remove_dir_all(a.join("_sdc")).unwrap();
    std::fs::write(a.join("_sdc"), b"a file").unwrap();
    std::fs::write(d.path().join("nif-edits"), b"a file").unwrap();
    m.set_project_root(Some(&b));
    let why = m.edit.as_ref().unwrap().export_block().unwrap();
    assert!(why.contains("while changing the project"), "{why}");
    let before = ops(&m);
    std::fs::remove_file(a.join("_sdc")).unwrap();
    m.set_project_root(Some(&a));
    let e = m.edit.as_ref().unwrap();
    assert!(
        !e.export_block()
            .is_some_and(|w| w.contains("while changing the project")),
        "the switch reason is gone"
    );
    assert_eq!(doc_values(&draft(&a)), [12.5, 13.5]);
    assert_eq!(ops(&m), before, "the same session, its ops unchanged");
    assert_eq!(e.selection, Selection::Shape(shape));
    assert!(
        m.undo_edit() && m.undo_edit(),
        "both steps are still undoable"
    );
    assert!(m.edit.as_ref().unwrap().doc.ops.is_empty());
    assert_eq!(glossiness(&m), source_value, "the source's value is back");
}

/// Review E6 (Important 5, ruling E6-E): a draft id read from disk is used only when it is 16 hex characters; any
/// other is no id (a new one in memory), so it never becomes part of a kept autosave's file name.
#[test]
fn a_draft_id_from_disk_is_used_only_when_it_is_16_hex_characters() {
    let (d, mut m) = export_session();
    let (a, b) = (project_folder(&d, "a"), project_folder(&d, "b"));
    m.set_project_root(Some(&a));
    m.open_edit();
    gloss(&mut m, 12.5);
    m.close_edit();
    let mut doc = load(&draft(&a)).unwrap().unwrap();
    doc.id = Some(r"x\..\..\escape".into());
    save(&draft(&a), &doc).unwrap();
    m.open_edit();
    gloss(&mut m, 13.5);
    let id = m.edit.as_ref().unwrap().doc.id.clone().unwrap();
    assert!(
        id.len() == 16 && id.bytes().all(|c| c.is_ascii_hexdigit()),
        "{id}"
    );
    std::fs::remove_dir_all(&a).unwrap();
    m.set_project_root(Some(&b));
    assert!(!d.path().join("escape.toml").exists());
    assert_eq!(doc_values(&kept(&d, &id)), [12.5, 13.5]);
}

/// Ruling E6-E: a project folder that goes away after `save_in` checked it is not made again by the save: the folders
/// under it are made one level at a time, and the first fails.
#[test]
fn a_project_folder_removed_during_the_save_is_not_made_again() {
    let (d, mut m) = export_session();
    let a = project_folder(&d, "a");
    m.set_project_root(Some(&a));
    m.open_edit();
    let gone = a.clone();
    sdc_gui::model::meshes::edit_doc::set_write_hook(Some(Box::new(move |stage| {
        if stage == "root checked" {
            let _ = std::fs::remove_dir_all(&gone);
        }
    })));
    gloss(&mut m, 12.5);
    sdc_gui::model::meshes::edit_doc::set_write_hook(None);
    assert!(!a.exists(), "the removed project is not made again");
    assert!(m.error.is_some(), "the save says it failed");
    assert!(
        m.edit.as_ref().unwrap().touched(),
        "and the edit is unsaved"
    );
}

/// Ruling E6-E: the kept autosave is published without a replace: a file that takes its name while it is written
/// (another Tessalith) is never replaced.
#[test]
fn a_kept_autosave_never_replaces_a_file_that_takes_its_name() {
    let (d, mut m) = export_session();
    let (a, b) = (project_folder(&d, "a"), project_folder(&d, "b"));
    m.set_project_root(Some(&a));
    m.open_edit();
    gloss(&mut m, 12.5);
    let id = id_of(&a);
    std::fs::remove_dir_all(&a).unwrap();
    let k = kept(&d, &id);
    let theirs = k.clone();
    sdc_gui::model::meshes::edit_doc::set_write_hook(Some(Box::new(move |stage| {
        if stage == "kept written" {
            std::fs::write(&theirs, b"another instance's").unwrap();
        }
    })));
    m.set_project_root(Some(&b));
    sdc_gui::model::meshes::edit_doc::set_write_hook(None);
    assert_eq!(std::fs::read(&k).unwrap(), b"another instance's");
    assert!(m.edit.is_some(), "no save worked: the session stays");
    assert_eq!(op_values(&m)[0].1, 12.5);
}

/// Review E6 (Important 7, ruling E6-E): an export whose draft save fails stops before the NIF and says so.
#[test]
fn an_export_stops_when_the_draft_cannot_be_saved() {
    let (d, mut m, p) = session_in_project("p", None);
    m.open_edit();
    gloss(&mut m, 12.5);
    tick(&mut m, 40.5); // a drag not saved yet
    read_only(&draft(&p), true);
    let r = export(&mut m);
    read_only(&draft(&p), false);
    let why = r.unwrap_err();
    assert!(why.contains("could not be saved"), "{why}");
    assert!(!exported(&d).exists(), "no NIF written");
    assert!(!nif_doc(&export_root(&d)).exists());
}

/// Review round 2 (Important B6, ruling E6-I): a partial export (the NIF written, its record not) keeps the row until
/// an export writes the record, even when the ops are undone back to the recorded ones; that export replaces the NIF
/// it wrote without asking.
#[test]
fn a_partial_export_keeps_its_row_until_it_is_repaired() {
    let (d, mut m, _p) = session_in_project("p", None);
    m.open_edit();
    gloss(&mut m, 12.5);
    assert_eq!(export(&mut m), Ok(1));
    let (rec, nif_x) = (
        nif_doc(&export_root(&d)),
        std::fs::read(exported(&d)).unwrap(),
    );
    gloss(&mut m, 13.5);
    m.fail_record_write_after_nif_for_test();
    let why = export(&mut m).unwrap_err();
    assert!(why.contains("is in place, but its record"), "{why}");
    assert_ne!(
        std::fs::read(exported(&d)).unwrap(),
        nif_x,
        "premise: NIF Y is in place"
    );
    assert!(m.undo_edit());
    assert_eq!(
        op_values(&m).iter().map(|v| v.1).collect::<Vec<_>>(),
        [12.5]
    );
    assert!(
        m.edit.as_ref().unwrap().pending(),
        "the installed NIF is not the recorded one"
    );
    let row = review_row(&m);
    assert_eq!(row.blocked, None, "Export can repair it");
    assert_eq!(row.foreign, None, "the NIF in place is this session's");
    // ruling E6-N (was: the reason kept in `export_note`): the reason is worked out from disk and is the enabled
    // Export this NIF's hover
    assert_eq!(
        m.quick_export_note(),
        Some(NIF_NOT_RECORDED),
        "the reason stays"
    );
    assert_eq!(m.quick_export_block(), None);
    assert_eq!(export(&mut m), Ok(1));
    assert_eq!(std::fs::read(exported(&d)).unwrap(), nif_x, "NIF X again");
    let r = load(&rec).unwrap().unwrap();
    assert_eq!(doc_values(&rec), [12.5]);
    assert_eq!(r.export.sha256, Some(sha256_hex(&nif_x)));
    assert!(m.nif_review_rows().is_empty(), "the row is gone");
}

/// A new model on the same folders (a restart), project `p` opened and x.nif selected.
fn restart(d: &tempfile::TempDir, p: &Path) -> MeshesModel {
    let mut m = MeshesModel::new(Some(d.path().join("scan.db")), MeshesSettings::default());
    m.procs = sdc_gui::model::meshes::no_procs;
    scan_and_select(&mut m, d.path());
    let x = m
        .nifs
        .iter()
        .find(|n| n.path.ends_with("x.nif"))
        .unwrap()
        .copy_id;
    m.select_nif(x);
    m.set_project_root(Some(p));
    m
}

/// In project `p`: export X (12.5), edit to Y, the record cannot be replaced so the export leaves NIF Y without its
/// record, undo to X. The record is writable again afterwards. Returns NIF X's bytes.
fn partial_y_undone_to_x(d: &tempfile::TempDir, m: &mut MeshesModel) -> Vec<u8> {
    m.open_edit();
    gloss(m, 12.5);
    assert_eq!(export(m), Ok(1));
    let nif_x = std::fs::read(exported(d)).unwrap();
    gloss(m, 13.5);
    m.fail_record_write_after_nif_for_test();
    let why = export(m).unwrap_err();
    assert!(why.contains("is in place, but its record"), "{why}");
    assert!(m.undo_edit());
    assert_eq!(vals(m), [12.5], "premise: undone to X");
    nif_x
}

/// The open edit is pending with its row, not blocked, and says why. Once the session that wrote NIF Y is gone, Y is
/// a file this session did not write, so the row asks Keep or Overwrite for it (the remembered hash is gone).
fn pending_and_repairable(m: &MeshesModel) {
    assert!(m.edit.as_ref().unwrap().pending(), "pending");
    let row = review_row(m);
    assert_eq!(row.blocked, None, "Export can repair it");
    assert_eq!(m.quick_export_note(), Some(NIF_NOT_RECORDED), "the reason");
}

/// Review round 3 (B4, B3, B7, ruling E6-N): export state comes from disk whenever a session meets a folder: the
/// record's ops, its source and its NIF against the installed one.
#[test]
fn export_state_is_read_from_disk_not_remembered() {
    table(&[
        ("restart after a partial export undone", || {
            let (d, mut m, p) = session_in_project("p", None);
            partial_y_undone_to_x(&d, &mut m);
            drop(m);
            let mut m = restart(&d, &p);
            m.open_edit();
            assert_eq!(vals(&m), [12.5], "premise: the draft holds X");
            pending_and_repairable(&m);
        }),
        ("Close edit and reopen", || {
            let (d, mut m, _p) = session_in_project("p", None);
            partial_y_undone_to_x(&d, &mut m);
            m.close_edit();
            m.open_edit();
            pending_and_repairable(&m);
        }),
        ("select another NIF and come back", || {
            let (d, mut m, _p) = session_in_project("p", None);
            partial_y_undone_to_x(&d, &mut m);
            let x = m.selected_nif.unwrap();
            m.select_nif(y_nif(&m));
            m.select_nif(x);
            m.open_edit();
            pending_and_repairable(&m);
        }),
        ("export folder A to B and back to A", || {
            let (d, mut m, _p) = session_in_project("p", None);
            partial_y_undone_to_x(&d, &mut m);
            let (a, b) = (export_root(&d), d.path().join(r"mods\Other"));
            std::fs::create_dir_all(&b).unwrap();
            m.set_nif_export_root(Some(b));
            m.set_nif_export_root(Some(a));
            pending_and_repairable(&m);
        }),
        (
            "a first export left partial and undone to no ops is blocked with its reason",
            || {
                let (d, mut m, _p) = session_in_project("p", None);
                m.open_edit();
                gloss(&mut m, 12.5);
                m.fail_record_write_after_nif_for_test();
                assert!(export(&mut m).is_err());
                assert!(exported(&d).is_file(), "premise: the NIF was written");
                assert!(m.undo_edit());
                let why = m.quick_export_block().unwrap_or_default();
                assert!(why.contains("holds an edit that was undone"), "{why}");
                assert_eq!(review_row(&m).blocked.as_deref(), Some(why.as_str()));
            },
        ),
        ("source S1 to S2 and back to S1 ends current", || {
            let (d, mut m, _p) = session_in_project("p", None);
            m.open_edit();
            gloss(&mut m, 12.5);
            assert_eq!(export(&mut m), Ok(1));
            let s1 = std::fs::read(d.path().join("mods/A/meshes/x.nif")).unwrap();
            let s2 = changed_source(&m);
            m.edit_with(|e| e.reload_source(s2).unwrap());
            assert!(m.edit.as_ref().unwrap().pending(), "premise: S2 is stale");
            m.edit_with(|e| e.reload_source(s1).unwrap());
            assert!(
                !m.edit.as_ref().unwrap().pending(),
                "S1 is what the record names"
            );
        }),
    ]);
}

/// Ruling E6-I: a partial export belongs to its folder: in another folder whose record is current, the session is not
/// pending.
#[test]
fn a_partial_export_does_not_follow_the_session_to_another_folder() {
    let (d, mut m, _p) = session_in_project("p", None);
    let (a, b) = (export_root(&d), d.path().join(r"mods\Other"));
    std::fs::create_dir_all(&b).unwrap();
    m.open_edit();
    gloss(&mut m, 12.5);
    assert_eq!(export(&mut m), Ok(1), "X recorded in A");
    m.set_nif_export_root(Some(b.clone()));
    assert_eq!(export(&mut m), Ok(1), "X recorded in B");
    m.set_nif_export_root(Some(a.clone()));
    gloss(&mut m, 13.5);
    m.fail_record_write_after_nif_for_test();
    let why = export(&mut m).unwrap_err();
    assert!(why.contains("is in place, but its record"), "{why}");
    assert!(m.undo_edit());
    assert!(m.edit.as_ref().unwrap().pending(), "premise: partial in A");
    m.set_nif_export_root(Some(b.clone()));
    assert!(
        !m.edit.as_ref().unwrap().pending(),
        "B's record is current for these ops"
    );
}

/// Review E6 (Important 7, ruling E6-E): when the NIF is written and its record is not, the export is not reported as
/// done: the export state stays where it was and the row stays pending with the reason. The next export of the same
/// edit replaces the NIF it wrote itself without asking (the bytes are the same).
#[test]
fn an_export_whose_record_fails_is_not_reported_as_done() {
    let (d, mut m, _p) = session_in_project("p", None);
    m.open_edit();
    gloss(&mut m, 12.5);
    let rec = nif_doc(&export_root(&d));
    m.fail_record_write_after_nif_for_test();
    let r = export(&mut m);
    let why = r.unwrap_err();
    assert!(why.contains("is in place, but its record"), "{why}");
    assert!(exported(&d).is_file(), "the NIF was written");
    let e = m.edit.as_ref().unwrap();
    assert_eq!(
        e.doc.export.sha256, None,
        "the export state is not advanced"
    );
    assert!(e.pending() && !e.up_to_date);
    assert!(
        !m.messages.iter().any(|l| l.starts_with("Exported ")),
        "{:?}",
        m.messages
    );
    assert_eq!(review_row(&m).foreign, None, "its own bytes: no question");
    assert_eq!(export(&mut m), Ok(1));
    assert!(load(&rec).unwrap().unwrap().export.sha256.is_some());
}

/// `doc` with its first op's value set to `v`.
fn with_value(mut doc: EditDoc, v: f32) -> EditDoc {
    if let Op::SetField { value, .. } = &mut doc.ops[0].op {
        *value = Value::F32(v.to_bits());
    }
    doc
}

/// Review round 2 (Critical B2, ruling E6-F): an equal draft id is not ownership. A copy B of project A (same id)
/// never replaces A's record when that record holds an op B's draft lacks.
#[test]
fn a_copied_project_never_replaces_the_record_of_its_original() {
    let (d, mut m) = export_session();
    let (a, b) = (project_folder(&d, "a"), project_folder(&d, "b2"));
    std::fs::remove_dir(&b).unwrap();
    m.set_project_root(Some(&a));
    m.open_edit();
    gloss(&mut m, 7.5);
    copy_dir(&a, &b);
    gloss(&mut m, 12.5);
    assert_eq!(export(&mut m), Ok(1));
    let rec = nif_doc(&export_root(&d));
    let before = std::fs::read(&rec).unwrap();
    m.set_project_root(Some(&b));
    assert_eq!(id_of(&b), id_of(&a), "premise: the copy keeps the id");
    gloss(&mut m, 9.5);
    assert!(review_row(&m).blocked.is_some(), "A's record holds 12.5");
    export_fails(&mut m, "holds other edits");
    assert_eq!(std::fs::read(&rec).unwrap(), before);
}

/// Review round 2 (Critical B3, ruling E6-F): a draft replaced on disk after it was opened (another edit) is not the
/// session's: leaving moves it aside before the session's edits are written.
#[test]
fn leaving_never_overwrites_a_draft_replaced_on_disk() {
    let (_d, mut m, p) = session_in_project("p", None);
    m.open_edit();
    gloss(&mut m, 12.5);
    let other = with_value(load(&draft(&p)).unwrap().unwrap(), 9.5);
    save(&draft(&p), &other).unwrap();
    let theirs = std::fs::read(draft(&p)).unwrap();
    m.close_edit();
    let aside = kept_beside(&draft(&p));
    assert_eq!(aside.len(), 1, "the other edit is kept aside: {aside:?}");
    assert_eq!(std::fs::read(&aside[0]).unwrap(), theirs);
    assert_eq!(
        doc_values(&draft(&p)),
        [12.5],
        "the session's edit is saved"
    );
}

/// Review round 2 (B1 table, ruling E6-F): the plain autosave a project session recovered is deleted after the
/// project save only while it holds the bytes the session loaded; other bytes at that name stay.
#[test]
fn a_reused_autosave_name_is_not_deleted() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    m.close_edit();
    let p = project_folder(&d, "p");
    m.set_project_root(Some(&p));
    m.open_edit();
    assert_eq!(
        op_values(&m)[0].1,
        12.5,
        "premise: the autosave is recovered"
    );
    let other = with_value(load(&autosave(&d)).unwrap().unwrap(), 9.5);
    save(&autosave(&d), &other).unwrap();
    let theirs = std::fs::read(autosave(&d)).unwrap();
    gloss(&mut m, 13.5);
    assert_eq!(doc_values(&draft(&p)), [12.5, 13.5]);
    assert_eq!(
        std::fs::read(autosave(&d)).unwrap(),
        theirs,
        "another session's autosave stays"
    );
}

/// Review round 2 (Important B2, ruling E6-F): moving the record that blocks the export away clears the block on the
/// next frame, without reopening the edit.
#[test]
fn moving_a_blocking_record_away_clears_the_block() {
    let (d, mut m) = export_session();
    old_record_in_export_folder(&d, &mut m, false);
    m.open_edit();
    gloss(&mut m, 12.5);
    assert!(review_row(&m).blocked.is_some(), "premise: blocked");
    let rec = nif_doc(&export_root(&d));
    std::fs::rename(&rec, d.path().join("moved.toml")).unwrap();
    let root = m.nif_export_root.clone();
    m.set_nif_export_root(root); // the app's next frame
    assert_eq!(review_row(&m).blocked, None, "the block is read again");
    // with the record gone, the NIF there is no longer recorded as Tessalith's (E6-E1): it asks, then exports
    answer(&mut m, GuardChoice::Overwrite);
    assert_eq!(export(&mut m), Ok(1));
}

/// Review round 3 (B6, ruling E6-O): a blocking record is read once per change of its file, also when it does not
/// load, and Export reads the record itself (no length-and-time shortcut).
#[test]
fn a_changed_record_is_read_once_and_export_reads_it_again() {
    table(&[
        (
            "a record replaced by one that does not load is read once and blocks with that",
            || {
                let (d, mut m) = export_session();
                old_record_in_export_folder(&d, &mut m, false);
                m.open_edit();
                gloss(&mut m, 12.5);
                assert!(review_row(&m).blocked.is_some(), "premise: blocked");
                std::fs::write(nif_doc(&export_root(&d)), b"not a document [[[").unwrap();
                let root = m.nif_export_root.clone();
                let before = load_count();
                for _ in 0..10 {
                    m.set_nif_export_root(root.clone()); // idle frames
                }
                assert_eq!(load_count() - before, 1, "read once");
                let why = review_row(&m).blocked.unwrap_or_default();
                assert!(why.contains("cannot be read"), "{why}");
            },
        ),
        (
            "a blocker replaced by a prefix of the same length and time does not block Export",
            || {
                let (d, mut m) = export_session();
                old_record_in_export_folder(&d, &mut m, false);
                m.open_edit();
                gloss(&mut m, 12.5);
                assert!(review_row(&m).blocked.is_some(), "premise: blocked");
                let rec = nif_doc(&export_root(&d));
                let (old, meta) = (
                    std::fs::read(&rec).unwrap(),
                    std::fs::metadata(&rec).unwrap(),
                );
                let mut empty = load(&rec).unwrap().unwrap();
                empty.ops.clear();
                let mut text = empty.to_text().into_bytes();
                assert!(text.len() < old.len(), "premise: shorter");
                text.push(b'#');
                text.resize(old.len(), b' ');
                std::fs::write(&rec, &text).unwrap();
                std::fs::File::options()
                    .write(true)
                    .open(&rec)
                    .unwrap()
                    .set_modified(meta.modified().unwrap())
                    .unwrap();
                assert!(
                    load(&rec).unwrap().unwrap().ops.is_empty(),
                    "premise: a valid prefix"
                );
                assert_eq!(export(&mut m), Ok(1));
            },
        ),
    ]);
}

/// Ruling E6-F: the record this session wrote stays its own after a carry-over changed its id, even when its ops no
/// longer start the session's (an undone export), which the prefix rule alone would refuse.
#[test]
fn the_record_a_session_wrote_stays_its_own_after_an_undo() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    assert_eq!(export(&mut m), Ok(1));
    let p = project_folder(&d, "p");
    m.set_project_root(Some(&p));
    assert!(m.undo_edit(), "premise: carried with its history");
    assert!(m.edit.as_ref().unwrap().doc.ops.is_empty());
    assert_eq!(review_row(&m).blocked, None);
    assert_eq!(export(&mut m), Ok(1));
    assert!(load(&nif_doc(&export_root(&d)))
        .unwrap()
        .unwrap()
        .ops
        .is_empty());
}

/// Project `p` whose draft (one op, 7.5, an hour old) lost at open to a newer kept autosave of its id holding `kept`
/// (written from the draft with `edit`); the session is open on the kept autosave. Returns (P, the kept path).
fn open_on_a_newer_kept_autosave(
    d: &tempfile::TempDir,
    m: &mut MeshesModel,
    edit: impl FnOnce(EditDoc) -> EditDoc,
) -> (std::path::PathBuf, std::path::PathBuf) {
    let p = project_folder(d, "p");
    project_with_a_draft(m, &p);
    let k = kept(d, &id_of(&p));
    save(&k, &edit(load(&draft(&p)).unwrap().unwrap())).unwrap();
    make_old(&draft(&p));
    m.open_edit();
    (p, k)
}

/// Review round 2 (Critical 1, ruling E6-G): the project cannot be written (its draft is held open by another
/// program), the kept autosave the session opened is removed and every op reverted. The draft still holds 7.5, which
/// the next open would load, so Select does not drop the session unsaved: here the kept autosave takes the empty state.
#[cfg(windows)]
#[test]
fn reverting_to_no_ops_over_a_draft_with_ops_is_saved_before_leaving() {
    use std::os::windows::fs::OpenOptionsExt as _;
    let (d, mut m) = export_session();
    let (p, k) = open_on_a_newer_kept_autosave(&d, &mut m, |doc| with_value(doc, 9.5));
    assert_eq!(op_values(&m)[0].1, 9.5, "premise: the kept autosave opened");
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(draft(&p))
        .unwrap();
    std::fs::remove_file(&k).unwrap();
    m.edit_with(|e| e.revert(0));
    let x = m.selected_nif.unwrap();
    m.select_nif(y_nif(&m));
    // Review round 3 (B5, ruling E6-K): checked before the unlock. The draft the next open picks cannot be read, so
    // the leave is refused even though its kept autosave save worked, and that save holds the revert.
    assert_eq!(
        m.selected_nif,
        Some(x),
        "the selection stays: the draft cannot be read"
    );
    assert!(m.edit.is_some());
    assert!(
        load(&k).unwrap().unwrap().ops.is_empty(),
        "the kept autosave holds the revert"
    );
    drop(lock);
    m.select_nif(x);
    m.open_edit();
    assert!(
        m.edit.as_ref().unwrap().doc.ops.is_empty(),
        "the revert holds: {:?}",
        op_values(&m)
    );
}

/// Review round 2 (Important B3, ruling E6-G): ops equal to the OLDER of two files are not durable when the newer
/// one would win at the next open. While neither file can be written (both held open, readable, by another program)
/// Close is refused; once they can, Close saves and the revert holds.
#[cfg(windows)]
#[test]
fn matching_the_older_draft_is_not_durable_when_a_newer_kept_autosave_wins() {
    use std::os::windows::fs::OpenOptionsExt as _;
    let (d, mut m) = export_session();
    let (p, k) = open_on_a_newer_kept_autosave(&d, &mut m, |doc| {
        let mut two = doc.clone();
        two.ops.push(with_value(doc, 9.5).ops.remove(0));
        two
    });
    assert_eq!(op_values(&m).len(), 2, "premise: the kept autosave opened");
    let hold = |f: &Path| {
        std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1) // FILE_SHARE_READ: readable, not movable, replaceable or deletable
            .open(f)
            .unwrap()
    };
    let locks = (hold(&draft(&p)), hold(&k));
    m.edit_with(|e| e.revert(1));
    m.close_edit();
    assert!(m.edit.is_some(), "the older draft is not what opens next");
    drop(locks);
    m.close_edit();
    m.open_edit();
    assert_eq!(
        op_values(&m).iter().map(|v| v.1).collect::<Vec<_>>(),
        [7.5],
        "the revert holds"
    );
    assert_eq!(doc_values(&draft(&p)), [7.5]);
}

/// The session's op values.
fn vals(m: &MeshesModel) -> Vec<f32> {
    op_values(m).iter().map(|v| v.1).collect()
}

/// Ruling E6-K's check after a leave: the session on `x` is still open with `want`, or a fresh open of `x` loads
/// `want` (the leave made those ops what the next open loads).
fn still_open_or_loads(m: &mut MeshesModel, x: Option<i64>, want: &[f32]) {
    if m.edit.is_some() && m.selected_nif == x {
        assert_eq!(vals(m), want, "still open with the current ops");
        return;
    }
    if m.selected_nif != x {
        m.select_nif(x.unwrap());
    }
    m.open_edit();
    assert_eq!(vals(m), want, "a fresh open loads the current ops");
}

/// Runs each named row; a failing row is reported by name and the others still run.
fn table(rows: &[(&str, fn())]) {
    let failed: Vec<&str> = rows
        .iter()
        .filter(|(_, row)| std::panic::catch_unwind(row).is_err())
        .map(|(name, _)| *name)
        .collect();
    assert!(failed.is_empty(), "rows failed: {failed:?}");
}

/// Holds `f` open the way another program can: readable, not replaceable, movable or deletable (`share` 1), or not
/// even readable (`share` 0).
#[cfg(windows)]
fn hold(f: &Path, share: u32) -> std::fs::File {
    use std::os::windows::fs::OpenOptionsExt as _;
    std::fs::OpenOptions::new()
        .read(true)
        .share_mode(share)
        .open(f)
        .unwrap()
}

/// Review round 3 (Critical 1 and round-one B3, ruling E6-K): a session is dropped only when, at the drop and after
/// the leave's saves, the next open would load its ops; when it cannot tell, it is not dropped. Each row ends with the
/// session still open or a fresh open loading the current ops.
#[cfg(windows)]
#[test]
fn a_session_is_dropped_only_when_the_next_open_loads_its_ops() {
    table(&[
        ("a save that worked earlier is no permission later", || {
            let (d, mut m) = export_session();
            let a = unsaveable_edit(&d, &mut m);
            let x = m.selected_nif;
            m.close_edit();
            assert!(m.edit.is_some(), "premise: refused");
            std::fs::remove_file(a.join("_sdc")).unwrap(); // the project folder is repaired
            gloss(&mut m, 13.5);
            assert!(draft(&a).exists(), "premise: the retry saved the draft");
            assert!(m.edit.is_some(), "premise: the session goes on");
            std::fs::remove_file(draft(&a)).unwrap();
            m.select_nif(y_nif(&m));
            still_open_or_loads(&mut m, x, &[12.5, 13.5]);
        }),
        (
            "a draft dated in the future wins over the kept autosave",
            || {
                let (d, mut m) = export_session();
                let p = project_folder(&d, "p");
                project_with_a_draft(&mut m, &p);
                let x = m.selected_nif;
                m.open_edit();
                let later = std::time::SystemTime::now() + std::time::Duration::from_secs(3600);
                std::fs::File::options()
                    .write(true)
                    .open(draft(&p))
                    .unwrap()
                    .set_modified(later)
                    .unwrap();
                let lock = hold(&draft(&p), 1);
                gloss(&mut m, 12.5);
                m.close_edit();
                drop(lock);
                still_open_or_loads(&mut m, x, &[7.5, 12.5]);
            },
        ),
        (
            "a draft with no id cannot lead the next open to the kept autosave",
            || {
                let (d, mut m) = export_session();
                let p = project_folder(&d, "p");
                project_with_a_draft(&mut m, &p);
                let x = m.selected_nif;
                let mut legacy = load(&draft(&p)).unwrap().unwrap();
                legacy.id = None;
                save(&draft(&p), &legacy).unwrap();
                m.open_edit();
                let lock = hold(&draft(&p), 1);
                gloss(&mut m, 12.5);
                m.close_edit();
                drop(lock);
                still_open_or_loads(&mut m, x, &[7.5, 12.5]);
            },
        ),
        (
            "a newer kept autosave that cannot be read is not an answer",
            || {
                let (d, mut m) = export_session();
                let (p, k) = open_on_a_newer_kept_autosave(&d, &mut m, |doc| {
                    let mut two = doc.clone();
                    two.ops.push(with_value(doc, 9.5).ops.remove(0));
                    two
                });
                let x = m.selected_nif;
                assert_eq!(vals(&m), [7.5, 9.5], "premise: the kept autosave opened");
                let draft_lock = hold(&draft(&p), 1);
                m.edit_with(|e| e.revert(1));
                let kept_lock = hold(&k, 0);
                m.close_edit();
                drop((draft_lock, kept_lock));
                still_open_or_loads(&mut m, x, &[7.5]);
            },
        ),
    ]);
}

/// Review round 3 (B2, ruling E6-H1; was `a_recovered_plain_autosave_is_durable_where_it_is` under E6-G): a project
/// session whose ops sit only in the plain autosave it recovered is not durable (that file is shared by every project),
/// so leaving writes the project's draft, keeps the old one aside and removes the autosave it took.
#[test]
fn a_recovered_plain_autosave_is_saved_into_the_project_on_leaving() {
    let (d, mut m) = export_session();
    let p = project_folder(&d, "p");
    let theirs = project_with_a_draft(&mut m, &p);
    make_old(&draft(&p));
    m.set_project_root(None);
    m.open_edit();
    gloss(&mut m, 12.5);
    m.close_edit();
    let mine = std::fs::read(autosave(&d)).unwrap();
    m.set_project_root(Some(&p));
    m.open_edit();
    assert_eq!(
        op_values(&m)[0].1,
        12.5,
        "premise: the autosave is recovered"
    );
    m.close_edit();
    assert!(m.edit.is_none(), "premise: closed");
    assert_eq!(doc_values(&draft(&p)), [12.5], "the project holds the edit");
    let aside = kept_beside(&draft(&p));
    assert_eq!(aside.len(), 1, "{aside:?}");
    assert_eq!(std::fs::read(&aside[0]).unwrap(), theirs);
    assert!(
        !autosave(&d).exists(),
        "the autosave it took is removed: {}",
        mine.len()
    );
}

/// Sets the modified time of `p` `secs` seconds back.
fn age(p: &Path, secs: u64) {
    let t = std::time::SystemTime::now() - std::time::Duration::from_secs(secs);
    std::fs::File::options()
        .write(true)
        .open(p)
        .unwrap()
        .set_modified(t)
        .unwrap();
}

/// Project P with its draft (7.5) and a kept autosave of its id (9.5), and a live session with no project whose edit
/// (12.5) is in the plain autosave. The files' ages are set by the caller. Returns (P, the kept path).
fn three_versions(
    d: &tempfile::TempDir,
    m: &mut MeshesModel,
) -> (std::path::PathBuf, std::path::PathBuf) {
    let p = project_folder(d, "p");
    project_with_a_draft(m, &p);
    let k = kept(d, &id_of(&p));
    save(&k, &with_value(load(&draft(&p)).unwrap().unwrap(), 9.5)).unwrap();
    m.set_project_root(None);
    m.open_edit();
    gloss(m, 12.5);
    (p, k)
}

/// Review round 2 (Important 4, ruling E6-H): P's draft < P's kept autosave < the live edit: the newest, the live
/// edit, goes into P; the two older files keep their bytes.
#[test]
fn the_newest_of_three_versions_is_carried_into_the_project() {
    let (d, mut m) = export_session();
    let (p, k) = three_versions(&d, &mut m);
    age(&draft(&p), 7200);
    age(&k, 3600);
    let (theirs, kept_bytes) = (
        std::fs::read(draft(&p)).unwrap(),
        std::fs::read(&k).unwrap(),
    );
    m.set_project_root(Some(&p));
    assert_eq!(
        op_values(&m).iter().map(|v| v.1).collect::<Vec<_>>(),
        [12.5],
        "the live edit is the newest"
    );
    assert_eq!(m.edit.as_ref().unwrap().draft_path, draft(&p));
    gloss(&mut m, 13.5);
    assert_eq!(doc_values(&draft(&p)), [12.5, 13.5]);
    let aside = kept_beside(&draft(&p));
    assert_eq!(aside.len(), 1, "{aside:?}");
    assert_eq!(std::fs::read(&aside[0]).unwrap(), theirs);
    let older = moved_aside_from(&k);
    assert_eq!(
        older.len(),
        1,
        "the older kept autosave is moved aside: {older:?}"
    );
    assert_eq!(std::fs::read(&older[0]).unwrap(), kept_bytes);
}

/// Ruling E6-H: when the project's side wins (here its kept autosave), Messages name the plain autosave the live edit
/// stays in.
#[test]
fn a_project_side_win_names_the_live_autosave_it_leaves() {
    let (d, mut m) = export_session();
    let (p, k) = three_versions(&d, &mut m);
    age(&draft(&p), 7200);
    age(&autosave(&d), 3600);
    let mine = std::fs::read(autosave(&d)).unwrap();
    m.messages.clear();
    m.set_project_root(Some(&p));
    assert_eq!(op_values(&m)[0].1, 9.5, "the kept autosave is the newest");
    assert!(
        m.messages.iter().any(|l| *l
            == format!(
                "Older edits of meshes/x.nif stay autosaved in {}.",
                autosave(&d).display()
            )),
        "{:?}",
        m.messages
    );
    assert_eq!(std::fs::read(autosave(&d)).unwrap(), mine);
    assert!(k.exists());
}

/// A session with no project and one edit (12.5), carried into a fresh project `p` and left there with no further
/// change. Returns `p`.
fn carried_into_p(d: &tempfile::TempDir, m: &mut MeshesModel) -> std::path::PathBuf {
    let p = project_folder(d, "p");
    m.open_edit();
    gloss(m, 12.5);
    m.set_project_root(Some(&p));
    assert_eq!(vals(m), [12.5], "premise: carried");
    p
}

/// Review round 3 (Critical B2, ruling E6-H1): a carried edit's home is the project's own files. After a carry with no
/// further change, whatever happens to the shared plain autosave, the project loads the carried ops.
#[test]
fn a_carried_edit_lives_in_the_project_whatever_the_plain_autosave_does() {
    table(&[
        ("quit and reopen the project", || {
            let (d, mut m) = export_session();
            carried_into_p(&d, &mut m);
            m.close_edit();
            m.open_edit();
            assert_eq!(vals(&m), [12.5]);
        }),
        (
            "open the NIF with no project, then the project again",
            || {
                let (d, mut m) = export_session();
                let p = carried_into_p(&d, &mut m);
                m.set_project_root(None);
                assert!(m.edit.is_some(), "premise: opened with no project");
                m.set_project_root(Some(&p));
                assert_eq!(vals(&m), [12.5]);
            },
        ),
        ("switch to another project and edit there", || {
            let (d, mut m) = export_session();
            let p = carried_into_p(&d, &mut m);
            m.set_project_root(Some(&project_folder(&d, "q")));
            gloss(&mut m, 50.0);
            m.set_project_root(Some(&p));
            assert_eq!(vals(&m), [12.5]);
        }),
        ("edit another NIF", || {
            let (d, mut m) = export_session();
            carried_into_p(&d, &mut m);
            let x = m.selected_nif.unwrap();
            m.select_nif(y_nif(&m));
            m.open_edit();
            gloss(&mut m, 50.0);
            m.select_nif(x);
            m.open_edit();
            assert_eq!(vals(&m), [12.5]);
        }),
        (
            "a carry whose draft cannot be written keeps the plain autosave and says why",
            || {
                let (d, mut m) = export_session();
                let p = project_folder(&d, "p");
                m.open_edit();
                gloss(&mut m, 12.5);
                let mine = std::fs::read(autosave(&d)).unwrap();
                std::fs::write(p.join("_sdc"), b"a file").unwrap(); // the project's draft cannot be written
                m.set_project_root(Some(&p));
                assert_eq!(vals(&m), [12.5], "the carried session stays");
                assert_eq!(
                    std::fs::read(autosave(&d)).unwrap(),
                    mine,
                    "the plain autosave stays"
                );
                let why = m.edit.as_ref().unwrap().export_block().unwrap_or_default();
                assert!(why.starts_with("Could not save the edits of"), "{why}");
                m.close_edit();
                assert!(m.edit.is_some(), "not durable in P: the leave is refused");
            },
        ),
        ("the plain autosave is deleted", || {
            let (d, mut m) = export_session();
            carried_into_p(&d, &mut m);
            m.close_edit();
            if autosave(&d).exists() {
                std::fs::remove_file(autosave(&d)).unwrap();
            }
            m.open_edit();
            assert_eq!(vals(&m), [12.5]);
        }),
    ]);
}

/// Review round 2 (Important B5, ruling E6-H): redo history alone writes nothing into a project: the carried
/// session's ops (none) are what the project would load (the empty autosave), so no draft is made and the autosave
/// stays.
#[test]
fn redo_history_alone_writes_nothing_into_a_project() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    assert!(m.undo_edit());
    let empty = std::fs::read(autosave(&d)).unwrap();
    let p = project_folder(&d, "p");
    m.set_project_root(Some(&p));
    let e = m.edit.as_ref().unwrap();
    assert!(
        e.history.can_redo(),
        "premise: the session is carried with its redo"
    );
    assert!(!draft(&p).exists(), "no draft for history alone");
    assert!(!p.join("_sdc").exists());
    assert_eq!(
        std::fs::read(autosave(&d)).unwrap(),
        empty,
        "the autosave stays"
    );
}

/// Sets the modified time of `p` to that of `like`.
fn same_time(p: &Path, like: &Path) {
    let t = std::fs::metadata(like).unwrap().modified().unwrap();
    std::fs::File::options()
        .write(true)
        .open(p)
        .unwrap()
        .set_modified(t)
        .unwrap();
}

/// Project `p` with its own draft of x.nif (7.5), then a session with no project with one edit (12.5) saved in the
/// plain autosave. Returns (P's draft bytes, the autosave's bytes).
fn live_edit_beside_a_project_draft(
    d: &tempfile::TempDir,
    m: &mut MeshesModel,
    p: &Path,
) -> (Vec<u8>, Vec<u8>) {
    let theirs = project_with_a_draft(m, p);
    m.set_project_root(None);
    m.open_edit();
    gloss(m, 12.5);
    (theirs, std::fs::read(autosave(d)).unwrap())
}

/// Review E6 (Important 4, ruling E6-D): opening a project whose draft is newer than the live edit (in its autosave)
/// opens the project's draft, as `open` would; the live edit stays in the autosave, and neither file is written.
#[test]
fn opening_a_project_with_a_newer_draft_opens_that_draft() {
    let (d, mut m) = export_session();
    let p = project_folder(&d, "p");
    let (theirs, mine) = live_edit_beside_a_project_draft(&d, &mut m, &p);
    make_old(&autosave(&d));
    let mine_time = std::fs::metadata(autosave(&d)).unwrap().modified().unwrap();
    m.messages.clear();
    m.set_project_root(Some(&p));
    assert_eq!(
        op_values(&m).iter().map(|v| v.1).collect::<Vec<_>>(),
        [7.5],
        "the newer draft wins"
    );
    assert_eq!(std::fs::read(draft(&p)).unwrap(), theirs);
    assert_eq!(std::fs::read(autosave(&d)).unwrap(), mine);
    assert_eq!(
        std::fs::metadata(autosave(&d)).unwrap().modified().unwrap(),
        mine_time,
        "the autosave was not written"
    );
    assert!(
        m.messages
            .iter()
            .any(|l| l.starts_with("Older edits of meshes/x.nif stay autosaved in ")),
        "{:?}",
        m.messages
    );
}

/// Ruling E6-D, Note GD24: at equal times the project's draft wins, as at open.
#[test]
fn opening_a_project_with_a_draft_of_the_same_time_opens_that_draft() {
    let (d, mut m) = export_session();
    let p = project_folder(&d, "p");
    let (theirs, mine) = live_edit_beside_a_project_draft(&d, &mut m, &p);
    same_time(&autosave(&d), &draft(&p));
    m.set_project_root(Some(&p));
    assert_eq!(op_values(&m).iter().map(|v| v.1).collect::<Vec<_>>(), [7.5]);
    assert_eq!(std::fs::read(draft(&p)).unwrap(), theirs);
    assert_eq!(std::fs::read(autosave(&d)).unwrap(), mine);
}

/// Ruling E6-D: the project's kept autosave of its draft's id takes part, as at open: newer than the draft, it opens.
#[test]
fn opening_a_project_checks_its_kept_autosave() {
    let (d, mut m) = export_session();
    let p = project_folder(&d, "p");
    let (theirs, mine) = live_edit_beside_a_project_draft(&d, &mut m, &p);
    let id = id_of(&p);
    let mut k = load(&draft(&p)).unwrap().unwrap();
    if let Op::SetField { value, .. } = &mut k.ops[0].op {
        *value = Value::F32(9.5f32.to_bits());
    }
    save(&kept(&d, &id), &k).unwrap();
    make_old(&draft(&p));
    make_old(&autosave(&d));
    m.set_project_root(Some(&p));
    assert_eq!(
        op_values(&m).iter().map(|v| v.1).collect::<Vec<_>>(),
        [9.5],
        "the kept autosave is the newest"
    );
    assert_eq!(std::fs::read(draft(&p)).unwrap(), theirs);
    assert_eq!(
        std::fs::read(autosave(&d)).unwrap(),
        mine,
        "the live edit stays"
    );
}

/// Ruling E6-D: a carried edit takes the project draft's id (so its kept autosave and a renamed project still
/// match), and keeps its own when the project has no draft.
#[test]
fn a_carried_edit_takes_the_project_drafts_id() {
    let (d, mut m) = export_session();
    let p = project_folder(&d, "p");
    live_edit_beside_a_project_draft(&d, &mut m, &p);
    let theirs = id_of(&p);
    make_old(&draft(&p));
    m.set_project_root(Some(&p));
    assert_eq!(op_values(&m)[0].1, 12.5, "premise: the live edit is newer");
    assert_eq!(
        m.edit.as_ref().unwrap().doc.id.as_deref(),
        Some(theirs.as_str()),
        "the carried session takes P's id"
    );
    // ruling E6-H1: the carry saves into P, under P's id
    assert_eq!(doc_values(&draft(&p)), [12.5]);
    assert_eq!(id_of(&p), theirs);
    // With no draft in Q, the plain autosave's id moves with its ops.
    let q = project_folder(&d, "q");
    m.set_project_root(None);
    gloss(&mut m, 13.5);
    let live = load(&autosave(&d)).unwrap().unwrap().id.unwrap();
    m.set_project_root(Some(&q));
    assert_eq!(vals(&m), [13.5], "premise: carried");
    assert_eq!(
        m.edit.as_ref().unwrap().doc.id.as_deref(),
        Some(live.as_str()),
        "the autosave keeps its id"
    );
    assert_eq!(id_of(&q), live);
}

/// Ruling E6-D: a kept autosave older than the project's draft does not open, and the next save moves it to
/// `<sha>.<id>.<secs>.toml`, bytes unchanged.
#[test]
fn an_older_kept_autosave_is_moved_aside_by_the_next_save() {
    let (d, mut m) = export_session();
    let p = project_folder(&d, "p");
    project_with_a_draft(&mut m, &p);
    let id = id_of(&p);
    let mut k = load(&draft(&p)).unwrap().unwrap();
    if let Op::SetField { value, .. } = &mut k.ops[0].op {
        *value = Value::F32(9.5f32.to_bits());
    }
    save(&kept(&d, &id), &k).unwrap();
    make_old(&kept(&d, &id));
    let old = std::fs::read(kept(&d, &id)).unwrap();
    m.open_edit();
    assert_eq!(op_values(&m)[0].1, 7.5, "the newer draft opens");
    gloss(&mut m, 13.5);
    assert!(!kept(&d, &id).exists(), "its name is free again");
    let aside = moved_aside_from(&kept(&d, &id));
    assert_eq!(aside.len(), 1, "{aside:?}");
    assert_eq!(std::fs::read(&aside[0]).unwrap(), old);
}

/// Review round 3 (B1, ruling E6-L): a fingerprint belongs to ops the session adopted or wrote; a file with other bytes
/// at a remembered name is not the session's to delete or move.
#[test]
fn a_file_is_cleaned_up_only_while_it_holds_the_ops_the_session_took() {
    table(&[
        ("a carry leaves another session's plain autosave", || {
            let (d, mut m) = export_session();
            let p = project_folder(&d, "p");
            m.set_nif_export_root(None);
            m.open_edit();
            gloss(&mut m, 12.5);
            read_only(&autosave(&d), true); // the plain autosave cannot be replaced: Close is refused
            gloss(&mut m, 13.5);
            m.close_edit();
            assert!(m.edit.is_some(), "premise: Close refused");
            read_only(&autosave(&d), false);
            gloss(&mut m, 14.5); // the retry saves
            let other = with_value(load(&autosave(&d)).unwrap().unwrap(), 99.5);
            save(&autosave(&d), &other).unwrap(); // another session's edit at the same name
            let theirs = std::fs::read(autosave(&d)).unwrap();
            m.set_project_root(Some(&p));
            let kept_somewhere = tomls_under(&d.path().join("nif-edits"))
                .iter()
                .any(|f| std::fs::read(f).unwrap() == theirs);
            assert!(kept_somewhere, "the other session's edit still exists");
        }),
        (
            "a carry leaves a plain autosave with the same ops in other bytes",
            || {
                // added after mutation l survived: with equal ops no save runs before the carry, so the bytes the carry
                // reads are not the session's; only the fingerprint tells them apart
                let (d, mut m) = export_session();
                let p = project_folder(&d, "p");
                m.set_nif_export_root(None);
                m.open_edit();
                gloss(&mut m, 12.5);
                let mut other = load(&autosave(&d)).unwrap().unwrap();
                other.id = Some("0123456789abcdef".into()); // another session's document of the same ops
                save(&autosave(&d), &other).unwrap();
                let theirs = std::fs::read(autosave(&d)).unwrap();
                m.set_project_root(Some(&p));
                assert_eq!(doc_values(&draft(&p)), [12.5], "premise: carried and saved");
                assert_eq!(
                    std::fs::read(autosave(&d)).unwrap(),
                    theirs,
                    "not the session's to delete"
                );
            },
        ),
        (
            "a kept autosave that lost and was replaced since is not moved",
            || {
                let (d, mut m) = export_session();
                let p = project_folder(&d, "p");
                project_with_a_draft(&mut m, &p);
                let k = kept(&d, &id_of(&p));
                save(&k, &with_value(load(&draft(&p)).unwrap().unwrap(), 9.5)).unwrap();
                make_old(&k);
                m.open_edit();
                assert_eq!(vals(&m), [7.5], "premise: the draft won");
                save(&k, &with_value(load(&draft(&p)).unwrap().unwrap(), 99.5)).unwrap();
                let replaced = std::fs::read(&k).unwrap();
                gloss(&mut m, 12.5);
                assert_eq!(
                    std::fs::read(&k).unwrap(),
                    replaced,
                    "the replacement stays where it is"
                );
                assert!(moved_aside_from(&k).is_empty());
            },
        ),
    ]);
}

/// Sets the modified time of `p` `secs` seconds ahead of now (a file dated in the future).
fn ahead(p: &Path, secs: u64) {
    let t = std::time::SystemTime::now() + std::time::Duration::from_secs(secs);
    std::fs::File::options()
        .write(true)
        .open(p)
        .unwrap()
        .set_modified(t)
        .unwrap();
}

/// Project `p` with one edit (12.5) saved in its draft by the open session. Returns (d, m, p).
fn saved_in_p() -> (tempfile::TempDir, MeshesModel, std::path::PathBuf) {
    let (d, mut m, p) = session_in_project("p", None);
    m.open_edit();
    gloss(&mut m, 12.5);
    assert_eq!(doc_values(&draft(&p)), [12.5], "premise: saved in P");
    (d, m, p)
}

/// Review round 4 (Critical "Rename P to P2 ..." and Important "deny metadata access ...", ruling E6-P): a leave is
/// allowed only where the next open loads the live ops: the session's home, or a switch destination whose draft carries
/// this session's draft id, checked as the home is (its own files, then the selection with the plain autosave). Each
/// row's first words are the review's sequence.
#[test]
fn a_leave_is_allowed_only_where_the_next_open_loads_the_ops() {
    let mut rows: Vec<(&str, fn())> = vec![
        (
            "rename P to P2, leave P's draft holding X, put a newer plain autosave holding Y, open P2",
            || {
                let (d, mut m, p) = saved_in_p();
                make_old(&draft(&p));
                let p2 = d.path().join("p2");
                std::fs::rename(&p, &p2).unwrap();
                let y = with_value(load(&draft(&p2)).unwrap().unwrap(), 99.5);
                save(&autosave(&d), &y).unwrap();
                age(&autosave(&d), 60); // newer than P's draft, older than anything the switch writes
                m.set_project_root(Some(&p2));
                assert_eq!(
                    vals(&m),
                    [12.5],
                    "the switch is refused or P2 loads the live ops, never Y"
                );
            },
        ),
        (
            "a destination whose draft has the same ops under another id is not the session's place",
            || {
                let (d, mut m, p) = saved_in_p();
                let q = project_folder(&d, "q");
                let mut theirs = load(&draft(&p)).unwrap().unwrap();
                theirs.id = Some("0123456789abcdef".into());
                save(&draft(&q), &theirs).unwrap();
                let id = id_of(&p);
                std::fs::remove_dir_all(&p).unwrap();
                m.set_project_root(Some(&q));
                assert_eq!(
                    m.edit.as_ref().unwrap().draft_path,
                    draft(&p),
                    "the home decides: the session stays"
                );
                assert_eq!(doc_values(&kept(&d, &id)), [12.5], "kept under its own id");
            },
        ),
        (
            "a no-project destination is not the session's place, whatever the plain autosave holds",
            || {
                let (d, mut m, p) = saved_in_p();
                std::fs::create_dir_all(autosave(&d).parent().unwrap()).unwrap();
                std::fs::copy(draft(&p), autosave(&d)).unwrap(); // the shared file holds the same ops
                let id = id_of(&p);
                std::fs::remove_dir_all(&p).unwrap();
                m.set_project_root(None);
                assert_eq!(
                    m.edit.as_ref().unwrap().draft_path,
                    draft(&p),
                    "the home decides: the session stays"
                );
                assert_eq!(doc_values(&kept(&d, &id)), [12.5], "kept under its own id");
            },
        ),
    ];
    rows.extend(windows_rows_p());
    table(&rows);
}

/// E6-P's rows that need Windows (a junction).
#[cfg(windows)]
fn windows_rows_p() -> Vec<(&'static str, fn())> {
    vec![(
        "open the newer kept edit, revert to the older draft's ops while the draft cannot be replaced, deny metadata \
         access to the kept directory, then Close and restore access",
        a_kept_autosave_that_cannot_be_checked_is_not_absent,
    )]
}

#[cfg(not(windows))]
fn windows_rows_p() -> Vec<(&'static str, fn())> {
    vec![]
}

/// Points `link` at `target` with a directory junction (no privilege needed). A junction at its own path is a loop:
/// every path through it fails with "cannot be resolved" (error 1921), not "not found".
#[cfg(windows)]
fn junction(link: &Path, target: &Path) {
    let ok = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()
        .unwrap()
        .status
        .success();
    assert!(ok, "mklink /J {}", link.display());
}

/// E6-P's unknown row: the kept autosave's folder cannot be checked (a junction loop stands in for denied metadata
/// access: an ACL deny does not stop Windows reading a file's attributes), so whether the newer kept edit is there is
/// unknown, and unknown is not absent.
#[cfg(windows)]
fn a_kept_autosave_that_cannot_be_checked_is_not_absent() {
    let (d, mut m) = export_session();
    let (p, k) = open_on_a_newer_kept_autosave(&d, &mut m, |doc| {
        let mut two = doc.clone();
        two.ops.push(with_value(doc, 9.5).ops.remove(0));
        two
    });
    assert_eq!(vals(&m), [7.5, 9.5], "premise: the kept autosave opened");
    let lock = hold(&draft(&p), 1);
    m.edit_with(|e| e.revert(1));
    assert_eq!(vals(&m), [7.5], "premise: reverted to the draft's ops");
    let (dir, real) = (d.path().join("nif-edits"), d.path().join("nif-edits-real"));
    std::fs::rename(&dir, &real).unwrap();
    junction(&dir, &dir);
    let unknown = std::fs::metadata(&k).is_err_and(|e| e.kind() != std::io::ErrorKind::NotFound);
    m.close_edit();
    let refused = m.edit.is_some();
    std::fs::remove_dir(&dir).unwrap(); // the junction only
    std::fs::rename(&real, &dir).unwrap();
    drop(lock);
    assert!(unknown, "premise: the kept autosave cannot be checked");
    assert!(refused, "Close is refused while it cannot tell");
    m.close_edit();
    assert!(m.edit.is_none(), "with access back, Close saves and ends");
    m.open_edit();
    assert_eq!(vals(&m), [7.5], "the reverted op is not restored");
}

/// Review round 4 (Critical "Export X in A, copy that record to export folder B ...", Important "Open P with an older
/// kept loser ..." and "Give kept, draft and plain successively later future timestamps ...", ruling E6-Q): a
/// remembered fingerprint belongs to the one pathname it was taken from, and a remembered loser is kept.
#[test]
fn ownership_is_of_one_pathname() {
    let mut rows: Vec<(&str, fn())> = vec![
        (
            "export X in A, copy that record to export folder B, add Y, select B, undo Y and X, then Export",
            || {
                let (d, mut m, _p) = saved_in_p();
                let (a, b) = (export_root(&d), d.path().join(r"mods\Other"));
                assert_eq!(export(&mut m), Ok(1), "X recorded in A");
                std::fs::create_dir_all(nif_doc(&b).parent().unwrap()).unwrap();
                std::fs::copy(nif_doc(&a), nif_doc(&b)).unwrap();
                let theirs = std::fs::read(nif_doc(&b)).unwrap();
                gloss(&mut m, 13.5);
                m.set_nif_export_root(Some(b.clone()));
                assert!(m.undo_edit() && m.undo_edit());
                assert!(m.edit.as_ref().unwrap().doc.ops.is_empty(), "premise: no ops");
                let r = export(&mut m);
                assert_eq!(
                    std::fs::read(nif_doc(&b)).unwrap(),
                    theirs,
                    "B's record still has its bytes: {r:?}"
                );
                assert!(!b.join(r"meshes\x.nif").exists(), "no NIF in B: {r:?}");
                let why = r.unwrap_err();
                assert!(why.contains("holds other edits"), "the export says why: {why}");
            },
        ),
        (
            "give kept, draft and plain successively later future timestamps, then carry the winning plain edit into \
             P and quit",
            || {
                let (d, mut m) = export_session();
                let (p, k) = three_versions(&d, &mut m);
                ahead(&k, 3600);
                ahead(&draft(&p), 7200);
                ahead(&autosave(&d), 10800);
                m.set_project_root(Some(&p));
                assert_eq!(vals(&m), [12.5], "premise: the live edit is the newest");
                drop(m); // quit
                let mut m = restart(&d, &p);
                m.open_edit();
                assert_eq!(vals(&m), [12.5], "the carried ops load");
            },
        ),
    ];
    rows.extend(windows_rows_q());
    table(&rows);
}

/// E6-Q's rows that need Windows (a share-mode lock).
#[cfg(windows)]
fn windows_rows_q() -> Vec<(&'static str, fn())> {
    vec![
        (
            "open P with an older kept loser, replace the loser with another edit, make P's draft unreplaceable, edit \
             and Close",
            a_replaced_kept_loser_stays_at_its_name,
        ),
        (
            "the same, with one edit saved into the draft before the draft becomes unreplaceable",
            a_replaced_kept_loser_stays_remembered_after_a_draft_save,
        ),
    ]
}

/// E6-Q's cleanup row: a draft save that finds the loser replaced leaves it and keeps it remembered, so the later kept
/// fallback still compares it (forgetting it let the fallback move the replacement aside by a second route).
#[cfg(windows)]
fn a_replaced_kept_loser_stays_remembered_after_a_draft_save() {
    let (d, mut m) = export_session();
    let p = project_folder(&d, "p");
    project_with_a_draft(&mut m, &p);
    let k = kept(&d, &id_of(&p));
    save(&k, &with_value(load(&draft(&p)).unwrap().unwrap(), 9.5)).unwrap();
    make_old(&k);
    m.open_edit();
    assert_eq!(vals(&m), [7.5], "premise: the draft won");
    save(&k, &with_value(load(&draft(&p)).unwrap().unwrap(), 99.5)).unwrap();
    let replaced = std::fs::read(&k).unwrap();
    gloss(&mut m, 12.5);
    assert_eq!(
        doc_values(&draft(&p)),
        [7.5, 12.5],
        "premise: the draft saved"
    );
    let lock = hold(&draft(&p), 1);
    gloss(&mut m, 13.5);
    m.close_edit();
    let (bytes, aside, open) = (
        std::fs::read(&k).unwrap(),
        moved_aside_from(&k),
        m.edit.is_some(),
    );
    drop(lock);
    assert_eq!(bytes, replaced, "its bytes are still at its name");
    assert!(aside.is_empty(), "nothing moved aside: {aside:?}");
    assert!(open, "the leave is refused: nowhere took the edit");
}

#[cfg(not(windows))]
fn windows_rows_q() -> Vec<(&'static str, fn())> {
    vec![]
}

/// E6-Q's kept-fallback row: the fallback writer compares the remembered loser for its pathname, as the cleanup does.
#[cfg(windows)]
fn a_replaced_kept_loser_stays_at_its_name() {
    let (d, mut m) = export_session();
    let p = project_folder(&d, "p");
    project_with_a_draft(&mut m, &p);
    let k = kept(&d, &id_of(&p));
    save(&k, &with_value(load(&draft(&p)).unwrap().unwrap(), 9.5)).unwrap();
    make_old(&k);
    m.open_edit();
    assert_eq!(vals(&m), [7.5], "premise: the draft won");
    save(&k, &with_value(load(&draft(&p)).unwrap().unwrap(), 99.5)).unwrap();
    let replaced = std::fs::read(&k).unwrap();
    let lock = hold(&draft(&p), 1);
    gloss(&mut m, 12.5);
    m.close_edit();
    let (bytes, aside, open) = (
        std::fs::read(&k).unwrap(),
        moved_aside_from(&k),
        m.edit.is_some(),
    );
    drop(lock);
    assert_eq!(bytes, replaced, "its bytes are still at its name");
    assert!(aside.is_empty(), "nothing moved aside: {aside:?}");
    assert!(open, "the leave is refused: nowhere took the edit");
}

/// Review round 4 (Critical "Edit without a project, make plain unwritable, request fresh P ..." and Important "Carry
/// X into P with `_sdc` blocked by a file ...", ruling E6-R): every route by which a no-project edit enters P runs the
/// one carry, and its failure stays until P's own files would load the live ops.
#[test]
fn every_route_into_a_project_runs_the_one_carry() {
    table(&[
        (
            "edit without a project, make plain unwritable, request fresh P, restore write access, and make one more \
             edit",
            || {
                let (d, mut m) = export_session();
                let p = project_folder(&d, "p");
                m.open_edit();
                gloss(&mut m, 12.5);
                read_only(&autosave(&d), true);
                gloss(&mut m, 13.5); // not saved
                m.set_project_root(Some(&p));
                assert_eq!(
                    m.edit.as_ref().unwrap().draft_path,
                    autosave(&d),
                    "premise: the switch waits"
                );
                read_only(&autosave(&d), false);
                gloss(&mut m, 14.5);
                assert_eq!(
                    doc_values(&draft(&p)),
                    [12.5, 13.5, 14.5],
                    "P's draft holds the edit after the retry"
                );
                if autosave(&d).exists() {
                    std::fs::remove_file(autosave(&d)).unwrap();
                }
                drop(m); // quit
                let mut m = restart(&d, &p);
                m.open_edit();
                assert_eq!(vals(&m), [12.5, 13.5, 14.5], "reopening P loads it");
            },
        ),
        (
            "carry X into P with _sdc blocked by a file, leave the obstruction in place, make edit Y, then quit and \
             reopen P",
            || {
                let (d, mut m) = export_session();
                let p = project_folder(&d, "p");
                m.open_edit();
                gloss(&mut m, 12.5);
                std::fs::write(p.join("_sdc"), b"a file").unwrap();
                m.set_project_root(Some(&p));
                assert_eq!(vals(&m), [12.5], "premise: carried, P's draft not written");
                gloss(&mut m, 13.5);
                let why = m.edit.as_ref().unwrap().export_block().unwrap_or_default();
                assert!(
                    why.starts_with("Could not save the edits of"),
                    "the reason is still shown: {why}"
                );
                assert_eq!(m.quick_export_block().as_deref(), Some(why.as_str()));
                std::fs::remove_file(p.join("_sdc")).unwrap();
                gloss(&mut m, 14.5);
                assert_eq!(
                    doc_values(&draft(&p)),
                    [12.5, 13.5, 14.5],
                    "the next save writes P's draft"
                );
                assert!(!m
                    .edit
                    .as_ref()
                    .unwrap()
                    .export_block()
                    .is_some_and(|w| w.starts_with("Could not save the edits of")));
                drop(m); // quit
                let mut m = restart(&d, &p);
                m.open_edit();
                assert_eq!(vals(&m), [12.5, 13.5, 14.5], "reopening P loads X and Y");
            },
        ),
        (
            "a refused Close whose retry reaches only the kept autosave keeps its reason (nothing waits)",
            || {
                let (d, mut m, a) = session_in_project("a", None);
                m.open_edit();
                std::fs::write(a.join("_sdc"), b"a file").unwrap(); // the project's draft cannot be written
                gloss(&mut m, 12.5);
                m.close_edit();
                assert!(m.edit.is_some(), "premise: Close refused");
                gloss(&mut m, 13.5);
                let id = m.edit.as_ref().unwrap().doc.id.clone().unwrap();
                assert_eq!(doc_values(&kept(&d, &id)), [12.5, 13.5], "premise: kept only");
                let why = m.edit.as_ref().unwrap().export_block().unwrap_or_default();
                assert!(
                    why.starts_with("Could not save the edits of"),
                    "the reason is still shown: {why}"
                );
            },
        ),
    ]);
}

/// Review round 4 (Critical "Export X, undo to zero, move its record away ..." and Important "Produce drift in A, then
/// choose an empty B ...", ruling E6-S): Export decides on the record and the installed NIF it reads then; derived
/// export state belongs to the present folder's files only.
#[test]
fn export_decides_on_what_it_reads_then() {
    table(&[
        (
            "export X, undo to zero, move its record away while the session stays open, then click Export",
            || {
                let (d, mut m, _p) = saved_in_p();
                assert_eq!(export(&mut m), Ok(1));
                let nif_x = std::fs::read(exported(&d)).unwrap();
                assert!(m.undo_edit());
                let rec = nif_doc(&export_root(&d));
                std::fs::rename(&rec, d.path().join("moved.toml")).unwrap();
                m.messages.clear();
                let r = export(&mut m);
                assert_eq!(
                    std::fs::read(exported(&d)).unwrap(),
                    nif_x,
                    "the NIF is not replaced: {r:?}"
                );
                assert!(!rec.exists(), "no empty record is written: {r:?}");
                assert!(
                    !m.messages.iter().any(|l| l.starts_with("Exported ")),
                    "{:?}",
                    m.messages
                );
            },
        ),
        (
            "with the record in place, replace the installed NIF and answer Overwrite with zero ops",
            || {
                let (d, mut m, _p) = saved_in_p();
                assert_eq!(export(&mut m), Ok(1));
                assert!(m.undo_edit());
                std::fs::write(exported(&d), b"another program's NIF").unwrap();
                answer(&mut m, GuardChoice::Overwrite);
                let r = export(&mut m);
                assert_eq!(
                    std::fs::read(exported(&d)).unwrap(),
                    b"another program's NIF",
                    "not replaced: {r:?}"
                );
                let why = r.unwrap_err();
                assert!(why.contains("holds an edit that was undone"), "blocked: {why}");
            },
        ),
        (
            "produce drift in A, then choose an empty B with no record or installed NIF",
            || {
                let (d, mut m, _p) = session_in_project("p", None);
                partial_y_undone_to_x(&d, &mut m);
                assert_eq!(m.quick_export_note(), Some(NIF_NOT_RECORDED), "premise: drift in A");
                let b = d.path().join(r"mods\Other");
                std::fs::create_dir_all(&b).unwrap();
                m.set_nif_export_root(Some(b));
                assert_eq!(m.quick_export_note(), None, "no hover from A");
                assert!(m.undo_edit(), "to zero ops in B");
                let why = m.quick_export_block().unwrap_or_default();
                assert!(!why.contains("undone"), "no block from A: {why}");
                let row = m.nif_review_rows().first().and_then(|r| r.blocked.clone());
                assert!(!row.is_some_and(|w| w.contains("undone")), "no row block from A");
            },
        ),
    ]);
}

// ---- Review round 5 ----

/// Review round 5 (Critical "Save X in P, rename P to P2, undo X ..." and Important "put an older autosave at the kept
/// or plain name, deny reads of its contents ...", rulings E6-P2, E6-P3): a home that is gone, and a candidate whose
/// contents cannot be read, are "cannot tell", never "no edits" or "older".
#[test]
fn a_vanished_home_or_an_unreadable_candidate_cannot_tell() {
    let mut rows: Vec<(&str, fn())> =
        vec![(
        "save X in P, rename P to P2, undo X while the editor still points at P, then select P2",
        || {
            let (d, mut m, p) = saved_in_p();
            let p2 = d.path().join("p2");
            std::fs::rename(&p, &p2).unwrap();
            assert!(m.undo_edit());
            assert!(vals(&m).is_empty(), "premise: undone");
            m.set_project_root(Some(&p2));
            assert!(vals(&m).is_empty(), "refused, or P2 loads the undo: never X");
            m.close_edit();
            if m.edit.is_none() {
                m.open_edit();
            }
            assert!(vals(&m).is_empty(), "the next open loads the undo");
        },
    )];
    rows.extend(windows_rows_p3());
    table(&rows);
}

/// E6-P3's rows that need Windows (a share-mode lock that leaves attributes readable and contents not).
#[cfg(windows)]
fn windows_rows_p3() -> Vec<(&'static str, fn())> {
    vec![
        (
            "save X in P, put an older autosave at the kept name, deny reads of its contents, then Close",
            || older_candidate_unreadable(true),
        ),
        (
            "save X in P, put an older autosave at the plain name, deny reads of its contents, then Close",
            || older_candidate_unreadable(false),
        ),
    ]
}

#[cfg(not(windows))]
fn windows_rows_p3() -> Vec<(&'static str, fn())> {
    vec![]
}

/// E6-P3's row: an older candidate (the kept autosave of the draft's id, or the plain autosave) whose contents cannot
/// be read keeps the session open until it can be read.
#[cfg(windows)]
fn older_candidate_unreadable(at_kept: bool) {
    let (d, mut m, p) = saved_in_p();
    let f = match at_kept {
        true => kept(&d, &id_of(&p)),
        false => autosave(&d),
    };
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    save(&f, &with_value(load(&draft(&p)).unwrap().unwrap(), 99.5)).unwrap();
    age(&f, 3600);
    let lock = hold(&f, 0);
    let attributes = std::fs::metadata(&f).is_ok();
    m.close_edit();
    let refused = m.edit.is_some();
    drop(lock);
    assert!(attributes, "premise: its attributes are readable");
    assert!(refused, "Close is refused while it cannot be read");
    m.close_edit();
    assert!(m.edit.is_none(), "once it can be read, Close ends");
    m.open_edit();
    assert_eq!(vals(&m), [12.5], "the older autosave does not open");
}

/// Review round 5 (Important "Open a future-dated draft with an older kept loser, lock the draft against replacement,
/// edit, attempt Close, then edit again", ruling E6-Q2): once the session's kept file is published at the loser's name,
/// the loser it displaced is retired; the session's own file there is replaced by its next save.
#[cfg(windows)]
#[test]
fn a_loser_the_session_displaced_is_retired() {
    table(&[
        (
            "open a future-dated draft with an older kept loser, lock the draft against replacement, edit, attempt \
             Close, then edit again",
            || displaced_loser(false),
        ),
        (
            "the same with the draft replaced by another program before the lock",
            || displaced_loser(true),
        ),
    ]);
}

/// E6-Q2's rows; `foreign_draft`: the draft holds bytes the session never loaded, so its save stops before the draft.
#[cfg(windows)]
fn displaced_loser(foreign_draft: bool) {
    let (d, mut m) = export_session();
    let p = project_folder(&d, "p");
    project_with_a_draft(&mut m, &p);
    let k = kept(&d, &id_of(&p));
    save(&k, &with_value(load(&draft(&p)).unwrap().unwrap(), 9.5)).unwrap();
    make_old(&k);
    ahead(&draft(&p), 3600);
    m.open_edit();
    assert_eq!(vals(&m), [7.5], "premise: the draft won");
    if foreign_draft {
        save(
            &draft(&p),
            &with_value(load(&draft(&p)).unwrap().unwrap(), 8.5),
        )
        .unwrap();
        ahead(&draft(&p), 3600);
    }
    let lock = hold(&draft(&p), 1);
    gloss(&mut m, 12.5);
    m.close_edit();
    let refused = m.edit.is_some();
    gloss(&mut m, 13.5);
    let kept_now = load(&k).ok().flatten().map(|_| doc_values(&k));
    drop(lock);
    assert!(refused, "premise: the future-dated draft still wins");
    assert_eq!(
        kept_now,
        Some(vec![7.5, 12.5, 13.5]),
        "the latest edit is kept on disk"
    );
}

/// Review round 5 (Critical "Give kept, draft and plain successively later future timestamps, hold kept readable but
/// deny its rename, carry plain into P, then quit and reopen", ruling E6-R2): a carry that cannot move the loser aside
/// replaces no draft and deletes no plain autosave; it stays failed with its reason.
#[cfg(windows)]
#[test]
fn a_carry_finishes_only_when_its_cleanup_did() {
    let (d, mut m) = export_session();
    let (p, k) = three_versions(&d, &mut m);
    ahead(&k, 3600);
    ahead(&draft(&p), 7200);
    ahead(&autosave(&d), 10800);
    let (theirs, plain) = (
        std::fs::read(draft(&p)).unwrap(),
        std::fs::read(autosave(&d)).unwrap(),
    );
    let lock = hold(&k, 1);
    m.set_project_root(Some(&p));
    let why = m.edit.as_ref().unwrap().export_block().unwrap_or_default();
    let (draft_now, plain_now) = (
        std::fs::read(draft(&p)).ok(),
        std::fs::read(autosave(&d)).ok(),
    );
    drop(lock);
    assert_eq!(draft_now, Some(theirs), "P's draft is not replaced");
    assert_eq!(plain_now, Some(plain), "the plain autosave stays");
    assert!(
        why.starts_with("Could not save the edits of"),
        "the failed carry says why: {why}"
    );
    drop(m); // quit
    let mut m = restart(&d, &p);
    m.open_edit();
    assert_eq!(vals(&m), [12.5], "the carried ops load");
}

#[cfg(windows)]
#[test]
fn returning_to_the_project_of_a_failed_carry_preserves_both_drafts() {
    let (d, mut m) = export_session();
    let (p, k) = three_versions(&d, &mut m);
    let plain = autosave(&d);
    ahead(&k, 3600);
    ahead(&draft(&p), 7200);
    ahead(&plain, 10800);
    let original = std::fs::read(draft(&p)).unwrap();
    let carried = std::fs::read(&plain).unwrap();
    let _lock = hold(&k, 1);
    m.set_project_root(Some(&p));
    assert!(review_row(&m).blocked.is_some());
    m.set_project_root(Some(&project_folder(&d, "q")));
    assert_eq!(m.edit.as_ref().unwrap().draft_path, draft(&p));
    m.set_project_root(Some(&p));
    assert!(
        std::fs::read(draft(&p)).unwrap() == original
            && std::fs::read(&plain).ok() == Some(carried),
        "returning to P replaced its draft or removed the plain autosave"
    );
    assert_eq!(doc_values(&k), [9.5]);
    assert!(review_row(&m).blocked.is_some());
}

#[cfg(windows)]
#[test]
fn reopening_after_a_failed_draft_save_loads_the_winning_kept_edit() {
    let (d, mut m) = export_session();
    let (p, k) = open_on_a_newer_kept_autosave(&d, &mut m, |doc| with_value(doc, 9.5));
    ahead(&k, 3600);
    assert_eq!(vals(&m), [9.5]);
    let lock = hold(&draft(&p), 1);
    export_fails(&mut m, "could not be saved");
    drop(m);
    drop(lock);
    let mut m = restart(&d, &p);
    m.open_edit();
    assert_eq!(vals(&m), [9.5], "the newer kept edit still wins at reopen");
    assert_eq!(doc_values(&k), [9.5]);
}

#[cfg(windows)]
#[test]
fn a_locked_future_winner_stops_the_draft_before_a_new_edit_is_written() {
    let (d, mut m) = export_session();
    let (p, k) = open_on_a_newer_kept_autosave(&d, &mut m, |doc| with_value(doc, 9.5));
    ahead(&k, 3600);
    let lock = hold(&k, 1);
    gloss(&mut m, 13.5);
    assert_eq!(
        doc_values(&draft(&p)),
        [7.5],
        "the draft stays untouched until the winner can be removed"
    );
    assert_eq!(doc_values(&k), [9.5]);
    drop(lock);
    assert_eq!(export(&mut m), Ok(1));
    assert_eq!(doc_values(&draft(&p)), [9.5, 13.5]);
    assert!(!k.exists());
}

#[test]
fn a_project_moved_during_the_draft_write_can_still_find_its_kept_edit() {
    let (d, mut m) = export_session();
    let (p, k) = open_on_a_newer_kept_autosave(&d, &mut m, |doc| with_value(doc, 9.5));
    ahead(&k, 3600);
    let moved = d.path().join("moved");
    let (from, to) = (p.clone(), moved.clone());
    set_write_hook(Some(Box::new(move |stage| {
        if stage == "root checked" {
            std::fs::rename(&from, &to).unwrap();
        }
    })));
    let result = export(&mut m);
    set_write_hook(None);
    assert!(result.unwrap_err().contains("could not be saved"));
    drop(m);
    let mut m = restart(&d, &moved);
    m.open_edit();
    assert_eq!(
        vals(&m),
        [9.5],
        "the draft still identifies its winning kept autosave"
    );
}

#[cfg(windows)]
#[test]
fn a_zero_op_overwrite_retries_after_the_locked_nif_is_released() {
    let (d, mut m, _a) = saved_in_p();
    assert_eq!(export(&mut m), Ok(1));
    let installed = exported(&d);
    let before = std::fs::read(&installed).unwrap();
    let record = nif_doc(&export_root(&d));
    let b = project_folder(&d, "b");
    m.set_project_root(Some(&b));
    m.set_nif_export_root(Some(d.path().join("B exports")));
    gloss(&mut m, 14.5);
    assert_eq!(export(&mut m), Ok(1));
    assert!(m.undo_edit());
    m.set_nif_export_root(Some(export_root(&d)));
    answer(&mut m, GuardChoice::Overwrite);
    let lock = hold(&installed, 1);
    export_fails(&mut m, "Cannot write");
    let pre = load(&record).unwrap().unwrap();
    assert_eq!(pre.export.owner, Some(id_of(&b)));
    assert_eq!(pre.export.sha256, Some(sha256_hex(&before)));
    assert_eq!(std::fs::read(&installed).unwrap(), before);
    drop(lock);
    m.set_nif_answer("meshes/x.nif", GuardChoice::Overwrite, sha(&before));
    assert_eq!(export(&mut m), Ok(1), "retry the authorized zero-op write");
    assert_eq!(
        std::fs::read(&installed).unwrap(),
        m.edit.as_ref().unwrap().out.as_slice()
    );
    let done = load(&record).unwrap().unwrap();
    assert_eq!(done.id, Some(id_of(&b)));
    assert!(done.ops.is_empty());
    assert_eq!(
        done.export.sha256,
        Some(sha256_hex(&std::fs::read(installed).unwrap()))
    );
}

/// Review round 5 (Critical "Make plain unwritable, request Q, restore plain, then choose writable P ...", Important
/// "Wait for P because plain is unwritable, make P's draft unreadable ..." and the out-of-scope "Fail a carry into P,
/// request Q while P is still blocked ...", rulings E6-R3, E6-R4): one waiting target, the latest request; a failure
/// clears only when the next open at home loads the ops.
#[test]
fn one_waiting_target_and_a_failure_clears_only_when_durable() {
    table(&[
        (
            "make plain unwritable, request Q, restore plain, then choose writable P before making the retry edit",
            || {
                let (d, mut m) = export_session();
                let (p, q) = (project_folder(&d, "p"), project_folder(&d, "q"));
                m.open_edit();
                gloss(&mut m, 12.5);
                read_only(&autosave(&d), true);
                gloss(&mut m, 13.5);
                m.set_project_root(Some(&q));
                assert_eq!(
                    m.edit.as_ref().unwrap().draft_path,
                    autosave(&d),
                    "premise: Q waits"
                );
                read_only(&autosave(&d), false);
                m.set_project_root(Some(&p));
                m.set_project_root(Some(&p)); // the next frame
                gloss(&mut m, 14.5);
                assert_eq!(m.edit.as_ref().unwrap().draft_path, draft(&p), "the edit is P's");
                assert_eq!(doc_values(&draft(&p)), [12.5, 13.5, 14.5]);
                assert!(!q.join("_sdc").exists(), "nothing is written to Q");
            },
        ),
        (
            "wait for P because plain is unwritable, make P's draft unreadable, restore plain, edit to retry, then \
             Export",
            || {
                let (d, mut m) = export_session();
                let p = project_folder(&d, "p");
                m.open_edit();
                gloss(&mut m, 12.5);
                read_only(&autosave(&d), true);
                gloss(&mut m, 13.5);
                m.set_project_root(Some(&p));
                std::fs::create_dir_all(draft(&p).parent().unwrap()).unwrap();
                std::fs::write(draft(&p), b"not a document [[[").unwrap();
                read_only(&autosave(&d), false);
                gloss(&mut m, 14.5);
                let r = export(&mut m);
                assert!(!exported(&d).exists(), "nothing is exported: {r:?}");
                assert!(!nif_doc(&export_root(&d)).exists(), "no record: {r:?}");
                let why = r.unwrap_err();
                assert!(why.contains("while changing the project"), "the reason: {why}");
                assert_eq!(std::fs::read(draft(&p)).unwrap(), b"not a document [[[");
            },
        ),
        (
            "fail a carry into P, request Q while P is still blocked, replace plain with a future-dated different \
             edit, repair P, return to P, then Export",
            || {
                let (d, mut m) = export_session();
                let (p, q) = (project_folder(&d, "p"), project_folder(&d, "q"));
                m.open_edit();
                gloss(&mut m, 12.5);
                std::fs::write(p.join("_sdc"), b"a file").unwrap();
                m.set_project_root(Some(&p));
                m.set_project_root(Some(&q));
                assert_eq!(
                    m.edit.as_ref().unwrap().draft_path,
                    draft(&p),
                    "premise: P's session waits"
                );
                let other = with_value(load(&autosave(&d)).unwrap().unwrap(), 99.5);
                save(&autosave(&d), &other).unwrap();
                ahead(&autosave(&d), 3600);
                std::fs::remove_file(p.join("_sdc")).unwrap();
                m.set_project_root(Some(&p));
                let r = export(&mut m);
                assert!(!exported(&d).exists(), "nothing is exported: {r:?}");
                let why = r.unwrap_err();
                assert!(why.contains("Could not save the edits of"), "the reason stays: {why}");
            },
        ),
    ]);
}

/// Review round 5 (Important "Export X, leave X applied, move its record away, then run Review's texture Export ...",
/// "Export X, undo to zero, move the record away and click Export" and "Export X, leave the session current, replace
/// its record with malformed contents ...", rulings E6-S2, E6-S3): a present unrecorded NIF needs its answer, and an
/// Export that does nothing says why.
#[test]
fn export_asks_for_an_unrecorded_nif_and_says_why_it_does_nothing() {
    table(&[
        (
            "export X, leave X applied, move its record away, then run Review's texture Export with the installed X \
             still present",
            || {
                let (d, mut m, _p) = saved_in_p();
                assert_eq!(export(&mut m), Ok(1));
                let t = std::fs::metadata(exported(&d)).unwrap().modified().unwrap();
                let rec = nif_doc(&export_root(&d));
                std::fs::rename(&rec, d.path().join("moved.toml")).unwrap();
                let r = export(&mut m);
                assert!(!rec.exists(), "no record is written: {r:?}");
                assert_eq!(
                    std::fs::metadata(exported(&d)).unwrap().modified().unwrap(),
                    t,
                    "the NIF is not replaced: {r:?}"
                );
                let why = r.unwrap_err();
                assert!(why.contains(NIF_UNANSWERED), "the question: {why}");
            },
        ),
        (
            "export X, undo to zero, move the record away and click Export",
            || {
                let (d, mut m, _p) = saved_in_p();
                assert_eq!(export(&mut m), Ok(1));
                let nif_x = std::fs::read(exported(&d)).unwrap();
                assert!(m.undo_edit());
                let rec = nif_doc(&export_root(&d));
                std::fs::rename(&rec, d.path().join("moved.toml")).unwrap();
                let r = export(&mut m);
                assert_eq!(std::fs::read(exported(&d)).unwrap(), nif_x, "{r:?}");
                assert!(!rec.exists(), "{r:?}");
                let why = r.unwrap_err();
                assert!(why.contains("holds no record of it"), "the reason: {why}");
            },
        ),
        (
            "export X, leave the session current, replace its record with malformed contents, then run Review's \
             texture Export",
            || {
                let (d, mut m, _p) = saved_in_p();
                assert_eq!(export(&mut m), Ok(1));
                let nif_x = std::fs::read(exported(&d)).unwrap();
                let rec = nif_doc(&export_root(&d));
                std::fs::write(&rec, b"not a document [[[").unwrap();
                let r = export(&mut m);
                assert_eq!(std::fs::read(exported(&d)).unwrap(), nif_x, "{r:?}");
                assert_eq!(std::fs::read(&rec).unwrap(), b"not a document [[[", "{r:?}");
                let why = r.unwrap_err();
                assert!(why.contains("cannot be read"), "the read error: {why}");
            },
        ),
    ]);
}

/// Note GD21 (replaces `a_recovered_empty_autosave_stays_on_a_folder_change`): an autosave with no ops (every edit
/// undone) and no history, newer than the project's draft, is recovered when the project opens: the session holds
/// that empty state, unsaved, and the project's draft is kept aside at its first save.
#[test]
fn a_recovered_empty_autosave_stays_when_a_project_is_opened() {
    let (d, mut m) = export_session();
    let p = project_folder(&d, "p");
    let theirs = project_with_a_draft(&mut m, &p);
    m.set_project_root(None);
    m.open_edit();
    gloss(&mut m, 12.5);
    assert!(m.undo_edit());
    m.close_edit();
    assert!(load(&autosave(&d)).unwrap().unwrap().ops.is_empty());
    make_old(&draft(&p));
    m.open_edit(); // no project: the empty autosave, no history
    let e = m.edit.as_ref().unwrap();
    assert!(e.doc.ops.is_empty() && !e.history.can_undo() && !e.history.can_redo());
    m.set_project_root(Some(&p));
    let e = m.edit.as_ref().unwrap();
    assert!(e.doc.ops.is_empty(), "the recovered empty state stays");
    assert!(e.touched(), "unsaved");
    assert_eq!(e.draft_path, draft(&p));
    m.edit_with(|e| e.flush());
    assert!(load(&draft(&p)).unwrap().unwrap().ops.is_empty());
    let aside = kept_beside(&draft(&p));
    assert_eq!(aside.len(), 1, "{aside:?}");
    assert_eq!(std::fs::read(&aside[0]).unwrap(), theirs);
}

/// Project `a`'s session of x.nif with one edit (12.5) that can be saved nowhere: after the NIF opened, `a\_sdc` and
/// the autosave folder `d\nif-edits` became plain files. Returns `a`.
fn unsaveable_edit(d: &tempfile::TempDir, m: &mut MeshesModel) -> std::path::PathBuf {
    let a = project_folder(d, "a");
    m.set_project_root(Some(&a));
    m.open_edit();
    std::fs::write(a.join("_sdc"), b"a file").unwrap();
    std::fs::write(d.path().join("nif-edits"), b"a file").unwrap();
    gloss(m, 12.5);
    assert!(m.error.is_some(), "premise: the edit's save failed");
    a
}

fn y_nif(m: &MeshesModel) -> i64 {
    m.nifs
        .iter()
        .find(|n| n.path.ends_with("y.nif"))
        .unwrap()
        .copy_id
}

/// Review E6 (Critical 1, ruling E6-C): selecting another NIF never drops an edit that could be saved nowhere: the
/// selection stays, and the footer and the model's error say why.
#[test]
fn selecting_another_nif_keeps_an_edit_that_cannot_be_saved() {
    let (d, mut m) = export_session();
    unsaveable_edit(&d, &mut m);
    let x = m.selected_nif;
    m.select_nif(y_nif(&m));
    assert!(m.edit.is_some(), "the session is not dropped");
    assert_eq!(
        op_values(&m).iter().map(|v| v.1).collect::<Vec<_>>(),
        [12.5]
    );
    assert_eq!(m.selected_nif, x, "the selection stays");
    let why = m.edit.as_ref().unwrap().export_block().unwrap();
    assert!(
        why.starts_with("Could not save the edits of meshes\\x.nif: "),
        "{why}"
    );
    assert_eq!(m.error.as_deref(), Some(why.as_str()));
}

/// Ruling E6-C: Close edit and the Edit toggle (`open_edit`) keep such an edit too, history and all.
#[test]
fn closing_or_reopening_keeps_an_edit_that_cannot_be_saved() {
    let (d, mut m) = export_session();
    unsaveable_edit(&d, &mut m);
    m.close_edit();
    assert!(m.edit.is_some(), "Close edit does not drop it");
    m.open_edit();
    assert_eq!(
        op_values(&m).iter().map(|v| v.1).collect::<Vec<_>>(),
        [12.5],
        "no new session over it"
    );
    assert!(
        m.edit.as_ref().unwrap().history.can_undo(),
        "the same session"
    );
}

/// Ruling E6-C1: the way out without quitting. Undo back to what is on disk (no draft here: no ops) and the NIF closes
/// with nothing written, redo history and all; the on-disk state is the session's, so nothing is lost.
#[test]
fn undoing_back_to_the_saved_state_lets_an_unsaveable_edit_go() {
    let (d, mut m) = export_session();
    let a = unsaveable_edit(&d, &mut m);
    let y = y_nif(&m);
    m.select_nif(y);
    assert_eq!(op_values(&m).len(), 1, "premise: refused while unsaved");
    assert!(m.undo_edit());
    assert!(
        m.edit.as_ref().unwrap().history.can_redo(),
        "premise: redo left"
    );
    m.select_nif(y);
    assert_eq!(m.selected_nif, Some(y), "the other NIF is selected");
    assert!(m.edit.is_none(), "the session ended");
    assert_eq!(std::fs::read(a.join("_sdc")).unwrap(), b"a file");
    assert_eq!(
        std::fs::read(d.path().join("nif-edits")).unwrap(),
        b"a file"
    );
    assert_eq!(
        std::fs::read_dir(&a).unwrap().count(),
        1,
        "no file was made in the project"
    );
}

/// Review E6 (Important 8, ruling E6-C): after a failed switch a drag tick is the next edit: it tries the two saves at
/// once, not at the drag's end.
#[test]
fn a_drag_after_a_failed_switch_tries_the_saves_again() {
    let (d, mut m) = export_session();
    let (a, b) = (project_folder(&d, "a"), project_folder(&d, "b"));
    project_with_a_draft(&mut m, &b);
    m.set_project_root(Some(&a));
    m.open_edit();
    gloss(&mut m, 12.5);
    let id = id_of(&a);
    let blocker = d.path().join("nif-edits");
    std::fs::write(&blocker, b"a file").unwrap();
    std::fs::remove_dir_all(&a).unwrap();
    m.set_project_root(Some(&b));
    let tries = m.switch_save_attempts_for_test();
    std::fs::remove_file(&blocker).unwrap();
    tick(&mut m, 40.5);
    // the tick's retry (one attempt); ruling E6-K: the session stays, as no open finds a removed project's kept
    // autosave
    assert_eq!(m.switch_save_attempts_for_test(), tries + 1);
    assert_eq!(doc_values(&kept(&d, &id)), [12.5, 40.5]);
    assert_eq!(vals(&m), [12.5, 40.5], "the session stays");
    // the switch waits (E6-K): each further tick saves once, and Messages name the kept autosave once
    for v in [41.5, 42.5, 43.5] {
        tick(&mut m, v);
    }
    assert_eq!(m.switch_save_attempts_for_test(), tries + 4);
    m.set_nif_export_root(m.nif_export_root.clone()); // the next frame moves the session's lines to Messages
    let named = m
        .messages
        .iter()
        .filter(|l| l.contains("They are kept in"))
        .count();
    assert_eq!(named, 1, "{:?}", m.messages);
    assert_eq!(
        doc_values(&kept(&d, &id)),
        [12.5, 43.5],
        "the drag's last tick is kept"
    );
}

/// Review E6 (Important 8, ruling E6-C): a return to the session's own project that cannot save there keeps the
/// switch's reason; the next edit that saves into the project goes on in the same session.
#[test]
fn a_failed_return_to_the_own_project_keeps_the_reason_until_a_save_works() {
    let (d, mut m) = export_session();
    let (a, b) = (project_folder(&d, "a"), project_folder(&d, "b"));
    project_with_a_draft(&mut m, &b);
    m.set_project_root(Some(&a));
    m.open_edit();
    gloss(&mut m, 12.5);
    gloss(&mut m, 13.5);
    std::fs::remove_dir_all(a.join("_sdc")).unwrap();
    std::fs::write(a.join("_sdc"), b"a file").unwrap();
    std::fs::write(d.path().join("nif-edits"), b"a file").unwrap();
    m.set_project_root(Some(&b));
    let why = m.edit.as_ref().unwrap().export_block().unwrap();
    assert!(why.contains("while changing the project"), "{why}");
    for _ in 0..3 {
        m.set_project_root(Some(&a)); // A still cannot be written
    }
    assert_eq!(
        m.edit.as_ref().unwrap().export_block(),
        Some(why),
        "the reason stays until a save works"
    );
    std::fs::remove_file(a.join("_sdc")).unwrap();
    gloss(&mut m, 14.5);
    assert_eq!(doc_values(&draft(&a)), [12.5, 13.5, 14.5]);
    let e = m.edit.as_ref().unwrap();
    assert!(!e
        .export_block()
        .is_some_and(|w| w.contains("while changing the project")));
    assert!(
        m.undo_edit() && m.undo_edit() && m.undo_edit(),
        "the same session goes on in A"
    );
}

/// Review E6 (Important 8, ruling E6-C): a session that cannot be saved says so on Export this NIF and in Review even
/// when its edits are the ones last exported (no "No edits").
#[test]
fn an_unsaveable_edit_equal_to_its_export_still_says_why() {
    let (d, mut m) = export_session();
    let (a, b) = (project_folder(&d, "a"), project_folder(&d, "b"));
    m.set_project_root(Some(&a));
    m.open_edit();
    gloss(&mut m, 12.5);
    assert_eq!(export(&mut m), Ok(1));
    assert!(!m.edit.as_ref().unwrap().pending(), "premise: exported");
    std::fs::write(d.path().join("nif-edits"), b"a file").unwrap();
    std::fs::remove_dir_all(&a).unwrap();
    m.set_project_root(Some(&b));
    let why = m.edit.as_ref().unwrap().export_block().unwrap();
    assert!(why.contains("while changing the project"), "{why}");
    assert_eq!(
        m.quick_export_block().as_deref(),
        Some(why.as_str()),
        "Export this NIF says why"
    );
    assert_eq!(review_row(&m).blocked.as_deref(), Some(why.as_str()));
    export_fails(&mut m, "while changing the project");
}

/// Review round 2 (Important 8, ruling E6-J): with no export folder chosen, Export this NIF gives the failed switch's
/// reason, not the missing folder.
#[test]
fn with_no_export_folder_quick_export_gives_the_switch_failure() {
    let (d, mut m) = export_session();
    let (a, b) = (project_folder(&d, "a"), project_folder(&d, "b"));
    m.set_nif_export_root(None);
    m.set_project_root(Some(&a));
    m.open_edit();
    gloss(&mut m, 12.5);
    std::fs::write(d.path().join("nif-edits"), b"a file").unwrap();
    std::fs::remove_dir_all(&a).unwrap();
    m.set_project_root(Some(&b));
    assert!(m.nif_export_root.is_none(), "premise: no folder");
    let why = m.edit.as_ref().unwrap().export_block().unwrap();
    assert!(why.contains("while changing the project"), "{why}");
    assert_eq!(m.quick_export_block().as_deref(), Some(why.as_str()));
}

/// Sets or clears the read-only attribute of `p` (Windows refuses to replace a read-only file).
fn read_only(p: &Path, on: bool) {
    let mut perm = std::fs::metadata(p).unwrap().permissions();
    perm.set_readonly(on);
    std::fs::set_permissions(p, perm).unwrap();
}

/// Review E6 (Critical 2, ruling E6-B): an edit whose save fails is unsaved, even when an older draft with the id is
/// still on disk, so the project switch keeps it (here in the kept autosave) instead of dropping it.
#[test]
fn a_failed_save_of_an_edit_is_kept_by_the_next_switch() {
    let (d, mut m) = export_session();
    let (a, b) = (project_folder(&d, "a"), project_folder(&d, "b"));
    m.set_project_root(Some(&a));
    m.open_edit();
    gloss(&mut m, 12.5);
    let id = id_of(&a);
    read_only(&draft(&a), true); // the draft stays, but cannot be replaced
    gloss(&mut m, 13.5);
    assert!(m.error.is_some(), "premise: the save failed");
    m.set_project_root(Some(&b));
    read_only(&draft(&a), false);
    assert_eq!(
        load(&kept(&d, &id)).unwrap().map(|k| k.ops.len()),
        Some(2),
        "the edit the failed save left is kept"
    );
    assert_eq!(doc_values(&draft(&a)), [12.5], "the old draft is as it was");
}

/// Ruling E6-B with E6-E: an edit whose save failed stays unsaved, so Export tries the draft again first and, when it
/// still fails, writes no NIF.
#[test]
fn an_export_after_a_failed_save_tries_the_draft_first() {
    let (d, mut m, p) = session_in_project("p", None);
    m.open_edit();
    gloss(&mut m, 12.5);
    read_only(&draft(&p), true);
    gloss(&mut m, 13.5);
    let r = export(&mut m);
    read_only(&draft(&p), false);
    let why = r.unwrap_err();
    assert!(why.contains("could not be saved"), "{why}");
    assert!(!exported(&d).exists(), "no NIF written");
}

/// Ruling E6-B (review Q6): a draft written before drafts had an id is not rewritten by a switch just to add one.
#[test]
fn a_switch_never_writes_a_draft_only_for_its_id() {
    let (d, mut m) = export_session();
    let (a, b) = (project_folder(&d, "a"), project_folder(&d, "b"));
    m.set_project_root(Some(&a));
    m.open_edit();
    gloss(&mut m, 12.5);
    m.close_edit();
    let mut legacy = load(&draft(&a)).unwrap().unwrap();
    legacy.id = None;
    save(&draft(&a), &legacy).unwrap();
    let before = std::fs::read(draft(&a)).unwrap();
    m.open_edit();
    assert!(
        m.edit.as_ref().unwrap().touched(),
        "premise: the draft's edit is open"
    );
    m.set_project_root(Some(&b));
    assert_eq!(
        std::fs::read(draft(&a)).unwrap(),
        before,
        "nothing new to save: the draft is not written"
    );
}

#[test]
fn export_still_writes_the_record_in_the_export_folder() {
    let (d, mut m) = export_session();
    // an ordinary folder: `sdc NIF Edits` is never a source anyway (GD28), so it would not show GQ3 at work
    let e = d.path().join(r"mods\MyOut");
    m.set_nif_export_root(Some(e.clone()));
    m.open_edit();
    gloss(&mut m, 12.5);
    let c = [copy_at(&e, "out", 9)];
    assert!(
        pick_source(&c, Path::new(r"C:\elsewhere"), "meshes/x.nif").is_some(),
        "premise: before the export the folder is a source like any other"
    );
    assert_eq!(export(&mut m), Ok(1));
    let rec = load(&nif_doc(&e)).unwrap().expect("the record is written");
    let want = sha256_hex(&std::fs::read(e.join(r"meshes\x.nif")).unwrap());
    assert_eq!(rec.export.sha256.as_deref(), Some(want.as_str()));
    // Note GQ3: the export folder holds our record, so it is never taken as a source
    assert!(pick_source(&c, Path::new(r"C:\elsewhere"), "meshes/x.nif").is_none());
}

/// An export of x.nif (one op, 7.5) that an earlier Tessalith left in the export folder (`mods\sdc NIF Edits`): its
/// record carries no draft id and, when `stale`, names another source. The session is closed and no draft is left, so
/// the next session has none of those ops. Returns the record's bytes.
fn old_record_in_export_folder(d: &tempfile::TempDir, m: &mut MeshesModel, stale: bool) -> Vec<u8> {
    m.open_edit();
    gloss(m, 7.5);
    assert_eq!(export(m), Ok(1));
    m.close_edit();
    let p = nif_doc(&export_root(d));
    let mut r = load(&p).unwrap().unwrap();
    r.id = None;
    r.export.owner = None;
    if stale {
        r.source.sha256 = "0".repeat(64);
    }
    save(&p, &r).unwrap();
    std::fs::remove_file(autosave(d)).unwrap();
    std::fs::read(&p).unwrap()
}

/// Review E6 (Critical 3, ruling E6-A): opening a NIF writes nothing in the export folder, even when its record was
/// made from another source (the forced re-export is worked out in memory).
#[test]
fn opening_never_rewrites_an_older_record_in_the_export_folder() {
    let (d, mut m) = export_session();
    let before = old_record_in_export_folder(&d, &mut m, true);
    m.open_edit();
    assert_eq!(m.error, None);
    let e = m.edit.as_ref().unwrap();
    assert!(
        e.doc.ops.is_empty(),
        "premise: the session has none of the record's ops"
    );
    assert!(e.pending(), "the record names another source: export again");
    assert_eq!(
        std::fs::read(nif_doc(&export_root(&d))).unwrap(),
        before,
        "the record holds its edits as before"
    );
}

/// Ruling E6-A: a source reload changes the session, never the record in the export folder.
#[test]
fn a_source_reload_never_rewrites_the_record() {
    let (d, mut m) = export_session();
    let before = old_record_in_export_folder(&d, &mut m, false);
    m.open_edit();
    let src = changed_source(&m);
    std::fs::write(d.path().join("mods/A/meshes/x.nif"), &src).unwrap();
    m.edit_with(|e| e.reload_source(src.clone()).unwrap());
    assert!(m.edit.as_ref().unwrap().pending());
    assert_eq!(
        std::fs::read(nif_doc(&export_root(&d))).unwrap(),
        before,
        "the reload wrote nothing in the export folder"
    );
}

/// Ruling E6-A: a record holding edits this draft never had (an earlier Tessalith's document) is not replaced by an
/// export; the row says why and nothing is written.
#[test]
fn export_never_replaces_an_older_record_with_other_edits() {
    let (d, mut m) = export_session();
    let before = old_record_in_export_folder(&d, &mut m, false);
    let nif_before = std::fs::read(exported(&d)).unwrap();
    m.open_edit();
    gloss(&mut m, 12.5);
    let why = review_row(&m).blocked.expect("the row is blocked");
    assert!(why.contains("or an earlier Tessalith"), "{why}");
    export_fails(&mut m, "or an earlier Tessalith");
    assert_eq!(std::fs::read(nif_doc(&export_root(&d))).unwrap(), before);
    assert_eq!(std::fs::read(exported(&d)).unwrap(), nif_before);
}

/// Ruling E6-A with E6-D: a carry-over gives the session another id (the project draft's, or a new one), yet the
/// record it exported itself stays its own, in that session and after a reopen (its ops start the session's).
#[test]
fn a_carried_edit_still_replaces_the_record_it_exported() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    assert_eq!(export(&mut m), Ok(1));
    let p = project_folder(&d, "p");
    m.set_project_root(Some(&p));
    assert_eq!(op_values(&m).len(), 1, "premise: carried into the project");
    gloss(&mut m, 13.5);
    assert_eq!(review_row(&m).blocked, None);
    assert_eq!(export(&mut m), Ok(1));
    gloss(&mut m, 14.5);
    m.close_edit();
    // a record written under the old id (before the carry-over) and reopened in the project
    let rec = nif_doc(&export_root(&d));
    let mut r = load(&rec).unwrap().unwrap();
    r.id = Some("0123456789abcdef".into());
    save(&rec, &r).unwrap();
    m.open_edit();
    gloss(&mut m, 15.5);
    assert_eq!(review_row(&m).blocked, None, "its ops start the session's");
    assert_eq!(export(&mut m), Ok(1));
    assert_eq!(doc_values(&rec), [12.5, 13.5, 14.5, 15.5]);
}

/// The guard of ruling E6-A's block: the draft's own record is replaced by every later export, in the same session and
/// in the next one.
#[test]
fn a_draft_replaces_its_own_record_at_every_export() {
    let (d, mut m, _p) = session_in_project("proj", None);
    m.open_edit();
    gloss(&mut m, 7.5);
    assert_eq!(export(&mut m), Ok(1));
    gloss(&mut m, 12.5);
    assert_eq!(export(&mut m), Ok(1));
    m.close_edit();
    m.open_edit();
    gloss(&mut m, 13.5);
    assert_eq!(review_row(&m).blocked, None);
    assert_eq!(export(&mut m), Ok(1));
    assert_eq!(doc_values(&nif_doc(&export_root(&d))), [7.5, 12.5, 13.5]);
}

/// Ruling E6-A: choosing a folder whose record was exported from another source makes the edit pending there, worked
/// out in memory (the record is not rewritten).
#[test]
fn choosing_a_folder_whose_record_names_another_source_makes_the_edit_pending() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    assert_eq!(export(&mut m), Ok(1));
    assert!(!m.edit.as_ref().unwrap().pending());
    let p = nif_doc(&export_root(&d));
    let mut r = load(&p).unwrap().unwrap();
    r.source.sha256 = "0".repeat(64);
    save(&p, &r).unwrap();
    let before = std::fs::read(&p).unwrap();
    m.set_nif_export_root(Some(d.path().join(r"mods\MyOut")));
    m.set_nif_export_root(Some(export_root(&d)));
    assert!(
        m.edit.as_ref().unwrap().pending(),
        "the folder's export was made from another source"
    );
    assert_eq!(std::fs::read(&p).unwrap(), before);
}

/// Note GD5: no export folder, no default mod. The edit opens and works, autosaves to the app's folder, and its
/// export is blocked with FB1's reason; choosing a folder later lets it export there, and the draft stays put.
#[test]
fn no_export_folder_blocks_the_nif_row() {
    let (d, mut m) = export_session();
    m.set_nif_export_root(None);
    assert_eq!(m.edit_mod(), None);
    m.open_edit();
    assert_eq!(m.error, None, "the edit opens");
    // Note GH5: nothing edited yet reads NO_EDITS; the first edit brings the folder's reason.
    let e = m.edit.as_ref().unwrap();
    assert_eq!(e.blocked.as_deref(), Some(NO_EDITS));
    assert_eq!(NO_EXPORT_FOLDER, "No export folder chosen");
    gloss(&mut m, 12.5);
    assert_eq!(
        m.edit.as_ref().unwrap().blocked.as_deref(),
        Some(NO_EXPORT_FOLDER)
    );
    assert_eq!(m.edit.as_ref().unwrap().draft_path, autosave(&d));
    assert_eq!(load(&autosave(&d)).unwrap().unwrap().ops.len(), 1);
    assert!(
        !nif_doc(&d.path().join("mods").join(EDIT_MOD)).exists(),
        "no default mod"
    );
    export_fails(&mut m, NO_EXPORT_FOLDER);
    // a crash: the next session finds the autosave
    m.close_edit();
    m.open_edit();
    assert_eq!(op_values(&m).len(), 1, "recovered from the autosave");
    // a folder is chosen: the draft stays in the autosave (it follows the project, not the folder), and the export
    // leaves its record in the folder (Review E6; was GD5 (a)-(c): the edit moved to the folder)
    let out = d.path().join(r"mods\MyOut");
    m.set_nif_export_root(Some(out.clone()));
    assert_eq!(m.edit.as_ref().unwrap().draft_path, autosave(&d));
    assert!(!nif_doc(&out).exists(), "the switch writes nothing");
    gloss(&mut m, 13.5);
    assert_eq!(load(&autosave(&d)).unwrap().unwrap().ops.len(), 2);
    assert!(!nif_doc(&out).exists(), "an edit writes no record");
    assert_eq!(m.edit.as_ref().unwrap().blocked, None);
    assert_eq!(export(&mut m), Ok(1));
    assert!(nif_doc(&out).is_file(), "the export writes its record");
}

/// Note GD11: an autosave this session never loaded is never deleted. Edit without a project, close, open a
/// project, reopen: the autosave (newer than the project's none) is offered, and its first save there migrates it.
#[test]
fn an_autosave_is_recovered_when_a_project_is_opened_before_reopening() {
    let (d, mut m) = export_session();
    m.set_nif_export_root(None);
    m.open_edit();
    gloss(&mut m, 12.5);
    m.close_edit();
    assert!(autosave(&d).is_file());
    let out = project_folder(&d, "proj");
    m.set_project_root(Some(&out));
    m.messages.clear();
    m.open_edit();
    assert_eq!(m.error, None);
    assert_eq!(op_values(&m).len(), 1, "the autosaved edit is offered");
    assert_eq!(
        m.messages,
        vec![format!(
            "Recovered the edits of meshes/x.nif autosaved in {}.",
            autosave(&d).display()
        )]
    );
    gloss(&mut m, 13.5);
    assert_eq!(
        load(&draft(&out)).unwrap().unwrap().ops.len(),
        2,
        "the autosave is not lost"
    );
    assert!(
        !autosave(&d).exists(),
        "migrated into the project, then removed"
    );
}

/// Note GD11: a project draft newer than the autosave wins; the autosave stays, named in Messages, and a save in
/// the project leaves it alone.
#[test]
fn an_older_autosave_stays_when_the_projects_draft_is_newer() {
    let (d, mut m) = export_session();
    m.set_nif_export_root(None);
    m.open_edit();
    gloss(&mut m, 12.5);
    m.close_edit();
    let aside = d.path().join("autosave.aside");
    std::fs::rename(autosave(&d), &aside).unwrap();
    let out = project_folder(&d, "proj");
    m.set_project_root(Some(&out));
    m.open_edit();
    gloss(&mut m, 7.5);
    m.close_edit();
    std::fs::rename(&aside, autosave(&d)).unwrap();
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    std::fs::File::options()
        .write(true)
        .open(autosave(&d))
        .unwrap()
        .set_modified(old)
        .unwrap();
    m.messages.clear();
    m.open_edit();
    assert_eq!(op_values(&m).len(), 1);
    assert_eq!(op_values(&m)[0].1, 7.5, "the newer project draft wins");
    assert_eq!(
        m.messages,
        vec![format!(
            "Older edits of meshes/x.nif stay autosaved in {}.",
            autosave(&d).display()
        )]
    );
    gloss(&mut m, 13.5);
    assert!(autosave(&d).is_file(), "an autosave never loaded stays");
}

/// An autosave of one op (12.5) older than the project's draft (one op, 7.5), and the session open on the draft:
/// the autosave is not loaded (GD11). Returns the autosave's bytes and the project.
fn older_autosave_open_on_newer_draft(
    d: &tempfile::TempDir,
    m: &mut MeshesModel,
) -> (Vec<u8>, std::path::PathBuf) {
    m.set_nif_export_root(None);
    m.open_edit();
    gloss(m, 12.5);
    m.close_edit();
    let mine = std::fs::read(autosave(d)).unwrap();
    let aside = d.path().join("autosave.aside");
    std::fs::rename(autosave(d), &aside).unwrap();
    let proj = project_folder(d, "proj");
    m.set_project_root(Some(&proj));
    m.open_edit();
    gloss(m, 7.5);
    m.close_edit();
    std::fs::rename(&aside, autosave(d)).unwrap();
    make_old(&autosave(d));
    m.open_edit();
    assert_eq!(op_values(m)[0].1, 7.5, "the project's draft opens");
    m.messages.clear();
    (mine, proj)
}

/// Note GD24: an autosave and a project draft with the same modified time: the draft opens, the autosave stays
/// (named in Messages) and a save in the project leaves it.
#[test]
fn equal_times_keep_the_project_draft_and_the_autosave() {
    let (d, mut m) = export_session();
    let (mine, out) = older_autosave_open_on_newer_draft(&d, &mut m);
    m.close_edit();
    let t = std::fs::metadata(draft(&out)).unwrap().modified().unwrap();
    std::fs::File::options()
        .write(true)
        .open(autosave(&d))
        .unwrap()
        .set_modified(t)
        .unwrap();
    m.messages.clear();
    m.open_edit();
    assert_eq!(op_values(&m)[0].1, 7.5, "the project's draft opens");
    assert_eq!(
        m.messages,
        vec![format!(
            "Older edits of meshes/x.nif stay autosaved in {}.",
            autosave(&d).display()
        )]
    );
    gloss(&mut m, 13.5);
    assert_eq!(
        std::fs::read(autosave(&d)).unwrap(),
        mine,
        "the autosave stays as it was"
    );
}

/// The files beside the autosave that a save moved aside (`<stem>.<seconds>.toml`).
fn kept_aside(d: &tempfile::TempDir) -> Vec<std::path::PathBuf> {
    kept_beside(&autosave(d))
}

/// The files a save moved aside from `a` (`<stem>.<seconds>.toml` in its folder), oldest name first.
fn kept_beside(a: &Path) -> Vec<std::path::PathBuf> {
    let stem = format!("{}.", a.file_stem().unwrap().to_string_lossy());
    let mut v: Vec<_> = std::fs::read_dir(a.parent().unwrap())
        .unwrap()
        .map(|f| f.unwrap().path())
        .filter(|p| p != a && p.file_name().unwrap().to_string_lossy().starts_with(&stem))
        .collect();
    v.sort();
    v
}

/// Note GD26: a move-aside name already taken (this second's and the next two) is skipped: the move takes a free
/// later name, and the files already there are unchanged. (Review E6: the file moved aside is the project's draft that
/// a carried-over edit replaces; the autosave is never written without being the session's.)
#[test]
fn a_move_aside_skips_names_already_taken() {
    let (d, mut m) = export_session();
    let proj = project_folder(&d, "proj");
    let theirs = project_with_a_draft(&mut m, &proj);
    m.set_project_root(None);
    m.open_edit();
    gloss(&mut m, 12.5);
    let a = draft(&proj);
    let stem = a.file_stem().unwrap().to_string_lossy().into_owned();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let taken: Vec<_> = (now..now + 3)
        .map(|s| a.with_file_name(format!("{stem}.{s}.toml")))
        .collect();
    for t in &taken {
        std::fs::write(t, b"another instance's").unwrap();
    }
    m.set_project_root(Some(&proj));
    for t in &taken {
        assert_eq!(
            std::fs::read(t).unwrap(),
            b"another instance's",
            "{t:?} unchanged"
        );
    }
    let new: Vec<_> = kept_beside(&a)
        .into_iter()
        .filter(|p| !taken.contains(p))
        .collect();
    assert_eq!(new.len(), 1, "{new:?}");
    assert_eq!(std::fs::read(&new[0]).unwrap(), theirs);
}

/// Note GD25: a newer autosave recovered over a project draft with other edits never overwrites it: the first save
/// moves the project's draft aside, and Messages names the kept file.
#[test]
fn a_recovered_autosave_keeps_the_project_draft_it_replaced() {
    let (d, mut m) = export_session();
    let out = project_folder(&d, "proj");
    m.set_nif_export_root(None);
    m.open_edit();
    gloss(&mut m, 12.5);
    m.close_edit();
    let aside = d.path().join("autosave.aside");
    std::fs::rename(autosave(&d), &aside).unwrap();
    m.set_project_root(Some(&out));
    m.open_edit();
    gloss(&mut m, 7.5);
    m.close_edit();
    let theirs = std::fs::read(draft(&out)).unwrap();
    std::fs::rename(&aside, autosave(&d)).unwrap();
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(3600);
    std::fs::File::options()
        .write(true)
        .open(autosave(&d))
        .unwrap()
        .set_modified(later)
        .unwrap();
    m.messages.clear();
    m.open_edit();
    assert_eq!(op_values(&m)[0].1, 12.5, "the newer autosave opens");
    gloss(&mut m, 13.5);
    let kept = kept_beside(&draft(&out));
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert_eq!(
        std::fs::read(&kept[0]).unwrap(),
        theirs,
        "the project's edits survive"
    );
    assert_eq!(load(&draft(&out)).unwrap().unwrap().ops.len(), 2);
    m.set_nif_export_root(m.nif_export_root.clone()); // the next frame moves the session's lines to Messages
    assert_eq!(
        m.messages.last(),
        Some(&format!(
            "The edit document {} held edits of meshes/x.nif that this session did not open: kept as {}.",
            draft(&out).display(),
            kept[0].display()
        ))
    );
}

/// Note GD25: a touched session carried into a project with its own draft of the NIF never overwrites it: the first
/// save there (the carry-over's, ruling E6-H1) moves that draft aside.
#[test]
fn opening_a_project_keeps_the_draft_it_replaced() {
    let (d, mut m) = export_session();
    let out = project_folder(&d, "proj");
    let theirs = project_with_a_draft(&mut m, &out);
    m.set_project_root(None);
    m.open_edit();
    gloss(&mut m, 12.5);
    m.set_project_root(Some(&out));
    let kept = kept_beside(&draft(&out));
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert_eq!(
        std::fs::read(&kept[0]).unwrap(),
        theirs,
        "the project's edits survive"
    );
    assert_eq!(doc_values(&draft(&out)), [12.5]);
    gloss(&mut m, 13.5);
    gloss(&mut m, 14.5);
    assert_eq!(kept_beside(&draft(&out)).len(), 1, "moved aside once");
    assert_eq!(doc_values(&draft(&out)), [12.5, 13.5, 14.5]);
}

/// Note GD19 (replaces the failed folder change's fallback, which is gone with GD12): a project switch whose save
/// fails keeps the edits in a kept autosave, and the plain autosave an older session left is not replaced by it.
#[test]
fn a_failed_project_switch_leaves_the_plain_autosave_alone() {
    let (d, mut m) = export_session();
    let (mine, proj) = older_autosave_open_on_newer_draft(&d, &mut m);
    gloss(&mut m, 13.5);
    let id = id_of(&proj);
    std::fs::remove_dir_all(&proj).unwrap();
    let other = project_folder(&d, "other");
    m.set_project_root(Some(&other));
    assert_eq!(doc_values(&kept(&d, &id)), [7.5, 13.5]);
    assert_eq!(
        std::fs::read(autosave(&d)).unwrap(),
        mine,
        "the plain autosave is as it was"
    );
    assert_eq!(
        kept_aside(&d),
        [kept(&d, &id)],
        "only the kept autosave is beside it, nothing was moved aside"
    );
}

/// Note GD19 (replaces "clearing the folder never overwrites an unloaded autosave"): closing the project opens the
/// autosave an older session left (it is the session's document then, so nothing is overwritten) and leaves the
/// project's draft as it was.
#[test]
fn closing_the_project_opens_the_autosave_and_leaves_the_draft() {
    let (d, mut m) = export_session();
    let (mine, proj) = older_autosave_open_on_newer_draft(&d, &mut m);
    let theirs = std::fs::read(draft(&proj)).unwrap();
    m.set_project_root(None);
    assert_eq!(
        op_values(&m).iter().map(|v| v.1).collect::<Vec<_>>(),
        [12.5],
        "the autosave's edit opens"
    );
    assert_eq!(std::fs::read(draft(&proj)).unwrap(), theirs);
    gloss(&mut m, 13.5);
    assert_eq!(load(&autosave(&d)).unwrap().unwrap().ops.len(), 2);
    assert!(kept_aside(&d).is_empty(), "{mine:?} was loaded, not moved");
}

#[test]
fn an_export_folder_outside_the_instance_is_not_checked_and_still_exports() {
    let o = tempfile::tempdir().unwrap();
    let out = o.path().join("out");
    let (_d, mut m) = session_into("+PGP\n+A\n", |_| out.clone());
    m.open_edit();
    let p = m.edit.as_ref().unwrap().placement.clone();
    assert_eq!(p, Some(Placement::NotInMo2));
    assert_eq!(
        Placement::NotInMo2.text("out"),
        "The export folder is not an MO2 mod here: load order not checked."
    );
    gloss(&mut m, 12.5);
    assert_eq!(export(&mut m), Ok(1));
    assert!(out.join(r"meshes\x.nif").is_file());
    assert!(nif_doc(&out).is_file());
}

/// Note GD5 (a) (Review E6: the draft stays where it is): a folder change keeps the open session as it is and writes
/// nothing; it takes the new folder's record (what was exported there), and no Messages line says a thing moved.
#[test]
fn changing_the_export_folder_keeps_the_edit_in_memory() {
    let (d, mut m) = export_session();
    let out = d.path().join(r"mods\MyOut");
    let old = d.path().join("mods").join(EDIT_MOD);
    // MyOut already holds the record of an export of x.nif (one op, 7.5).
    m.set_nif_export_root(Some(out.clone()));
    m.open_edit();
    gloss(&mut m, 7.5);
    assert_eq!(export(&mut m), Ok(1));
    m.close_edit();
    let theirs = std::fs::read(nif_doc(&out)).unwrap();
    let their_hash = load(&nif_doc(&out)).unwrap().unwrap().export.sha256;
    assert!(their_hash.is_some(), "premise: the record has an export");
    std::fs::remove_file(autosave(&d)).unwrap(); // the session below starts clean
                                                 // Back to the first folder: an edit there, a shape selected and a drag in progress (not saved yet).
    m.set_nif_export_root(Some(old.clone()));
    m.open_edit();
    assert_eq!(m.edit.as_ref().unwrap().doc.ops.len(), 0);
    gloss(&mut m, 12.5);
    let shape = lit_shape(m.edit.as_ref().unwrap()).0;
    m.select_edit(Selection::Shape(shape));
    tick(&mut m, 40.5);
    m.messages.clear();
    let draft_before = std::fs::read(autosave(&d)).unwrap();
    m.set_nif_export_root(Some(out.clone()));
    let e = m.edit.as_ref().unwrap();
    assert_eq!(e.draft_path, autosave(&d), "the draft stays");
    assert_eq!(e.record_path, Some(nif_doc(&out)));
    assert_eq!(e.export_path, out.join(r"meshes\x.nif"));
    assert_eq!(op_values(&m).len(), 2, "the live edit stays");
    assert_eq!(op_values(&m)[0].1, 12.5);
    assert_eq!(
        e.selection,
        Selection::Shape(shape),
        "the selection is kept"
    );
    assert_eq!(
        e.doc.export.sha256, None,
        "the draft keeps its own export state"
    );
    assert_eq!(
        review_row(&m).foreign,
        Some(sha(&std::fs::read(&e.export_path).unwrap()))
    );
    assert_eq!(
        std::fs::read(nif_doc(&out)).unwrap(),
        theirs,
        "the switch writes no record"
    );
    assert_eq!(
        std::fs::read(autosave(&d)).unwrap(),
        draft_before,
        "the switch does not save the drag"
    );
    assert!(!nif_doc(&old).exists());
    assert!(m.messages.is_empty(), "{:?}", m.messages);
    // dirty: closing the edit saves the drag in the draft
    m.close_edit();
    assert_eq!(load(&autosave(&d)).unwrap().unwrap().ops.len(), 2);
}

/// Note GD12 (an export folder whose record does not load): the root is not taken, the row is blocked with the
/// reason, the draft goes on saving where it is, and a later root that works clears it.
#[test]
fn a_malformed_document_in_the_new_folder_blocks_the_row() {
    let (d, mut m) = export_session();
    let old = d.path().join("mods").join(EDIT_MOD);
    assert_eq!(m.nif_export_root.as_deref(), Some(old.as_path()));
    m.open_edit();
    gloss(&mut m, 12.5);
    let bad = d.path().join(r"mods\Bad");
    std::fs::create_dir_all(nif_doc(&bad).parent().unwrap()).unwrap();
    std::fs::write(nif_doc(&bad), b"not = [toml").unwrap();
    m.set_nif_export_root(Some(bad.clone()));
    assert_eq!(
        m.nif_export_root.as_deref(),
        Some(old.as_path()),
        "the new root is not taken"
    );
    let why = review_row(&m).blocked.expect("the row is blocked");
    let head = format!("could not open the edit document in {}: ", bad.display());
    assert!(why.starts_with(&head), "{why}");
    gloss(&mut m, 13.5);
    assert!(!nif_doc(&old).exists(), "no record in the old folder");
    export_fails(&mut m, &head);
    assert!(!old.join(r"meshes\x.nif").exists());
    assert_eq!(
        load(&autosave(&d)).unwrap().unwrap().ops.len(),
        2,
        "the draft keeps saving"
    );
    // asked again (the next frame): still blocked; a root that works clears it
    m.set_nif_export_root(Some(bad.clone()));
    assert!(review_row(&m).blocked.is_some());
    let good = d.path().join(r"mods\Good");
    m.set_nif_export_root(Some(good.clone()));
    assert_eq!(m.nif_export_root.as_deref(), Some(good.as_path()));
    assert_eq!(review_row(&m).blocked, None);
    assert_eq!(export(&mut m), Ok(1));
    assert!(
        nif_doc(&good).is_file(),
        "the record is in the folder that worked"
    );
    assert_eq!(load(&autosave(&d)).unwrap().unwrap().ops.len(), 2);
}

/// Note HS12: a touched edit never takes as its export folder the folder holding its source copy (an export there
/// would replace the source); the row is blocked as GD12's, in any spelling of that folder.
#[test]
fn an_export_folder_holding_the_source_copy_is_not_taken() {
    let (d, mut m) = export_session();
    m.set_nif_export_root(None);
    m.open_edit();
    let src = d.path().join(r"mods\A");
    assert_eq!(
        m.edit.as_ref().unwrap().source_copy.origin,
        "A",
        "precondition: the source is A's copy"
    );
    let before = std::fs::read(src.join(r"meshes\x.nif")).unwrap();
    gloss(&mut m, 12.5);
    // Review Focus 5: the same folder in another case and with a trailing separator
    let other = std::path::PathBuf::from(format!("{}\\", d.path().join(r"MODS\a").display()));
    for root in [src.clone(), other] {
        m.set_nif_export_root(Some(root.clone()));
        assert_eq!(m.nif_export_root, None, "{} is not taken", root.display());
        let e = m.edit.as_ref().unwrap();
        assert_ne!(e.export_path, src.join(r"meshes\x.nif"));
        let why = review_row(&m).blocked.expect("the row is blocked");
        assert!(why.contains(&root.display().to_string()), "{why}");
        assert!(why.contains("holds the mesh"), "{why}");
        export_fails(&mut m, "holds the mesh");
    }
    assert_eq!(std::fs::read(src.join(r"meshes\x.nif")).unwrap(), before);
    assert!(!nif_doc(&src).exists());
    gloss(&mut m, 13.5);
    assert_eq!(
        load(&autosave(&d)).unwrap().unwrap().ops.len(),
        2,
        "the work is kept in the autosave meanwhile"
    );
    let good = d.path().join(r"mods\Good");
    m.set_nif_export_root(Some(good.clone()));
    assert_eq!(m.nif_export_root.as_deref(), Some(good.as_path()));
    assert_eq!(review_row(&m).blocked, None);
}

/// Note GD20: a failed folder change reads the new folder's document once, not each frame; a change to the file (its
/// length or time) reads it again.
#[test]
fn a_failed_folder_change_reads_the_document_once_until_it_changes() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    let bad = d.path().join(r"mods\Bad");
    std::fs::create_dir_all(nif_doc(&bad).parent().unwrap()).unwrap();
    std::fs::write(nif_doc(&bad), b"not = [toml").unwrap();
    let n = load_count();
    for _ in 0..5 {
        m.set_nif_export_root(Some(bad.clone()));
    }
    assert_eq!(load_count() - n, 1, "one read over five frames");
    assert!(review_row(&m).blocked.is_some());
    std::fs::write(nif_doc(&bad), b"still = [not toml").unwrap();
    for _ in 0..5 {
        m.set_nif_export_root(Some(bad.clone()));
    }
    assert_eq!(load_count() - n, 2, "read again after the file changed");
    assert!(review_row(&m).blocked.is_some());
}

/// Note GD23: after an export, idle frames hash nothing: the Review rows and the panel read the cached pending state.
#[test]
fn idle_frames_after_an_export_hash_nothing() {
    let (_d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    assert_eq!(export(&mut m), Ok(1));
    let root = m.nif_export_root.clone();
    let mut h = view(m);
    h.run_steps(2);
    let _ = h.state().nif_review_rows();
    let n = ops_hash_count();
    for _ in 0..10 {
        let _ = h.state().nif_review_rows();
        h.state_mut().set_nif_export_root(root.clone());
        h.run_steps(1);
    }
    assert_eq!(ops_hash_count() - n, 0, "no hash over ten idle frames");
    gloss(h.state_mut(), 13.5);
    assert_eq!(review_row(h.state()).blocked, None);
    assert!(ops_hash_count() > n, "an edit is hashed again");
}

/// Note GD13 (Review E6: a folder change is a light switch): a session undone back to the source after an export keeps
/// its document and its redo when the export folder changes, and nothing says a thing moved.
#[test]
fn an_undone_session_keeps_its_live_document_on_a_folder_change() {
    let (d, mut m) = export_session();
    let out = d.path().join(r"mods\MyOut");
    m.open_edit();
    gloss(&mut m, 12.5);
    assert_eq!(export(&mut m), Ok(1));
    assert!(m.undo_edit());
    assert!(m.edit.as_ref().unwrap().doc.ops.is_empty());
    m.messages.clear();
    m.set_nif_export_root(Some(out.clone()));
    let e = m.edit.as_ref().unwrap();
    assert_eq!(e.record_path, Some(nif_doc(&out)));
    assert!(e.doc.ops.is_empty(), "the live (undone) document stays");
    assert!(e.history.can_redo(), "its redo stays");
    assert_eq!(e.doc.export.sha256, None, "nothing was exported to MyOut");
    assert!(m.messages.is_empty(), "{:?}", m.messages);
    assert!(m.redo_edit());
    assert_eq!(op_values(&m)[0].1, 12.5);
}

/// Note GD13, GD21 (Review E6: the carry-over into a project): a session undone back to the source is not idle: opening
/// a project keeps its live document and its redo, and the project's draft is kept aside at the next save (said in
/// Messages). Ruling E6-H1: the empty carried edit differs from the project's draft, so the carry writes it and the
/// draft is kept aside then.
#[test]
fn an_undone_session_stays_when_a_project_is_opened() {
    let (d, mut m) = export_session();
    let proj = project_folder(&d, "proj");
    let theirs = project_with_a_draft(&mut m, &proj);
    m.set_project_root(None);
    m.open_edit();
    gloss(&mut m, 12.5);
    assert!(m.undo_edit());
    m.messages.clear();
    m.set_project_root(Some(&proj));
    let e = m.edit.as_ref().unwrap();
    assert_eq!(e.draft_path, draft(&proj));
    assert!(e.doc.ops.is_empty(), "the live (undone) document stays");
    assert!(e.history.can_redo(), "its redo stays");
    assert_eq!(
        m.messages.first(),
        Some(&format!(
            "Project opened: the open edit of meshes/x.nif goes to {}; the edits saved there are kept aside at the next save.",
            draft(&proj).display()
        ))
    );
    let aside = kept_beside(&draft(&proj));
    assert_eq!(aside.len(), 1, "the carry keeps the draft aside");
    assert_eq!(std::fs::read(&aside[0]).unwrap(), theirs);
    assert!(load(&draft(&proj)).unwrap().unwrap().ops.is_empty());
    assert!(m.redo_edit());
    assert_eq!(kept_beside(&draft(&proj)).len(), 1, "moved aside once");
    assert_eq!(doc_values(&draft(&proj)), [12.5]);
}

/// Note GD27: a save that fails leaves the session unsaved: a recovered empty autosave whose save into the project
/// fails is still touched, and a project switch then keeps it in the kept autosave.
#[test]
fn a_failed_save_keeps_a_recovered_empty_autosave_touched() {
    let (d, mut m) = export_session();
    m.set_nif_export_root(None);
    m.open_edit();
    gloss(&mut m, 12.5);
    assert!(m.undo_edit());
    m.close_edit();
    let good = project_folder(&d, "good");
    m.set_project_root(Some(&good));
    m.open_edit();
    assert!(
        m.edit.as_ref().unwrap().doc.ops.is_empty(),
        "the empty autosave is recovered"
    );
    std::fs::create_dir_all(draft(&good)).unwrap(); // the save cannot write a directory
    m.edit_with(|e| e.flush());
    let e = m.edit.as_ref().unwrap();
    assert!(e.error.is_some() || m.error.is_some(), "the save failed");
    assert!(e.touched(), "still unsaved");
    let id = e.doc.id.clone().unwrap();
    m.set_project_root(Some(&project_folder(&d, "other")));
    assert!(
        load(&kept(&d, &id)).unwrap().unwrap().ops.is_empty(),
        "the recovered state is kept"
    );
}

/// Note GD12: back to the folder the session never left, the block clears and the draft goes on saving.
#[test]
fn going_back_from_a_failed_folder_change_clears_the_block() {
    let (d, mut m) = export_session();
    let old = d.path().join("mods").join(EDIT_MOD);
    m.open_edit();
    gloss(&mut m, 12.5);
    let bad = d.path().join(r"mods\Bad");
    std::fs::create_dir_all(nif_doc(&bad).parent().unwrap()).unwrap();
    std::fs::write(nif_doc(&bad), b"not = [toml").unwrap();
    m.set_nif_export_root(Some(bad));
    gloss(&mut m, 13.5);
    assert!(review_row(&m).blocked.is_some());
    m.set_nif_export_root(Some(old.clone()));
    assert_eq!(review_row(&m).blocked, None);
    assert_eq!(load(&autosave(&d)).unwrap().unwrap().ops.len(), 2);
}

/// Note GD5, GD13 (Review E6: the new folder's saved edit is no longer loaded into an idle session): a session with
/// no edits keeps its empty document when the folder changes, and takes only the new folder's export record.
#[test]
fn an_idle_session_takes_only_the_new_folders_record() {
    let (d, mut m) = export_session();
    let out = d.path().join(r"mods\MyOut");
    m.set_nif_export_root(Some(out.clone()));
    m.open_edit();
    gloss(&mut m, 7.5);
    assert_eq!(export(&mut m), Ok(1));
    m.close_edit();
    // An ownerless record remains readable even by a new session with no draft of its own.
    let path = nif_doc(&out);
    let mut legacy = load(&path).unwrap().unwrap();
    legacy.export.owner = None;
    save(&path, &legacy).unwrap();
    std::fs::remove_file(autosave(&d)).unwrap();
    m.set_nif_export_root(Some(d.path().join("mods").join(EDIT_MOD)));
    m.open_edit();
    m.messages.clear();
    m.set_nif_export_root(Some(out.clone()));
    assert_eq!(
        op_values(&m).len(),
        0,
        "MyOut's record is not loaded as edits"
    );
    let e = m.edit.as_ref().unwrap();
    assert!(e.doc.export.sha256.is_some(), "it is its export state");
    assert!(m.messages.is_empty());
}

#[test]
fn the_destination_line_shows_the_real_path() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    let full = writes_line(&d);
    let mut h = view(m);
    h.run_steps(3);
    let name = h.get_by_label_contains(&full).value();
    assert_eq!(name, Some(format!("{full}\n{WRITES_NOTE}")));
    assert!(h.query_by_label(WRITES_NOTE).is_none());
}

/// The sentence the Writes line's accessible name and hover carry after the line.
const WRITES_NOTE: &str =
    "The source file is never changed. Change the folder with Export to ▾ in the header.";

/// The text of the `Shape::Text` galleys whose centre lies inside `cell`, as painted last frame (a label's node
/// rect is a little taller and narrower than its glyph bounds).
fn painted_text_in(h: &Harness<'static, MeshesModel>, cell: egui::Rect) -> String {
    h.output()
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Text(t) if cell.contains(t.visual_bounding_rect().center()) => {
                Some(t.galley.rows.iter().map(|r| r.text()).collect::<String>())
            }
            _ => None,
        })
        .collect()
}

/// Review E23, CZ2: the Writes line is cut in the middle to the footer's width in pixels (the export path first,
/// then the data path), keeps its file name, and carries the whole line and the source-file sentence in its name.
#[test]
fn the_writes_line_fits_and_keeps_the_full_line() {
    let d = files(false);
    put(
        d.path(),
        "mods/A/meshes/a very long folder/another long folder/x.nif",
        None,
    );
    let mut m = MeshesModel::new(Some(d.path().join("scan.db")), MeshesSettings::default());
    m.procs = sdc_gui::model::meshes::no_procs;
    scan_and_select(&mut m, d.path());
    let deep = d
        .path()
        .join("mods")
        .join("a deep folder for the export")
        .join("with several more folders")
        .join(EDIT_MOD);
    std::fs::create_dir_all(&deep).unwrap();
    m.set_nif_export_root(Some(deep));
    let long = m
        .nifs
        .iter()
        .find(|n| {
            n.path.ends_with("another long folder\\x.nif")
                || n.path.ends_with("another long folder/x.nif")
        })
        .expect("the long-path nif")
        .copy_id;
    m.select_nif(long);
    m.open_edit();
    gloss(&mut m, 12.5);
    let e = m.edit.as_ref().unwrap();
    let data = e.data_path.replace('/', "\\");
    let export = e.export_path.display().to_string().replace('/', "\\");
    let full = format!("Writes {data} to {export}");
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1536.0, 864.0))
        .build_ui_state(
            |ui, m: &mut MeshesModel| sdc_gui::ui::meshes::show(ui, m),
            m,
        );
    h.run_steps(4);
    let node = h.get_by_label_contains(&full);
    let shown = painted_text_in(&h, node.rect());
    let panel = support::panel(&h.ctx, "meshes-edit");
    assert!(shown.starts_with("Writes "), "{shown}");
    assert!(shown.ends_with("x.nif"), "{shown}");
    assert!(shown.chars().count() < full.chars().count(), "{shown}");
    assert!(
        node.rect().width() <= panel.width(),
        "{:?} {panel:?}",
        node.rect()
    );
    assert_eq!(node.value(), Some(format!("{full}\n{WRITES_NOTE}")));
    assert!(h.query_by_label(WRITES_NOTE).is_none());
}

#[test]
fn the_app_passes_the_textures_export_folder_to_the_meshes_view() {
    app_data();
    let t = tempfile::tempdir().unwrap();
    let model = support::opened(&t.path().join("mod"));
    let first = model.export_root();
    let (_d, mut h) = in_the_meshes_view(model, 1);
    assert_eq!(
        h.state().meshes.nif_export_root.as_deref(),
        Some(first.as_path())
    );
    // Export to ▾ changes (Note GD5, Review E6): the open edit takes the new folder for its record only; its draft is
    // with the opened project (the app's first frame carried it there) and nothing is written to either folder.
    let want = t.path().join("other");
    h.state_mut().model.set_export_folder(want.clone());
    assert_eq!(h.state().model.export_root(), want);
    h.run_steps(2);
    let m = &h.state().meshes;
    assert_eq!(m.nif_export_root.as_deref(), Some(want.as_path()));
    let e = m.edit.as_ref().unwrap();
    assert_eq!(e.record_path, Some(nif_doc(&want)));
    assert!(
        e.draft_path.starts_with(t.path().join("mod")),
        "the draft is under the project: {}",
        e.draft_path.display()
    );
    assert_eq!(e.doc.ops.len(), 1, "the live edit stays");
    // ruling E6-H1: the first frame's carry-over wrote the project's draft
    assert_eq!(load(&e.draft_path).unwrap().unwrap().ops.len(), 1);
    assert!(!nif_doc(&first).exists() && !nif_doc(&want).exists());
    assert!(
        h.state()
            .model
            .messages
            .entries()
            .all(|m| !m.text.contains("Export folder changed")),
        "no line says the edit moved"
    );
}

// ---- v2 B3: the clock, the per-frame cbuffer path ----

use render3d::{EditKind, ShapeId};
use sdc_gui::model::viewport3d::V3Reply;

/// `bytes` with a free BSLightingShaderPropertyFloatController var 20 (U offset), Loop + Active (0x48), start 0,
/// stop 1, LINEAR keys (0, 0) → (1, 1), put first in the first lit shape's controller chain.
fn with_uv_ctrl(bytes: &[u8]) -> Vec<u8> {
    with_uv_ctrl_timed(bytes, 0x48, [1.0, 0.0, 0.0, 1.0])
}

/// [`with_uv_ctrl`] with the controller's `flags` and (frequency, phase, start, stop).
fn with_uv_ctrl_timed(bytes: &[u8], flags: u16, timing: [f32; 4]) -> Vec<u8> {
    let m = nif::model::read_model(bytes).unwrap();
    let lsp = m
        .shapes
        .iter()
        .find_map(|s| s.shader.filter(|b| m.lighting.contains_key(b)))
        .unwrap();
    let net = m.lighting[&lsp].prop.net.clone();
    let mut doc = nif::write::NifDoc::open(bytes).unwrap();
    let at = net.controller_at() - doc.header().offsets()[lsp as usize];
    let key = |time: f32, value: f32| Key {
        time,
        value,
        forward: None,
        backward: None,
        tbc: None,
    };
    let keys = KeyData::Float(KeyGroup {
        key_type: Some(1),
        keys: vec![key(0.0, 0.0), key(1.0, 1.0)],
    });
    let data = doc
        .append(b"NiFloatData", nif::keys::encode_data(&keys))
        .unwrap();
    let interp = doc
        .append(
            b"NiFloatInterpolator",
            builder::W::default().f(0.0).i32(data as i32).0,
        )
        .unwrap();
    let ctrl = doc
        .append(
            b"BSLightingShaderPropertyFloatController",
            builder::W::default()
                .i32(net.controller)
                .u16(flags)
                .fs(&timing)
                .i32(lsp as i32)
                .i32(interp as i32)
                .u32(20)
                .0,
        )
        .unwrap();
    doc.patch(lsp, at, &(ctrl as i32).to_le_bytes()).unwrap();
    let out = doc.bytes();
    let back = nif::model::read_model(&out).unwrap();
    assert!(back.lighting[&lsp].prop.ctrls.contains(&ctrl));
    assert!(back.ctrls[&ctrl].known, "{:?}", back.ctrls[&ctrl]);
    out
}

/// A scanned session whose x.nif has `with_uv_ctrl`, the edit open, the first load sent and answered.
fn playing_session() -> (tempfile::TempDir, MeshesModel, Receiver<V3Cmd>) {
    session_on(&with_uv_ctrl(&fixture("TestNifFile_Skinned_SE.nif")))
}

/// [`playing_session`] on the given x.nif bytes.
fn session_on(nif: &[u8]) -> (tempfile::TempDir, MeshesModel, Receiver<V3Cmd>) {
    let d = files(false);
    put(d.path(), "mods/A/meshes/x.nif", Some(nif));
    let mut m = MeshesModel::new(Some(d.path().join("scan.db")), MeshesSettings::default());
    m.procs = sdc_gui::model::meshes::no_procs;
    scan_and_select(&mut m, d.path());
    let cmds = attach(&mut m);
    m.open_edit();
    assert_eq!(m.error, None);
    m.tick_anim(0.0); // makes the clock
    m.v3.request_frame(64, 64);
    let Some(V3Cmd::LoadParsed { id, nif, .. }) = cmds
        .try_iter()
        .find(|c| matches!(c, V3Cmd::LoadParsed { .. }))
    else {
        panic!("the first LoadParsed")
    };
    m.v3.handle(V3Reply::Loaded {
        id,
        report: render3d::LoadReport {
            shapes: nif.shapes.len(),
            ..Default::default()
        },
        bounds: Some(([0.0; 3], 1.0)),
    });
    cmds.try_iter().for_each(drop);
    (d, m, cmds)
}

/// Review Focus 5: 300 playing ticks send only SetMaterial and Render (no load, no shader, feature or deferred
/// command), and every material differs from the static one only in cbuffer fields.
#[test]
fn playback_is_cbuffer_only() {
    let (_d, mut m, cmds) = playing_session();
    let c = m.v3.clock.expect("an animated session has a clock");
    assert_eq!((c.t, c.playing, c.range), (0.0, false, (0.0, 1.0)));
    let animated = m.edit.as_ref().unwrap().frame_materials(0.0);
    assert!(!animated.is_empty());
    let base: std::collections::BTreeMap<ShapeId, nif::material::MaterialState> =
        m.edit.as_ref().unwrap().materials().into_iter().collect();
    m.v3.clock.as_mut().unwrap().play();
    let (mut sets, mut renders) = (vec![], vec![]);
    for _ in 0..300 {
        m.tick_anim(1.0 / 60.0);
        m.v3.request_frame(64, 64);
        for c in cmds.try_iter() {
            match c {
                V3Cmd::SetMaterial { id, material, .. } => sets.push((id, material)),
                V3Cmd::Render { time, .. } => renders.push(time),
                _ => panic!("only SetMaterial and Render during playback"),
            }
        }
    }
    assert_eq!(renders.len(), 300, "one Render per tick");
    assert_eq!(
        sets.len(),
        300 * animated.len(),
        "one SetMaterial per animated shape per tick"
    );
    assert!(
        renders.iter().all(|t| (0.0..=1.0).contains(t)),
        "looping stays in range"
    );
    let mut cbuffer = 0;
    for (id, mat) in &sets {
        let k = render3d::classify(&base[id], mat, &[], false);
        assert!(
            matches!(k, EditKind::Cbuffer | EditKind::Same),
            "{id:?}: {k:?}"
        );
        cbuffer += (k == EditKind::Cbuffer) as usize;
        let mut back = mat.clone();
        back.params.uv_offset[0] = base[id].params.uv_offset[0];
        assert_eq!(&back, &base[id], "only U offset is animated");
    }
    assert!(cbuffer > 0);
    // The 30th tick is t = 0.5 (dt 1/60 < MAX_DT): U offset follows the LINEAR curve.
    assert!((sets[29 * animated.len()].1.params.uv_offset[0] - renders[29]).abs() < 1e-5);
    assert!((renders[29] - 0.5).abs() < 1e-4);
}

#[test]
fn scrub_sets_time_and_pauses() {
    let (_d, mut m, cmds) = playing_session();
    m.v3.clock.as_mut().unwrap().play();
    m.tick_anim(0.02);
    m.v3.request_frame(64, 64);
    cmds.try_iter().for_each(drop);
    m.v3.clock.as_mut().unwrap().scrub(0.25);
    m.tick_anim(0.02);
    let c = m.v3.clock.unwrap();
    assert_eq!(
        (c.t, c.playing),
        (0.25, false),
        "a paused clock does not move"
    );
    m.v3.request_frame(64, 64);
    let got: Vec<V3Cmd> = cmds.try_iter().collect();
    assert!(got.iter().any(
        |c| matches!(c, V3Cmd::SetMaterial { material, .. } if material.params.uv_offset[0] == 0.25)
    ));
    assert!(got
        .iter()
        .any(|c| matches!(c, V3Cmd::Render { time, .. } if *time == 0.25)));
    m.tick_anim(0.02);
    m.v3.request_frame(64, 64);
    assert!(
        cmds.try_recv().is_err(),
        "paused and shown: nothing more is sent"
    );
}

/// F6 and F21: In game pauses the clock and sends nothing; closing the session drops the clock.
#[test]
fn the_clock_pauses_in_game_and_goes_with_the_session() {
    let (_d, mut m, cmds) = playing_session();
    m.v3.clock.as_mut().unwrap().play();
    m.set_view_source(false);
    assert!(m.v3.nif_view, "In game (simulated) is the edit preview too");
    assert!(m.v3.clock.is_some());
    m.tick_anim(0.02);
    assert!(!m.v3.clock.unwrap().playing);
    assert!(!cmds
        .try_iter()
        .any(|c| matches!(c, V3Cmd::SetMaterial { .. })));
    m.close_edit();
    assert_eq!(m.v3.clock, None);
}

/// The controls under the 3D view drive the clock.
#[test]
fn play_and_pause_buttons_drive_the_clock() {
    let (_d, m, _cmds) = playing_session();
    let mut h = view(m);
    h.run_steps(2);
    h.get_by_label("Play").click();
    h.run_steps(3);
    let c = h.state().v3.clock.unwrap();
    assert!(c.playing && c.t > 0.0, "{c:?}");
    h.get_by_label("Pause").click();
    h.run_steps(2);
    assert!(!h.state().v3.clock.unwrap().playing);
    h.get_by_label("Loop");
    h.get_by_role_and_label(AkRole::Slider, "Time");
}

/// Rule 3.1: at 1100, 1280 and 1600 px the clock rows (Play, Loop, Speed, the Time slider and its text) stay
/// inside the centre column.
#[test]
fn the_clock_row_fits_the_centre_column() {
    for w in [1100.0, 1280.0, 1600.0] {
        let (_d, m, _cmds) = playing_session();
        let mut h = Harness::builder()
            .with_context_setup(|ctx| {
                sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
            })
            .with_size(egui::vec2(w, 800.0))
            .build_ui_state(
                |ui, m: &mut MeshesModel| sdc_gui::ui::meshes::show(ui, m),
                m,
            );
        h.run_steps(3);
        let rect = |id: &str| support::panel(&h.ctx, id);
        rect("meshes-3d-anim");
        // A panel grows to its content, so the bound is the centre column: between the tree and the edit panel.
        let (lo, hi) = (rect("meshes-tree").max.x, rect("meshes-edit").min.x);
        let mut nodes = h.query_all_by_label("Time").collect::<Vec<_>>();
        nodes.push(h.get_by_label("Play"));
        nodes.push(h.get_by_label("Loop"));
        // v2 B8: the edit-mode toggle above them, too.
        nodes.push(h.get_by_label("In game"));
        nodes.push(h.get_by_label("Source"));
        let speed: Vec<_> = h.query_all_by_label("Speed").collect();
        assert!(!speed.is_empty(), "the Speed label and its value");
        nodes.extend(speed);
        assert!(nodes.len() >= 4);
        for n in nodes {
            let r = n.rect();
            assert!(
                r.max.x <= hi + 0.5 && r.min.x >= lo - 0.5,
                "{w} px: {r:?} outside {lo}..{hi}"
            );
        }
    }
}

// ---- v2 B3 fix round 1 (B-P33): the clock range is in scene time ----

/// The U offset sent for clock time `t` (scrubbed, then one tick).
fn u_at(m: &mut MeshesModel, cmds: &Receiver<V3Cmd>, t: f32) -> f32 {
    cmds.try_iter().for_each(drop);
    m.v3.clock.as_mut().unwrap().scrub(t);
    m.tick_anim(0.0);
    cmds.try_iter()
        .find_map(|c| match c {
            V3Cmd::SetMaterial { material, .. } => Some(material.params.uv_offset[0]),
            _ => None,
        })
        .expect("a SetMaterial")
}

/// A 0-1 controller at frequency 0.5 takes 2 s of scene time: the clock plays the whole curve.
#[test]
fn half_speed_plays_the_whole_curve() {
    let nif = with_uv_ctrl_timed(
        &fixture("TestNifFile_Skinned_SE.nif"),
        0x48,
        [0.5, 0.0, 0.0, 1.0],
    );
    let (_d, mut m, cmds) = session_on(&nif);
    assert_eq!(m.v3.clock.unwrap().range, (0.0, 2.0));
    assert!((u_at(&mut m, &cmds, 1.9) - 0.95).abs() < 1e-5);
}

/// A frequency-1 Reverse (ping-pong) controller over 0-1 needs 2 s for both legs: the clock reaches the way back.
#[test]
fn ping_pong_reaches_its_reverse_leg() {
    let nif = with_uv_ctrl_timed(
        &fixture("TestNifFile_Skinned_SE.nif"),
        0x48 | (1 << 1),
        [1.0, 0.0, 0.0, 1.0],
    );
    let (_d, mut m, cmds) = session_on(&nif);
    assert_eq!(m.v3.clock.unwrap().range, (0.0, 2.0));
    assert!((u_at(&mut m, &cmds, 0.5) - 0.5).abs() < 1e-5);
    assert!(
        (u_at(&mut m, &cmds, 1.75) - 0.25).abs() < 1e-5,
        "the reverse leg"
    );
}

/// B-P33 Important 2: a controller whose stop is +inf or NaN gets no clock and no slider, and the panel says why.
#[test]
fn invalid_controller_times_turn_playback_off() {
    for stop in [f32::INFINITY, f32::NAN] {
        let nif = with_uv_ctrl_timed(
            &fixture("TestNifFile_Skinned_SE.nif"),
            0x48,
            [1.0, 0.0, 0.0, stop],
        );
        let (_d, m, _cmds) = session_on(&nif);
        assert_eq!(m.v3.clock, None, "stop {stop}");
        assert!(m.edit.as_ref().unwrap().anim_invalid());
        let mut h = view(m);
        h.run_steps(2);
        h.get_by_label(ANIM_INVALID);
        assert!(h.query_by_label("Play").is_none(), "stop {stop}");
        assert_eq!(h.query_all_by_label("Time").count(), 0, "stop {stop}");
        assert_eq!(h.state().v3.clock, None);
    }
}

// ---- v2 B6: the Controllers tab (curve, key table, scrub, add and remove) ----

use nif::keys::encode_data;

fn float_keys(m: &MeshesModel, ctrl: u32) -> KeyGroup<f32> {
    match &m.edit.as_ref().unwrap().keys[&(ctrl, None)] {
        Ok(KeyData::Float(g)) => g.clone(),
        k => panic!("{k:?}"),
    }
}

/// Press on the node labelled `label`, move `dy` px up in three steps, release.
fn drag_up(h: &mut Harness<'static, MeshesModel>, label: &str, dy: f32) {
    h.get_by_label(label).scroll_to_me();
    h.run_steps(2);
    let c = h.get_by_label(label).rect().center();
    drag_from(h, c, dy);
}

/// Press at `c`, move `dy` px up in three steps, release.
fn drag_from(h: &mut Harness<'static, MeshesModel>, c: egui::Pos2, dy: f32) {
    h.hover_at(c);
    h.run_steps(1);
    h.drag_at(c);
    h.run_steps(1);
    for k in 1..=3 {
        h.hover_at(c - egui::vec2(0.0, dy * k as f32 / 3.0)); // the first step is past max_click_dist (6 px)
        h.run_steps(1);
    }
    h.drop_at(c - egui::vec2(0.0, dy));
    h.run_steps(3);
}

/// A handle drag is one SetKeys with the base it started from, one undo step; the release flushes, so the next
/// drag is a second op.
#[test]
fn dragging_a_key_records_one_undo_step() {
    let b = scenes::keyed_pair(scenes::Share::No, [1, 2]); // ctrl 2: var 11, LINEAR (0,0) (1,1), data 6
    let orig = nif::keys::read_data(&nif::header::Header::parse(&b).unwrap(), &b, 6).unwrap();
    let mut m = direct(b);
    select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Controllers");
    assert!(
        ops(h.state()).is_empty(),
        "drawing the curve records nothing"
    );
    drag_up(&mut h, "Emissive multiple key 2", 30.0);
    let got = ops(h.state());
    assert_eq!(got.len(), 1, "{got:?}");
    let Op::SetKeys {
        clip, keys, base, ..
    } = &got[0]
    else {
        panic!("{got:?}")
    };
    assert_eq!(*clip, None);
    assert_eq!(
        encode_data(base),
        encode_data(&orig),
        "the op keeps the keys the drag started from"
    );
    let g = float_keys(h.state(), 2);
    assert_eq!(
        &KeyData::Float(g.clone()),
        keys,
        "the patched file has the op's keys"
    );
    assert_eq!(
        (g.keys[0].time, g.keys[0].value),
        (0.0, 0.0),
        "the other key is untouched"
    );
    assert_eq!(g.keys[1].time, 1.0, "a vertical drag keeps the time");
    assert!(g.keys[1].value > 1.0, "dragged up: {}", g.keys[1].value);
    assert_eq!(
        h.state().edit.as_ref().unwrap().outcomes,
        vec![nif::edit::Outcome::Applied]
    );
    // The release flushed: a second drag is a second op. (Before any undo: `restore` also ends a drag, so a drag
    // after an undo/redo would be a new op even without the release's flush.)
    drag_up(&mut h, "Emissive multiple key 2", 30.0);
    assert_eq!(ops(h.state()).len(), 2, "{:?}", ops(h.state()));
    // One undo step per drag, back to the source.
    assert!(h.state_mut().undo_edit());
    assert_eq!(ops(h.state()).len(), 1);
    assert!(h.state_mut().undo_edit());
    assert!(ops(h.state()).is_empty());
    assert!(!h.state().edit.as_ref().unwrap().history.can_undo());
}

/// A typed key value is stored with its exact bits; a typed decreasing time is kept (never clamped) and its
/// validate_ops error shows under the controller.
#[test]
fn key_table_edit_sets_exact_value() {
    let mut m = direct(scenes::keyed_pair(scenes::Share::No, [1, 2]));
    select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Controllers");
    edit_number(&mut h, "Emissive multiple key 2 value", "0.3");
    let got = ops(h.state());
    assert_eq!(got.len(), 1, "{got:?}");
    let g = float_keys(h.state(), 2);
    assert_eq!(g.keys[1].value.to_bits(), 0.3f32.to_bits());
    assert_eq!(g.keys[1].time, 1.0);
    // The bytes on file: data block 6 (unshared, re-emitted in place, B4).
    let e = h.state().edit.as_ref().unwrap();
    let on_file = nif::keys::read_data(
        &nif::header::Header::parse(&e.patched).unwrap(),
        &e.patched,
        6,
    )
    .unwrap();
    assert_eq!(encode_data(&on_file), encode_data(&KeyData::Float(g)));
    // A QUADRATIC tangent (ctrl 3, var 20).
    edit_number(&mut h, "U offset key 1 backward", "-2.5");
    let KeyData::Float(q) = h.state().edit.as_ref().unwrap().keys[&(3, None)]
        .clone()
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(
        q.keys[0].backward.map(f32::to_bits),
        Some((-2.5f32).to_bits())
    );
    // Decreasing time: kept as typed, Error shown, no clamp.
    edit_number(&mut h, "Emissive multiple key 2 time", "-1");
    assert_eq!(float_keys(h.state(), 2).keys[1].time, -1.0);
    h.get_by_label("Key times must not decrease");
    let e = h.state().edit.as_ref().unwrap();
    assert!(
        e.op_warnings
            .iter()
            .any(|w| w.target == Target::Block { block: 2 }),
        "{:?}",
        e.op_warnings
    );
    // 3 typed edits; the time edit also moves the stop to the last key (B-P44) in the same step.
    assert_eq!(ops(h.state()).len(), 4, "each typed edit is its own step");
}

/// D9: Add controller appends a free controller and sets BSX bit 0; the Extra data tab then shows Animated ticked
/// and locked. Remove unlinks it; the bit stays (add-only) and is no longer locked.
#[test]
fn add_controller_ticks_and_locks_bsx_animated() {
    let mut m = direct(scenes::emit_scene("BSX", 0x82, true)); // S -> BSLSP 2, no controllers; root BSX 0x82
    select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Extra data");
    assert!(!h
        .get_by_role_and_label(AkRole::CheckBox, "Animated")
        .accesskit_node()
        .is_disabled());
    tab(&mut h, "Controllers");
    h.get_by_label("No controllers.");
    h.get_by_role_and_label(AkRole::ComboBox, "Add controller variable")
        .click();
    h.run_steps(3);
    h.get_by_role_and_label(AkRole::Button, "Emissive multiple (11)")
        .click();
    h.run_steps(3);
    h.get_by_label("Add controller to shader property").click();
    h.run_steps(4);
    let got = ops(h.state());
    let e = h.state().edit.as_ref().unwrap();
    let v = e
        .model
        .lighting
        .values()
        .next()
        .unwrap()
        .lighting
        .params
        .emissive_multiple;
    let Some(Op::AddController {
        class,
        var,
        keys,
        flags,
        freq,
        start,
        stop,
        ..
    }) = got.first()
    else {
        panic!("{got:?}")
    };
    assert_eq!(
        (class.as_str(), *var),
        ("BSLightingShaderPropertyFloatController", Some(11))
    );
    assert_eq!((*flags, *freq, *start, *stop), (0x48, 1.0, 0.0, 1.0));
    let KeyData::Float(g) = keys else {
        panic!("{keys:?}")
    };
    assert_eq!(g.key_type, Some(1));
    assert_eq!(
        g.keys.iter().map(|k| (k.time, k.value)).collect::<Vec<_>>(),
        [(0.0, v), (1.0, v)]
    );
    assert_eq!(e.outcomes, vec![nif::edit::Outcome::Applied]);
    assert_eq!(
        e.model.lighting.values().next().unwrap().prop.ctrls.len(),
        1
    );
    let bsx = e
        .model
        .extras
        .values()
        .find(|x| &*x.kind == "BSXFlags")
        .and_then(|x| x.value.clone());
    assert_eq!(bsx, Some(ExtraValue::Bsx(0x83)));
    tab(&mut h, "Extra data");
    let a = h.get_by_role_and_label(AkRole::CheckBox, "Animated");
    assert!(
        a.accesskit_node().is_disabled(),
        "locked while a free controller exists"
    );
    assert_eq!(
        a.accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::True)
    );
    tab(&mut h, "Controllers");
    h.get_by_label("Remove Emissive multiple controller")
        .click();
    h.run_steps(4);
    assert!(matches!(
        ops(h.state()).last(),
        Some(Op::RemoveController { .. })
    ));
    assert!(h
        .state()
        .edit
        .as_ref()
        .unwrap()
        .model
        .lighting
        .values()
        .next()
        .unwrap()
        .prop
        .ctrls
        .is_empty());
    tab(&mut h, "Extra data");
    let a = h.get_by_role_and_label(AkRole::CheckBox, "Animated");
    assert!(
        !a.accesskit_node().is_disabled(),
        "no free controller: unlocked"
    );
    assert_eq!(
        a.accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::True),
        "D9 is add-only"
    );
}

/// A click on the curve moves the clock (B3) to that time and pauses it; the next frame sends the material at
/// that time and renders with it.
#[test]
fn scrub_moves_the_preview_time() {
    let (_d, mut m, cmds) = playing_session();
    let (shape, _) = lit_shape(m.edit.as_ref().unwrap());
    m.select_edit(Selection::Shape(shape));
    m.v3.clock.as_mut().unwrap().play();
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Controllers");
    h.get_by_label("U offset curve").scroll_to_me();
    h.run_steps(2);
    let r = h.get_by_label("U offset curve").rect();
    // curve.rs PAD = 6 px; the var-20 controller spans [0, 1] (start 0, stop 1, keys at 0 and 1).
    let p = egui::pos2(r.left() + 6.0 + 0.25 * (r.width() - 12.0), r.top() + 10.0);
    cmds.try_iter().for_each(drop);
    h.hover_at(p);
    h.run_steps(1);
    h.drag_at(p);
    h.run_steps(1);
    h.drop_at(p);
    h.run_steps(3);
    let c = h.state().v3.clock.unwrap();
    assert!(!c.playing, "a scrub pauses");
    assert!((c.t - 0.25).abs() < 0.01, "{c:?}");
    let sent: Vec<V3Cmd> = cmds.try_iter().collect();
    assert!(
        sent.iter().any(|x| matches!(x, V3Cmd::SetMaterial { material, .. } if material.params.uv_offset[0] == c.t)),
        "the animated material at the scrubbed time"
    );
    assert!(sent
        .iter()
        .any(|x| matches!(x, V3Cmd::Render { time, .. } if *time == c.t)));
}

/// Removing a playing shape's only controller sends its file material once: the viewport must not keep the last
/// animated frame.
#[test]
fn a_removed_controller_restores_the_file_material() {
    let (_d, mut m, cmds) = playing_session();
    let (shape, lsp) = lit_shape(m.edit.as_ref().unwrap());
    let file_u = m.edit.as_ref().unwrap().model.lighting[&lsp]
        .lighting
        .params
        .uv_offset[0];
    m.v3.clock.as_mut().unwrap().scrub(0.5);
    m.tick_anim(0.0);
    assert!(cmds.try_iter().any(
        |c| matches!(c, V3Cmd::SetMaterial { material, .. } if material.params.uv_offset[0] == 0.5)
    ));
    let e = m.edit.as_ref().unwrap();
    let ctrl = e.model.lighting[&lsp].prop.ctrls[0];
    let c = &e.model.ctrls[&ctrl];
    let at = Anchor {
        block: ctrl,
        kind: c.class.to_string(),
        shape: e
            .model
            .shape(shape)
            .and_then(|s| s.name.as_deref().map(str::to_string)),
        role: Role::Ctrl {
            owner: nif::edit::Owner::Shader,
            class: c.class.to_string(),
            var: c.var,
            nth: 0,
        },
    };
    m.edit_push(vec![Op::RemoveController { at }], None);
    assert_eq!(
        m.edit.as_ref().unwrap().outcomes.last(),
        Some(&nif::edit::Outcome::Applied)
    );
    cmds.try_iter().for_each(drop);
    m.tick_anim(0.0);
    let sent: Vec<V3Cmd> = cmds.try_iter().collect();
    assert!(
        sent.iter().any(|c| matches!(c, V3Cmd::SetMaterial { id, material, .. } if id.0 == shape && material.params.uv_offset[0] == file_u)),
        "no file material in {} commands (V3Cmd is not Debug)",
        sent.len()
    );
    assert_eq!(m.v3.clock, None, "nothing animated: no clock");
}

// ---- v2 B6 fix round 1 (B-P35) ----

/// Replaces the keys of shape "S"'s float controller `var` (one undo step), before any drawing.
fn set_float_keys(m: &mut MeshesModel, var: u32, keys: KeyData) {
    let e = m.edit.as_ref().unwrap();
    let role = Role::Ctrl {
        owner: nif::edit::Owner::Shader,
        class: "BSLightingShaderPropertyFloatController".into(),
        var: Some(var),
        nth: 0,
    };
    let at = nif::edit::anchor_role(&e.model, Some(shape_named(e, "S")), role).unwrap();
    let base = e.keys[&(at.block, None)].clone().unwrap();
    m.edit_push(
        vec![Op::SetKeys {
            at,
            clip: None,
            clip_name: None,
            keys,
            base,
        }],
        None,
    );
}

fn lin2(a: f32, b: f32) -> KeyData {
    KeyData::Float(KeyGroup {
        key_type: Some(1),
        keys: vec![fkey(0.0, a, None), fkey(1.0, b, None)],
    })
}

/// B-P35 I1: the clip-shared guard is the alpha's (D9); a shared shader with a managed controller still adds.
#[test]
fn a_shared_shader_with_a_managed_controller_keeps_add_enabled() {
    // A and B share BSLSP 3; the added U offset controller has bit 0x20 (managed).
    let b = with_uv_ctrl_timed(
        &scenes::shared_alpha_with_testref_chain(),
        0x68,
        [1.0, 0.0, 0.0, 1.0],
    );
    let mut m = direct(b);
    select(&mut m, "A");
    let e = m.edit.as_ref().unwrap();
    let lsp = e.model.lighting.values().next().unwrap();
    assert!(lsp.prop.users.len() > 1 && lsp.prop.ctrls.iter().any(|&c| e.model.managed(c)));
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Controllers");
    let add = h.get_by_label("Add controller to shader property");
    assert!(!add.accesskit_node().is_disabled());
}

/// B-P35 I2: a tangent handle that projects outside the graph cannot start a drag there.
#[test]
fn a_drag_where_a_tangent_handle_projects_outside_the_graph_changes_no_keys() {
    let mut m = direct(scenes::keyed_pair(scenes::Share::No, [1, 2]));
    select(&mut m, "S");
    let KeyData::Float(mut q) = m.edit.as_ref().unwrap().keys[&(3, None)].clone().unwrap() else {
        panic!()
    };
    q.keys[0].backward = Some(100.0); // ctrl 3: QUADRATIC (0,0) -> (1,1)
    set_float_keys(&mut m, 20, KeyData::Float(q));
    let mut h = view(m);
    // Tall enough that nothing scrolls: an animated scroll_to_me would move the content under the drag.
    h.set_size(egui::vec2(1400.0, 1400.0));
    h.run_steps(2);
    tab(&mut h, "Controllers");
    h.run_steps(2);
    let graph = h.get_by_label("U offset curve").rect();
    let p = h
        .get_by_label("U offset key 1 backward handle")
        .rect()
        .center();
    assert!(
        p.y < graph.top() && p.y > 150.0,
        "the handle projects above the graph, on screen: {p:?} {graph:?}"
    );
    let before = (
        ops(h.state()),
        h.state().edit.as_ref().unwrap().keys.clone(),
    );
    drag_from(&mut h, p, 20.0);
    let e = h.state().edit.as_ref().unwrap();
    assert_eq!(ops(h.state()), before.0, "no op");
    assert_eq!(e.keys, before.1, "no key changed");
}

/// B-P35 I3: a flat curve at 1e8 maps to finite handle positions inside the graph.
#[test]
fn a_flat_curve_at_1e8_has_finite_handles() {
    let mut m = direct(scenes::keyed_pair(scenes::Share::No, [1, 2]));
    select(&mut m, "S");
    set_float_keys(&mut m, 11, lin2(1e8, 1e8));
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Controllers");
    let graph = h.get_by_label("Emissive multiple curve").rect();
    for k in ["Emissive multiple key 1", "Emissive multiple key 2"] {
        let c = h.get_by_label(k).rect().center();
        assert!(
            c.x.is_finite() && c.y.is_finite() && graph.contains(c),
            "{k}: {c:?} in {graph:?}"
        );
    }
}

/// B-P35 I3: keys the graph cannot map (an infinite value) show a line instead of the graph; the table stays.
#[test]
fn an_infinite_key_shows_the_out_of_range_line() {
    let mut m = direct(scenes::keyed_pair(scenes::Share::No, [1, 2]));
    select(&mut m, "S");
    set_float_keys(&mut m, 11, lin2(0.0, f32::INFINITY));
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Controllers");
    h.get_by_label("These keys are out of range for the graph.");
    assert!(h.query_by_label("Emissive multiple curve").is_none());
    h.get_by_label("U offset curve");
    h.get_by_label("Emissive multiple key 2 value");
}

/// Adds an LSCC "Emissive colour" (var 1, LINEAR keys `k1` at 0 and `k2` at 1) to `shape`'s shader as the last
/// op; returns the new controller's block.
fn add_colour_ctrl(m: &mut MeshesModel, shape: u32, k1: [f32; 3], k2: [f32; 3]) -> u32 {
    let e = m.edit.as_ref().unwrap();
    let at = nif::edit::anchor_role(&e.model, Some(shape), Role::Shader).unwrap();
    let key = |time: f32, value: [f32; 3]| Key {
        time,
        value,
        forward: None,
        backward: None,
        tbc: None,
    };
    m.edit_push(
        vec![Op::AddController {
            at: at.clone(),
            class: "BSLightingShaderPropertyColorController".into(),
            var: Some(1),
            keys: KeyData::Pos(KeyGroup {
                key_type: Some(1),
                keys: vec![key(0.0, k1), key(1.0, k2)],
            }),
            flags: 0x48,
            freq: 1.0,
            start: 0.0,
            stop: 1.0,
        }],
        None,
    );
    let e = m.edit.as_ref().unwrap();
    assert!(e.outcomes.iter().all(|o| *o == nif::edit::Outcome::Applied));
    let ctrls = &e.model.lighting[&at.block].prop.ctrls;
    *ctrls.last().unwrap()
}

/// `emit_scene` with an LSCC on S (see `add_colour_ctrl`).
fn colour_scene(k1: [f32; 3], k2: [f32; 3]) -> (MeshesModel, u32) {
    let mut m = direct(scenes::emit_scene("BSX", 0x82, true));
    let s = select(&mut m, "S");
    let ctrl = add_colour_ctrl(&mut m, s, k1, k2);
    (m, ctrl)
}

/// C5b: a colour curve has a colour strip under its graph (the graph keeps its height); a float curve has none.
#[test]
fn colour_strip_shows_on_colour_curves_only() {
    let (m, _) = colour_scene([0.2, 0.4, 0.6], [0.5, 0.5, 0.5]);
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Controllers");
    let graph = h.get_by_label("Emissive colour curve").rect();
    let strip = h.get_by_label("Emissive colour colour strip").rect();
    assert!(strip.top() >= graph.bottom(), "{strip:?} under {graph:?}");
    assert_eq!(graph.height(), 120.0, "the graph is unchanged");

    let mut h = view(clip_scene());
    h.run_steps(2);
    tab(&mut h, "Controllers");
    h.get_by_label("Emissive multiple curve");
    h.get_by_label("U offset curve");
    assert!(h.query_by_label("Emissive multiple colour strip").is_none());
    assert!(h.query_by_label("U offset colour strip").is_none());
}

/// C5b-D5: a click or drag on the colour strip scrubs like the graph.
#[test]
fn clicking_the_colour_strip_scrubs() {
    let (_d, mut m, _cmds) = playing_session();
    let (shape, _) = lit_shape(m.edit.as_ref().unwrap());
    add_colour_ctrl(&mut m, shape, [0.2, 0.4, 0.6], [0.8, 0.6, 0.4]);
    m.select_edit(Selection::Shape(shape));
    m.tick_anim(0.0);
    m.v3.clock.as_mut().unwrap().play();
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Controllers");
    // the strip sits further down than the scrub test's curve: wait for the scroll to settle
    reveal(&mut h, "Emissive colour colour strip");
    let r = h.get_by_label("Emissive colour colour strip").rect();
    // curve.rs PAD = 6 px; the colour controller spans [0, 1].
    let p = egui::pos2(r.left() + 6.0 + 0.25 * (r.width() - 12.0), r.center().y);
    h.hover_at(p);
    h.run_steps(1);
    h.drag_at(p);
    h.run_steps(1);
    h.drop_at(p);
    h.run_steps(3);
    let c = h.state().v3.clock.unwrap();
    assert!(!c.playing, "a scrub pauses");
    assert!((c.t - 0.25).abs() < 0.01, "{c:?}");
}

fn pos_keys(m: &MeshesModel, ctrl: u32) -> KeyGroup<[f32; 3]> {
    match &m.edit.as_ref().unwrap().keys[&(ctrl, None)] {
        Ok(KeyData::Pos(g)) => g.clone(),
        k => panic!("{k:?}"),
    }
}

/// B-P35 minor: a colour key's handle moves its own channel only.
#[test]
fn dragging_one_colour_channel_changes_only_it() {
    let (m, ctrl) = colour_scene([0.2, 0.4, 0.6], [0.8, 0.6, 0.4]);
    let pos = |m: &MeshesModel| pos_keys(m, ctrl);
    let before = pos(&m);
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Controllers");
    drag_up(&mut h, "Emissive colour key 2 G", 30.0);
    assert_eq!(ops(h.state()).len(), 2, "{:?}", ops(h.state()));
    let after = pos(h.state());
    let bits = |k: &Key<[f32; 3]>| (k.time.to_bits(), k.value.map(f32::to_bits));
    assert_eq!(
        bits(&after.keys[0]),
        bits(&before.keys[0]),
        "key 1 untouched"
    );
    assert_eq!(after.keys[1].time.to_bits(), before.keys[1].time.to_bits());
    assert_eq!(
        after.keys[1].value[0].to_bits(),
        before.keys[1].value[0].to_bits(),
        "R"
    );
    assert_eq!(
        after.keys[1].value[2].to_bits(),
        before.keys[1].value[2].to_bits(),
        "B"
    );
    assert!(
        after.keys[1].value[1] > 0.6,
        "G dragged up: {:?}",
        after.keys[1]
    );
}

/// Sets the popup DragValue (sRGB byte) of the open colour picker at `index` (0 R, 1 G, 2 B) to `byte`.
fn type_picker_byte(h: &mut Harness<'static, MeshesModel>, index: usize, byte: u8) {
    // The picker's own R/G/B DragValues: egui 0.37 names each by its prefix (0.36.2 left them unnamed).
    let mut spins: Vec<_> = h
        .query_all(egui_kittest::kittest::by().role(AkRole::SpinButton))
        .filter(|n| picker_channel(&n.accesskit_node()))
        .map(|n| (n.rect().min.x, n.accesskit_node().id()))
        .collect();
    assert_eq!(spins.len(), 3, "the picker's R, G and B");
    spins.sort_by(|a, b| a.0.total_cmp(&b.0));
    let id = spins[index].1;
    h.get(egui_kittest::kittest::by().predicate(move |n| n.id() == id))
        .focus();
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.run_steps(1);
    h.get(egui_kittest::kittest::by().predicate(move |n| n.id() == id))
        .type_text(&byte.to_string());
    h.run_steps(1);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
}

/// Owner follow-up: a colour key row has a picker swatch. A picker change is one merged SetKeys on that key (the
/// same path as an R/G/B drag) until the popup closes; channels the picker did not change keep their exact bits.
#[test]
fn key_table_colour_picker_sets_the_key_colour() {
    let (m, ctrl) = colour_scene([0.2, 0.4, 0.6], [0.8, 0.6, 0.4]);
    let before = pos_keys(&m, ctrl);
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Controllers");
    let sw = "Emissive colour key 2 colour";
    let n = h.get_by_role_and_label(AkRole::ColorWell, sw);
    assert!(!n.accesskit_node().is_disabled());
    n.click();
    h.run_steps(3);
    type_picker_byte(&mut h, 1, 51); // G
    type_picker_byte(&mut h, 1, 102); // G again, same popup: merged
    h.key_press(egui::Key::Escape); // closes the popup: the step ends
    h.run_steps(3);
    let got = ops(h.state());
    assert_eq!(got.len(), 2, "AddController + one SetKeys: {got:?}");
    let Op::SetKeys { clip, base, .. } = &got[1] else {
        panic!("{got:?}")
    };
    assert_eq!(*clip, None);
    assert_eq!(
        encode_data(base),
        encode_data(&KeyData::Pos(before.clone())),
        "the step keeps the keys it started from"
    );
    let after = pos_keys(h.state(), ctrl);
    let bits = |k: &Key<[f32; 3]>| (k.time.to_bits(), k.value.map(f32::to_bits));
    assert_eq!(
        bits(&after.keys[0]),
        bits(&before.keys[0]),
        "key 1 untouched"
    );
    let (b, a) = (&before.keys[1], &after.keys[1]);
    assert_eq!(a.time.to_bits(), b.time.to_bits());
    assert_eq!(a.value[0].to_bits(), b.value[0].to_bits(), "R");
    assert_eq!(a.value[2].to_bits(), b.value[2].to_bits(), "B");
    // C-P17': the key stores sRGB, so the picker's byte 102 is the stored value itself (0.4), not its linear.
    assert!(
        (a.value[1] * 255.0 - 102.0).abs() < 0.01,
        "G: {}",
        a.value[1]
    );
    // The popup closed and flushed: a second picker change is a second step.
    h.get_by_role_and_label(AkRole::ColorWell, sw).click();
    h.run_steps(3);
    type_picker_byte(&mut h, 2, 51); // B
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert_eq!(ops(h.state()).len(), 3, "{:?}", ops(h.state()));
    assert!(h.state_mut().undo_edit());
    assert!(h.state_mut().undo_edit());
    assert_eq!(ops(h.state()).len(), 1, "one undo step per popup");
    assert_eq!(bits(&pos_keys(h.state(), ctrl).keys[1]), bits(b));
}

/// Whether `n` is one of the open colour picker's channel boxes: egui 0.37 names a DragValue by its prefix, and the
/// picker's are "R", "G" and "B" (the key table's own boxes say "… key 2 G").
fn picker_channel(n: &egui_kittest::kittest::AccessKitNode<'_>) -> bool {
    matches!(n.label().as_deref(), Some("R" | "G" | "B"))
}

/// The open colour picker's R, G and B bytes (its own DragValues, named by their prefix), left to right.
fn picker_bytes(h: &Harness<'static, MeshesModel>) -> Vec<f64> {
    let mut spins: Vec<_> = h
        .query_all(egui_kittest::kittest::by().role(AkRole::SpinButton))
        .filter(|n| picker_channel(&n.accesskit_node()))
        .map(|n| (n.rect().min.x, n.accesskit_node().numeric_value().unwrap()))
        .collect();
    spins.sort_by(|a, b| a.0.total_cmp(&b.0));
    spins.into_iter().map(|s| s.1).collect()
}

/// sRGB 0.5 is byte 127.5 (ecolor rounds it to 127); as linear it would show 188.
fn assert_srgb_half(bytes: &[f64]) {
    assert_eq!(bytes.len(), 3, "{bytes:?}");
    assert!(
        bytes.iter().all(|b| (127.0..=128.0).contains(b)),
        "{bytes:?}"
    );
}

/// C-P17': NIF colours are sRGB, so a key of 0.5 shows sRGB byte ~128 in the picker (not 188, its value as linear).
#[test]
fn key_table_colour_picker_shows_the_key_as_srgb() {
    let (m, _) = colour_scene([0.2, 0.4, 0.6], [0.5, 0.5, 0.5]);
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Controllers");
    h.get_by_role_and_label(AkRole::ColorWell, "Emissive colour key 2 colour")
        .click();
    h.run_steps(3);
    assert_srgb_half(&picker_bytes(&h));
}

fn specular(m: &MeshesModel, a: u32) -> [f32; 3] {
    let e = m.edit.as_ref().unwrap();
    e.model.lighting[&e.model.shape(a).unwrap().shader.unwrap()]
        .lighting
        .params
        .specular_color
}

/// `shared_alpha_and_set` with shape A selected and its Specular colour set to `v`; returns A.
fn specular_scene(v: [f32; 3]) -> (MeshesModel, u32) {
    let mut m = direct(scenes::shared_alpha_and_set());
    let a = select(&mut m, "A");
    let e = m.edit.as_ref().unwrap();
    let at = nif::edit::anchor_role(&e.model, Some(a), Role::Shader).unwrap();
    let base = specular(&m, a);
    let ops = (0..3usize)
        .map(|i| Op::SetField {
            at: at.clone(),
            field: Field::SpecularColor(i as u8),
            value: Value::F32(v[i].to_bits()),
            base: Value::F32(base[i].to_bits()),
        })
        .collect();
    m.edit_push(ops, None);
    (m, a)
}

/// C-P25: a Shader tab colour outside 0-1 (or NaN) gets a disabled swatch with the reason on hover, like the key
/// swatch; the picker would clamp it.
#[test]
fn shader_colour_picker_is_disabled_outside_0_1() {
    for v in [[2.0, 0.5, 0.5], [0.5, -0.25, 0.5], [0.5, 0.5, f32::NAN]] {
        let (m, _) = specular_scene(v);
        let mut h = view(m);
        h.run_steps(2);
        let sw = h.get_by_role_and_label(AkRole::ColorWell, "Specular colour");
        assert!(sw.accesskit_node().is_disabled(), "{v:?}");
        let c = sw.rect().center();
        h.hover_at(c);
        h.run_steps(TOOLTIP_STEPS);
        h.get_by_label("Values outside 0-1: edit R, G and B directly.");
    }
}

/// C-P17': the Shader tab's colour picker shows the stored colour as sRGB and stores what is picked as sRGB; the
/// channels the picker did not change keep their exact bits, and the R/G/B fields show the raw stored floats.
#[test]
fn shader_colour_picker_shows_and_stores_srgb() {
    let (m, a) = specular_scene([0.5; 3]);
    let spec = |m: &MeshesModel| specular(m, a);
    let mut h = view(m);
    h.run_steps(2);
    h.get_by_role_and_label(AkRole::ColorWell, "Specular colour")
        .click();
    h.run_steps(3);
    assert_srgb_half(&picker_bytes(&h));
    type_picker_byte(&mut h, 1, 102); // G
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    let c = spec(h.state());
    assert_eq!(c[0].to_bits(), 0.5f32.to_bits(), "R untouched");
    assert_eq!(c[2].to_bits(), 0.5f32.to_bits(), "B untouched");
    assert!(
        (c[1] * 255.0 - 102.0).abs() < 0.01,
        "G stored as sRGB: {}",
        c[1]
    );
    let g = h.get_by_label("Specular colour G");
    assert_eq!(g.accesskit_node().numeric_value(), Some(c[1] as f64));
}

/// Note GQ2: the Shader tab's colour picker ends its undo step when its popup closes (like the key swatch): a second
/// popup pick is a second step, and one undo keeps the first pick.
#[test]
fn shader_colour_picker_flushes_on_popup_close() {
    let (mut m, a) = specular_scene([0.5; 3]);
    // `direct` has no document path (nothing to save): give it one.
    let d = tempfile::tempdir().unwrap();
    m.edit.as_mut().unwrap().draft_path = d.path().join("x.toml");
    let mut h = view(m);
    h.run_steps(2);
    let well = |h: &mut Harness<'static, MeshesModel>| {
        h.get_by_role_and_label(AkRole::ColorWell, "Specular colour")
            .click();
        h.run_steps(3);
    };
    well(&mut h);
    type_picker_byte(&mut h, 1, 102); // G
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    // A pick is 3 ops (R, G, B); the scene is 3 more.
    assert_eq!(ops(h.state()).len(), 6, "scene + the first pick");
    // The close flushed the merged step to disk.
    let e = h.state().edit.as_ref().unwrap();
    assert_eq!(load(&e.draft_path).unwrap().unwrap().ops.len(), 6, "saved");
    well(&mut h);
    type_picker_byte(&mut h, 2, 51); // B
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert_eq!(ops(h.state()).len(), 9, "the second pick is its own step");
    assert!(h.state_mut().undo_edit());
    let c = specular(h.state(), a);
    assert_eq!(ops(h.state()).len(), 6, "one undo");
    assert!(
        (c[1] * 255.0 - 102.0).abs() < 0.01,
        "first pick kept: {c:?}"
    );
    assert_eq!(c[2].to_bits(), 0.5f32.to_bits(), "B back to before");
}

/// Owner follow-up: a key outside 0-1 (an HDR emissive colour) gets a disabled swatch with the reason on hover; the
/// picker would clamp it. The in-range key on the same table keeps its picker.
#[test]
fn key_table_colour_picker_is_disabled_for_hdr_values() {
    let (m, _) = colour_scene([2.0, 0.5, 0.5], [0.8, 0.6, 0.4]);
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Controllers");
    let hdr = h.get_by_role_and_label(AkRole::ColorWell, "Emissive colour key 1 colour");
    assert!(hdr.accesskit_node().is_disabled());
    let c = hdr.rect().center();
    let ok = h.get_by_role_and_label(AkRole::ColorWell, "Emissive colour key 2 colour");
    assert!(!ok.accesskit_node().is_disabled());
    h.hover_at(c);
    h.run();
    h.get_by_label("Values outside 0-1: edit R, G and B directly.");
}

/// B-P35 minor (D11): TBC, XYZ and CONST keys are drawn, but their handles, table cells and buttons are disabled
/// and a drag records nothing.
/// Review minor: NaN and negative channels disable the swatch too (never clamped by the picker).
#[test]
fn key_table_colour_picker_is_disabled_for_nan_and_negative_values() {
    let (m, _) = colour_scene([f32::NAN, 0.5, 0.5], [0.5, -0.25, 0.5]);
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Controllers");
    for k in [1, 2] {
        let n = format!("Emissive colour key {k} colour");
        let sw = h.get_by_role_and_label(AkRole::ColorWell, &n);
        assert!(sw.accesskit_node().is_disabled(), "{n}");
    }
}

/// Review minor (D11): a TBC, XYZ or CONST colour key's swatch is disabled like its cells, though in range.
#[test]
fn key_table_colour_picker_is_disabled_for_tbc_xyz_and_const_keys() {
    for t in [3, 4, 5] {
        let (mut m, ctrl) = colour_scene([0.2, 0.4, 0.6], [0.8, 0.6, 0.4]);
        let role = Role::Ctrl {
            owner: nif::edit::Owner::Shader,
            class: "BSLightingShaderPropertyColorController".into(),
            var: Some(1),
            nth: 0,
        };
        let e = m.edit.as_ref().unwrap();
        let at = nif::edit::anchor_role(&e.model, Some(shape_named(e, "S")), role).unwrap();
        let base = pos_keys(&m, ctrl);
        let mut keys = base.clone();
        keys.key_type = Some(t);
        for k in &mut keys.keys {
            k.tbc = (t == 3).then_some([0.0; 3]);
        }
        m.edit_push(
            vec![Op::SetKeys {
                at,
                clip: None,
                clip_name: None,
                keys: KeyData::Pos(keys),
                base: KeyData::Pos(base),
            }],
            None,
        );
        assert_eq!(
            pos_keys(&m, ctrl).key_type,
            Some(t),
            "{:?}",
            m.edit.as_ref().unwrap().outcomes
        );
        let mut h = view(m);
        h.run_steps(2);
        tab(&mut h, "Controllers");
        h.get_by_label("These keys are previewed, not edited.");
        let sw = h.get_by_role_and_label(AkRole::ColorWell, "Emissive colour key 2 colour");
        assert!(sw.accesskit_node().is_disabled(), "type {t}");
    }
}

#[test]
fn tbc_xyz_and_const_keys_are_drawn_not_edited() {
    for t in [3, 4, 5] {
        let mut m = direct(scenes::keyed_pair(scenes::Share::No, [t, 1]));
        select(&mut m, "S");
        let mut h = view(m);
        h.run_steps(2);
        tab(&mut h, "Controllers");
        h.get_by_label("Emissive multiple curve");
        h.get_by_label("These keys are previewed, not edited.");
        assert!(
            h.query_by_label("Emissive multiple interpolation")
                .is_none(),
            "type {t}"
        );
        for l in [
            "Emissive multiple key 1",
            "Emissive multiple key 1 time",
            "Emissive multiple key 1 value",
            "Remove Emissive multiple key 1",
            "Add key to Emissive multiple",
        ] {
            assert!(
                h.get_by_label(l).accesskit_node().is_disabled(),
                "type {t}: {l}"
            );
        }
        drag_up(&mut h, "Emissive multiple key 2", 30.0);
        assert!(ops(h.state()).is_empty(), "type {t}: {:?}", ops(h.state()));
    }
}

// ---- v2 B7: managed clips (the Preview selector, the clip list, clip key edits) ----

/// `with_uv_ctrl(bytes)` plus, first in the same chain, a manager-controlled LSCF var 11 (Emissive multiple, flags
/// 0x68, a NiBlendFloatInterpolator), and a NiControllerSequence "Glow" whose one CB names it by ref with a keyed
/// NiFloatInterpolator LINEAR (0, 0) -> (1, 4); Loop, frequency 1, start 0, stop 1; text keys "start" at 0, "end"
/// at 1. Returns the bytes and the sequence block.
fn with_clip(bytes: &[u8]) -> (Vec<u8>, u32) {
    let keys = KeyData::Float(KeyGroup {
        key_type: Some(1),
        keys: [(0.0, 0.0), (1.0, 4.0)]
            .map(|(time, value)| Key {
                time,
                value,
                forward: None,
                backward: None,
                tbc: None,
            })
            .into(),
    });
    with_clip_at(bytes, true, 0.0, Some(keys), [1.0, 0.0, 1.0], 0)
}

/// [`with_clip`] with the CB bound by ref or (ref -1) by strings only, the CB interpolator's value, its keys (None:
/// data -1, the empty group), the sequence's (frequency, start, stop) and its cycle type (0 Loop, 1 Reverse).
fn with_clip_at(
    bytes: &[u8],
    by_ref: bool,
    value: f32,
    keys: Option<KeyData>,
    timing: [f32; 3],
    cycle: u32,
) -> (Vec<u8>, u32) {
    let m = nif::model::read_model(bytes).unwrap();
    let (shape, lsp) = m
        .shapes
        .iter()
        .find_map(|s| {
            s.shader
                .filter(|b| m.lighting.contains_key(b))
                .map(|b| (s, b))
        })
        .unwrap();
    let net = m.lighting[&lsp].prop.net.clone();
    let mut doc = nif::write::NifDoc::open(bytes).unwrap();
    let at = net.controller_at() - doc.header().offsets()[lsp as usize];
    let blend = doc
        .append(
            b"NiBlendFloatInterpolator",
            builder::W::default().u8(1).u8(0).f(0.0).f(0.0).0,
        )
        .unwrap();
    let ctrl = doc
        .append(
            b"BSLightingShaderPropertyFloatController",
            scenes::ctrl_body(
                net.controller,
                0x68,
                lsp as i32,
                builder::W::default().i32(blend as i32).u32(11),
            ),
        )
        .unwrap();
    doc.patch(lsp, at, &(ctrl as i32).to_le_bytes()).unwrap();
    let data = keys.map_or(-1, |k| {
        doc.append(b"NiFloatData", nif::keys::encode_data(&k))
            .unwrap() as i32
    });
    let interp = doc
        .append(
            b"NiFloatInterpolator",
            builder::W::default().f(value).i32(data).0,
        )
        .unwrap();
    let s = |doc: &mut nif::write::NifDoc, x: &[u8]| doc.string(x).unwrap();
    let (start, end) = (s(&mut doc, b"start"), s(&mut doc, b"end"));
    let tk = doc
        .append(
            b"NiTextKeyExtraData",
            builder::W::default()
                .i32(-1)
                .u32(2)
                .f(0.0)
                .i32(start)
                .f(1.0)
                .i32(end)
                .0,
        )
        .unwrap();
    let (name, prop, ctype, cid) = (
        s(&mut doc, b"Glow"),
        s(&mut doc, b"BSLightingShaderProperty"),
        s(&mut doc, b"BSLightingShaderPropertyFloatController"),
        s(&mut doc, b"11"),
    );
    let seq = builder::W::default()
        .i32(name)
        .u32(1)
        .u32(0)
        .i32(interp as i32)
        .i32(if by_ref { ctrl as i32 } else { -1 })
        .u8(0)
        .i32(shape.net.name)
        .i32(prop)
        .i32(ctype)
        .i32(cid)
        .i32(-1)
        .f(1.0)
        .i32(tk as i32)
        .u32(cycle)
        .fs(&timing)
        .i32(-1)
        .i32(-1)
        .u16(0)
        .0;
    let seq = doc.append(b"NiControllerSequence", seq).unwrap();
    let out = doc.bytes();
    let back = nif::model::read_model(&out).unwrap();
    assert!(back.problems.is_empty(), "{:?}", back.problems);
    assert!(back.managed(ctrl));
    assert_eq!(back.lighting[&lsp].prop.ctrls[0], ctrl);
    assert_eq!(back.sequences.len(), 1);
    assert_eq!(back.sequences[0].text_keys.len(), 2);
    assert_eq!(
        nif::anim::clips_of(&back, &out, ctrl),
        vec![seq],
        "the CB binds"
    );
    (out, seq)
}

/// G6, D4: Free runs the free chain and keeps the managed emissive at the property value; selecting the clip runs
/// the CB interpolator and pauses the free chain; back to Free swaps back. The UI selector and the clip's key table
/// drive the same model.
#[test]
fn clip_preview_swaps_interpolators_only_while_selected() {
    let (nif, seq) = with_clip(&with_uv_ctrl(&fixture("TestNifFile_Skinned_SE.nif")));
    let (_d, mut m, cmds) = session_on(&nif);
    let (shape, lsp) = lit_shape(m.edit.as_ref().unwrap());
    let base = m.edit.as_ref().unwrap().model.lighting[&lsp]
        .lighting
        .params
        .clone();
    let at = |m: &MeshesModel, t: f32| {
        let f = m.edit.as_ref().unwrap().frame_materials(t);
        f.into_iter()
            .find(|(id, _)| id.0 == shape)
            .unwrap()
            .1
            .params
    };
    let p = at(&m, 0.5);
    assert_eq!(
        (p.uv_offset[0], p.emissive_multiple),
        (0.5, base.emissive_multiple),
        "Free"
    );
    m.edit_with(|e| e.set_playing(Playing::Clip(seq)));
    let p = at(&m, 0.5);
    assert_eq!(
        (p.uv_offset[0], p.emissive_multiple),
        (base.uv_offset[0], 2.0),
        "the clip"
    );
    assert_eq!(
        m.edit.as_ref().unwrap().anim_range(),
        Some((0.0, 1.0)),
        "the clip's start and stop"
    );
    m.edit_with(|e| e.set_playing(Playing::Free));
    let p = at(&m, 0.5);
    assert_eq!(
        (p.uv_offset[0], p.emissive_multiple),
        (0.5, base.emissive_multiple),
        "Free again"
    );
    // The UI.
    m.select_edit(Selection::Shape(shape));
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Controllers");
    // Meshes UI T4: the clip is one weak line under its controller.
    h.get_by_label("In clip Glow · Loop · 0 to 1 s");
    h.get_by_label("Text keys: start at 0 s, end at 1 s");
    h.get_by_label("Pick this clip in Preview to see and edit its keys.");
    h.get_by_role_and_label(AkRole::ComboBox, "Preview animation")
        .click();
    h.run_steps(3);
    h.get_by_role_and_label(AkRole::Button, "Glow").click();
    h.run_steps(3);
    assert_eq!(h.state().edit.as_ref().unwrap().playing, Playing::Clip(seq));
    // The clip's own key table edits that clip (SetKeys with clip Some).
    edit_number(&mut h, "Emissive multiple in Glow key 2 value", "3");
    let last = ops(h.state()).pop();
    assert!(
        matches!(last, Some(Op::SetKeys { clip: Some(s), .. }) if s == seq),
        "{last:?}"
    );
    assert_eq!(
        h.state().edit.as_ref().unwrap().outcomes.last(),
        Some(&nif::edit::Outcome::Applied)
    );
    assert_eq!(
        at(h.state(), 0.5).emissive_multiple,
        1.5,
        "the edited clip curve: (0,0) -> (1,3)"
    );
    h.get_by_role_and_label(AkRole::ComboBox, "Preview animation")
        .click();
    h.run_steps(3);
    h.get_by_role_and_label(AkRole::Button, "Free chains")
        .click();
    h.run_steps(3);
    assert_eq!(h.state().edit.as_ref().unwrap().playing, Playing::Free);
    drop(cmds);
}

// ---- v2 B7 fix round 1 (B-P36) ----

const IDLE_FIRST: [f32; 3] = [0.9, 0.3, 0.1];
const IDLE_LAST: [f32; 3] = [0.1, 0.2, 0.3];

/// The skinned fixture with, first in the lit shape's chain, a manager-controlled LSCC "Emissive colour" (var 1, flags
/// 0x68, a NiBlendPoint3Interpolator) driven by a NiControllerSequence `name` (one CB bound by ref, NiPosData LINEAR
/// `IDLE_FIRST` at 0 and `IDLE_LAST` at 1; Loop, frequency 1, 0 to 1) that a NiControllerManager on the root lists.
fn with_idle_clip(name: impl AsRef<[u8]>) -> Vec<u8> {
    let bytes = fixture("TestNifFile_Skinned_SE.nif");
    let m = nif::model::read_model(&bytes).unwrap();
    let (shape, lsp) = m
        .shapes
        .iter()
        .find_map(|s| {
            s.shader
                .filter(|b| m.lighting.contains_key(b))
                .map(|b| (s, b))
        })
        .unwrap();
    let net = m.lighting[&lsp].prop.net.clone();
    let root = m.root.unwrap();
    let root_net = m.root_net.clone().unwrap();
    let mut doc = nif::write::NifDoc::open(&bytes).unwrap();
    let at = net.controller_at() - doc.header().offsets()[lsp as usize];
    let inv = f32::from_bits(0xFF7F_FFFF);
    let blend = doc
        .append(
            b"NiBlendPoint3Interpolator",
            nif::clips::encode_blend_interp("NiBlendPoint3Interpolator", &[inv; 3]).unwrap(),
        )
        .unwrap();
    let ctrl = doc
        .append(
            b"BSLightingShaderPropertyColorController",
            scenes::ctrl_body(
                net.controller,
                0x68,
                lsp as i32,
                builder::W::default().i32(blend as i32).u32(1),
            ),
        )
        .unwrap();
    doc.patch(lsp, at, &(ctrl as i32).to_le_bytes()).unwrap();
    let key = |time: f32, value: [f32; 3]| Key {
        time,
        value,
        forward: None,
        backward: None,
        tbc: None,
    };
    let keys = KeyData::Pos(KeyGroup {
        key_type: Some(1),
        keys: vec![key(0.0, IDLE_FIRST), key(1.0, IDLE_LAST)],
    });
    let data = doc
        .append(b"NiPosData", nif::keys::encode_data(&keys))
        .unwrap();
    let interp = doc
        .append(
            b"NiPoint3Interpolator",
            builder::W::default().fs(&IDLE_FIRST).i32(data as i32).0,
        )
        .unwrap();
    let s = |doc: &mut nif::write::NifDoc, x: &[u8]| doc.string(x).unwrap();
    let (start, end) = (s(&mut doc, b"start"), s(&mut doc, b"end"));
    let tk = doc
        .append(
            b"NiTextKeyExtraData",
            builder::W::default()
                .i32(-1)
                .u32(2)
                .f(0.0)
                .i32(start)
                .f(1.0)
                .i32(end)
                .0,
        )
        .unwrap();
    let (name, prop, ctype, cid) = (
        s(&mut doc, name.as_ref()),
        s(&mut doc, b"BSLightingShaderProperty"),
        s(&mut doc, b"BSLightingShaderPropertyColorController"),
        s(&mut doc, b"1"),
    );
    let seq = builder::W::default()
        .i32(name)
        .u32(1)
        .u32(0)
        .i32(interp as i32)
        .i32(ctrl as i32)
        .u8(0)
        .i32(shape.net.name)
        .i32(prop)
        .i32(ctype)
        .i32(cid)
        .i32(-1)
        .f(1.0)
        .i32(tk as i32)
        .u32(0)
        .fs(&[1.0, 0.0, 1.0])
        .i32(-1)
        .i32(-1)
        .u16(0)
        .0;
    let seq = doc.append(b"NiControllerSequence", seq).unwrap();
    let manager = doc
        .append(
            b"NiControllerManager",
            scenes::ctrl_body(
                root_net.controller,
                0x48,
                root as i32,
                builder::W::default().u8(0).u32(1).i32(seq as i32).i32(-1),
            ),
        )
        .unwrap();
    let at = root_net.controller_at() - doc.header().offsets()[root as usize];
    doc.patch(root, at, &(manager as i32).to_le_bytes())
        .unwrap();
    let out = doc.bytes();
    let back = nif::model::read_model(&out).unwrap();
    assert!(back.problems.is_empty(), "{:?}", back.problems);
    assert!(back.managed(ctrl));
    assert_eq!(back.sequences.len(), 1);
    out
}

/// [`with_idle_clip`] `name` in a scanned session. Returns the session, the shape and the sequence block.
fn idle_session(name: &str) -> (tempfile::TempDir, MeshesModel, Receiver<V3Cmd>, u32, u32) {
    let (d, m, cmds) = session_on(&with_idle_clip(name));
    let e = m.edit.as_ref().unwrap();
    let (shape, lsp) = lit_shape(e);
    assert_eq!(e.playing, Playing::Free);
    assert_ne!(
        e.model.lighting[&lsp].lighting.params.emissive_color,
        IDLE_FIRST
    );
    let seq = e.model.sequences[0].block;
    (d, m, cmds, shape, seq)
}

/// `shape`'s emissive colour in the preview at clock time `t`.
fn emissive_at(m: &MeshesModel, shape: u32, t: f32) -> [f32; 3] {
    m.edit
        .as_ref()
        .unwrap()
        .frame_materials(t)
        .into_iter()
        .find(|(id, _)| id.0 == shape)
        .unwrap()
        .1
        .params
        .emissive_color
}

/// C-P32 (IDLE-NM, the C5 in-game check): with no clip playing, a managed controller previews its clip's first key,
/// not the property value; the clip itself still plays its keys. C-P34 (b): with only held values the range has
/// zero length, so there is no clock (no controls), and the held values still reach the viewport.
#[test]
fn an_idle_clip_previews_its_first_key() {
    let (_d, mut m, cmds, shape, seq) = idle_session("Glow");
    for t in [0.0, 0.5, 1.0] {
        assert_eq!(emissive_at(&m, shape, t), IDLE_FIRST, "Free at {t}");
    }
    assert_eq!(m.edit.as_ref().unwrap().anim_range(), None, "held only");
    cmds.try_iter().for_each(drop);
    m.tick_anim(0.0);
    assert!(m.v3.clock.is_none(), "no clock, no controls");
    assert!(
        cmds.try_iter().any(|c| matches!(c, V3Cmd::SetMaterial { material, .. } if material.params.emissive_color == IDLE_FIRST)),
        "the held value is shown"
    );
    m.edit_with(|e| e.set_playing(Playing::Clip(seq)));
    assert_eq!(emissive_at(&m, shape, 1.0), IDLE_LAST, "the clip");
}

/// C-P34 (a), AUTO-SI: under Free chains the SpecialIdle clip plays by itself, with its own timing, and the clock
/// spans it.
#[test]
fn a_special_idle_clip_plays_under_free_chains() {
    let (_d, mut m, _cmds, shape, _) = idle_session(nif::anim::CONVERT_NAME);
    assert_eq!(emissive_at(&m, shape, 0.0), IDLE_FIRST);
    assert_eq!(emissive_at(&m, shape, 1.0), IDLE_LAST);
    let mid = emissive_at(&m, shape, 0.5);
    assert!((mid[0] - 0.5).abs() < 1e-6, "{mid:?}");
    assert_eq!(m.edit.as_ref().unwrap().anim_range(), Some((0.0, 1.0)));
    m.tick_anim(0.0);
    assert_eq!(m.v3.clock.unwrap().range, (0.0, 1.0));
}

/// The managed controller with_clip puts first in the lit shape's chain.
fn clip_ctrl(e: &NifEdit) -> u32 {
    let (_, lsp) = lit_shape(e);
    e.model.lighting[&lsp].prop.ctrls[0]
}

/// Review I2: a source reload that removes the selected clip's sequence returns the preview to the free chains, so
/// they still play (the selector is gone with the last sequence).
#[test]
fn losing_the_selected_clip_returns_to_the_free_chains() {
    let plain = with_uv_ctrl(&fixture("TestNifFile_Skinned_SE.nif"));
    let (nif, seq) = with_clip(&plain);
    let (_d, mut m, _cmds) = session_on(&nif);
    m.edit_with(|e| e.set_playing(Playing::Clip(seq)));
    m.edit_with(|e| e.reload_source(plain.clone()).unwrap());
    let e = m.edit.as_ref().unwrap();
    assert!(e.model.sequences.is_empty());
    assert_eq!(e.playing, Playing::Free);
    assert_eq!(
        e.anim_range(),
        Some((0.0, 1.0)),
        "the free U offset chain plays"
    );
    m.tick_anim(0.0);
    assert!(m.v3.clock.is_some());
}

/// Review I3: Add key on a string-bound clip with an empty key group starts at its CB interpolator's value (5).
#[test]
fn add_key_on_a_string_bound_empty_clip_starts_at_its_interpolator_value() {
    let plain = with_uv_ctrl(&fixture("TestNifFile_Skinned_SE.nif"));
    let (nif, seq) = with_clip_at(&plain, false, 5.0, None, [1.0, 0.0, 1.0], 0);
    let mut m = direct(nif);
    m.edit_with(|e| e.set_playing(Playing::Clip(seq)));
    let e = m.edit.as_ref().unwrap();
    let ctrl = clip_ctrl(e);
    assert!(
        matches!(&e.keys[&(ctrl, Some(seq))], Ok(KeyData::Float(g)) if g.keys.is_empty()),
        "{:?}",
        e.keys.get(&(ctrl, Some(seq)))
    );
    let (shape, _) = lit_shape(e);
    m.select_edit(Selection::Shape(shape));
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Controllers");
    h.get_by_role_and_label(AkRole::Button, "Add key to Emissive multiple in Glow")
        .click();
    h.run_steps(3);
    let last = ops(h.state()).pop();
    let Some(Op::SetKeys {
        clip: Some(s),
        clip_name,
        keys: KeyData::Float(g),
        ..
    }) = &last
    else {
        panic!("{last:?}")
    };
    assert_eq!(*s, seq);
    assert_eq!(
        clip_name.as_deref(),
        Some("Glow"),
        "C-P35 I1: the edit names its clip"
    );
    assert_eq!(g.keys.len(), 1);
    assert_eq!((g.keys[0].time, g.keys[0].value), (0.0, 5.0));
}

/// Review I4: every combo of the Controllers tab (Preview, the Variable combos, Add controller) ends inside the edit
/// panel at 1100 px.
#[test]
fn controllers_tab_combos_fit_the_edit_panel() {
    let (nif, _) = with_clip(&with_uv_ctrl(&fixture("TestNifFile_Skinned_SE.nif")));
    let (_d, mut m, _cmds) = session_on(&nif);
    let (shape, _) = lit_shape(m.edit.as_ref().unwrap());
    m.select_edit(Selection::Shape(shape));
    // A colour controller: its key rows (with the picker swatch) must fit too.
    add_colour_ctrl(&mut m, shape, [0.2, 0.4, 0.6], [0.8, 0.6, 0.4]);
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1100.0, 1600.0))
        .build_ui_state(
            |ui, m: &mut MeshesModel| sdc_gui::ui::meshes::show(ui, m),
            m,
        );
    h.run_steps(2);
    tab(&mut h, "Controllers");
    h.run_steps(3);
    let panel = support::panel(&h.ctx, "meshes-edit");
    let combos: Vec<_> = h
        .query_all(egui_kittest::kittest::by().role(AkRole::ComboBox))
        .filter(|n| n.rect().min.x >= panel.min.x)
        .collect();
    let names: Vec<String> = combos
        .iter()
        .map(|n| n.accesskit_node().label().unwrap_or_default())
        .collect();
    for want in [
        "Preview animation",
        "Emissive multiple variable",
        "U offset variable",
        "Add controller variable",
    ] {
        assert!(names.iter().any(|n| n == want), "{want} in {names:?}");
    }
    for n in &combos {
        let r = n.rect();
        assert!(
            r.max.x <= panel.max.x + 0.5,
            "{:?}: {r:?} past the edit panel's right edge {}",
            n.accesskit_node().label(),
            panel.max.x
        );
    }
    // The colour key rows: swatch and Remove end inside the panel (AccessKit rects are not clipped).
    for (role, name) in [
        (AkRole::ColorWell, "Emissive colour key 2 colour"),
        (AkRole::Button, "Remove Emissive colour key 2"),
    ] {
        let r = h.get_by_role_and_label(role, name).rect();
        assert!(
            r.max.x <= panel.max.x + 0.5,
            "{name}: {r:?} past the edit panel's right edge {}",
            panel.max.x
        );
    }
}

/// Review minor: a clip whose timing differs from the free chain (0 to 3 s) sets the clock's range when picked;
/// Free sets it back.
#[test]
fn picking_a_clip_sets_the_clock_range_to_its_timing() {
    let plain = with_uv_ctrl(&fixture("TestNifFile_Skinned_SE.nif"));
    let key = |time: f32, value: f32| Key {
        time,
        value,
        forward: None,
        backward: None,
        tbc: None,
    };
    let keys = KeyData::Float(KeyGroup {
        key_type: Some(1),
        keys: vec![key(0.0, 0.0), key(3.0, 3.0)],
    });
    let (nif, seq) = with_clip_at(&plain, true, 0.0, Some(keys), [1.0, 0.0, 3.0], 0);
    let (_d, mut m, _cmds) = session_on(&nif);
    assert_eq!(m.v3.clock.unwrap().range, (0.0, 1.0), "Free");
    m.edit_with(|e| e.set_playing(Playing::Clip(seq)));
    m.tick_anim(0.0);
    assert_eq!(m.v3.clock.unwrap().range, (0.0, 3.0), "the clip");
    m.edit_with(|e| e.set_playing(Playing::Free));
    m.tick_anim(0.0);
    assert_eq!(m.v3.clock.unwrap().range, (0.0, 1.0), "Free again");
}

/// Kept by Note GR1 (it was `clip_timing_moves_the_time_slider`): the Time slider spans a picked clip's
/// `(stop - start) / |frequency|`, and a Reverse clip counts both legs.
#[test]
fn a_clips_cycle_and_frequency_set_the_time_slider_span() {
    let plain = with_uv_ctrl(&fixture("TestNifFile_Skinned_SE.nif"));
    let key = |time: f32, value: f32| Key {
        time,
        value,
        forward: None,
        backward: None,
        tbc: None,
    };
    for (cycle, span) in [(0, 0.75), (1, 1.5)] {
        let keys = KeyData::Float(KeyGroup {
            key_type: Some(1),
            keys: vec![key(0.0, 0.0), key(3.0, 3.0)],
        });
        let (nif, seq) = with_clip_at(&plain, true, 0.0, Some(keys), [4.0, 0.0, 3.0], cycle);
        let (_d, mut m, _cmds) = session_on(&nif);
        m.edit_with(|e| e.set_playing(Playing::Clip(seq)));
        assert_eq!(
            m.edit.as_ref().unwrap().anim_range(),
            Some((0.0, span)),
            "cycle {cycle}"
        );
        m.tick_anim(0.0);
        assert_eq!(m.v3.clock.unwrap().range, (0.0, span), "cycle {cycle}");
    }
}

/// Kept by Note CU1 and GR1 (it was `preview_never_switches_clips_after_a_revert`, `preview_tells_non_ascii_clip_names_apart`):
/// the picked clip stays picked only while a sequence at its block keeps its name; a source reload that puts another
/// name there returns the Preview to Free chains. Names are Windows-1252 bytes, so "Café" and "Cafè" differ.
#[test]
fn a_picked_clip_stays_picked_only_while_its_block_keeps_its_name() {
    for (a, b) in [
        (&b"Glow"[..], &b"Dim"[..]),
        (&b"Caf\xe9"[..], &b"Caf\xe8"[..]),
    ] {
        let (_d, mut m, _cmds) = session_on(&with_idle_clip(a));
        let seq = m.edit.as_ref().unwrap().model.sequences[0].block;
        m.edit_with(|e| e.set_playing(Playing::Clip(seq)));
        m.edit_with(|e| e.reload_source(with_idle_clip(a)).unwrap());
        assert_eq!(
            m.edit.as_ref().unwrap().playing,
            Playing::Clip(seq),
            "same name"
        );
        m.edit_with(|e| e.reload_source(with_idle_clip(b)).unwrap());
        let e = m.edit.as_ref().unwrap();
        assert_eq!(
            e.model.sequences[0].block, seq,
            "the other name is on the same block"
        );
        assert_eq!(e.playing, Playing::Free, "{a:?} then {b:?}");
    }
}

// ---- v2 B8: In game (simulated) through pgpsim ----

use sdc_gui::model::meshes::pg_entries::*;
use sdc_gui::model::meshes::ScanTarget;

/// Mods (highest first) PGP, A, Low. A: x.nif, the PBR textures of `white`, PBRNIFPatcher\x.json (object form, BOM,
/// roughness 0.5 from `default`, a `smooth_angle` the preview does not show) and bad.json (no `default`: PGPatcher
/// skips the file). Low: a lower copy of the same path (hidden by A's) and a.json (sorts first, but Low is below A).
fn pg_files() -> tempfile::TempDir {
    let d = files_with(false, "+PGP\n+A\n+Low\n");
    for t in ["white", "white_n", "white_rmaos"] {
        put(
            d.path(),
            &format!("mods/A/textures/pbr/{t}.dds"),
            Some(b"dds"),
        );
    }
    put_x_json(d.path(), 0.5);
    put(
        d.path(),
        "mods/A/PBRNIFPatcher/bad.json",
        Some(br#"{"entries": [{"match_diffuse": "white", "roughness_scale": 0.7}]}"#),
    );
    put(
        d.path(),
        "mods/Low/PBRNifPatcher/x.json",
        Some(br#"[{"match_diffuse": "white", "roughness_scale": 0.1}]"#),
    );
    put(
        d.path(),
        "mods/Low/PBRNifPatcher/a.json",
        Some(br#"[{"match_diffuse": "white", "roughness_scale": 0.9}]"#),
    );
    d
}

/// A's x.json with `roughness` as its default roughness_scale.
fn put_x_json(root: &Path, roughness: f32) {
    let x = format!(
        "\u{feff}{{\"$schema\": \"x\", \"default\": {{\"roughness_scale\": {roughness}}}, \"entries\": [{{\"match_diffuse\": \"white\", \"smooth_angle\": 60}}]}}"
    );
    put(root, "mods/A/PBRNIFPatcher/x.json", Some(x.as_bytes()));
}

fn pg_session() -> (tempfile::TempDir, MeshesModel) {
    let d = pg_files();
    let m = pg_session_in(d.path());
    (d, m)
}

/// Scans `root`, selects A's x.nif and opens an edit session on it.
fn pg_session_in(root: &Path) -> MeshesModel {
    let mut m = MeshesModel::new(Some(root.join("scan.db")), MeshesSettings::default());
    m.procs = sdc_gui::model::meshes::no_procs;
    scan_and_select(&mut m, root);
    m.open_edit();
    assert_eq!(m.error, None);
    m
}

/// Polls until the PGPatcher JSON read lands (fix F2: it runs on a worker thread).
fn wait_pg(m: &mut MeshesModel) {
    let t = Instant::now();
    m.poll();
    while m.pg_reading() {
        assert!(
            t.elapsed() < Duration::from_secs(30),
            "the JSON read did not end"
        );
        std::thread::sleep(Duration::from_millis(5));
        m.poll();
    }
}

/// In game for `m`: switch, then wait for the JSON.
fn in_game(m: &mut MeshesModel) {
    m.set_view_source(false);
    wait_pg(m);
}

/// The Parsed content's material of shape block `b`.
fn shown(m: &MeshesModel, b: u32) -> nif::material::MaterialState {
    let Some(Content::Parsed { nif, .. }) = m.v3.content() else {
        panic!("the edit preview shows")
    };
    nif.shapes
        .iter()
        .find(|s| s.id.0 == b)
        .expect("the shape")
        .material
        .clone()
}

/// The In game lines of shape block `b`.
fn notes(m: &MeshesModel, b: u32) -> Vec<String> {
    let rows = m.edit.as_ref().unwrap().in_game.clone().expect("In game");
    rows.into_iter().find(|(id, _, _)| id.0 == b).unwrap().2
}

#[test]
fn in_game_mode_applies_pgpatcher_json_from_mods() {
    let d = pg_files();
    let inst = modscan::mo2::Instance::load(d.path(), Some("Default")).unwrap();
    let files: Vec<String> = pg_entries(&inst)
        .unwrap()
        .into_iter()
        .map(|f| f.file)
        .collect();
    assert_eq!(files, [r"PBRNIFPatcher\x.json", r"PBRNifPatcher\a.json"]);
    drop(d);

    let (_d, mut m) = pg_session();
    assert!(m.view_source, "the edit opens on Edited source");
    let e = m.edit.as_ref().unwrap();
    let (shape, shader) = lit_shape(e);
    assert_eq!(shown(&m, shape).slots[0].raw, r"textures\white.dds");
    let cur = e.model.lighting[&shader].lighting.params.uv_offset[0];
    let at = nif::edit::anchor_role(&e.model, Some(shape), Role::Shader).unwrap();
    m.edit_push(
        vec![Op::SetField {
            at,
            field: Field::UvOffset(0),
            value: Value::F32(0.25f32.to_bits()),
            base: Value::F32(cur.to_bits()),
        }],
        None,
    );
    m.select_edit(Selection::Shape(shape));
    let mut h = view(m);
    h.run_steps(4);
    h.get_by_label("In game").click();
    h.run_steps(4);
    wait_pg(h.state_mut());
    h.run_steps(4);
    h.get_by_label(r"In game: PGPatcher applies PBRNIFPatcher\x.json.");
    h.get_by_label("Not previewed: smooth_angle.");
    h.get_by_label(IN_GAME_SCOPE);
    let m = h.state();
    assert!(!m.view_source);
    assert!(m.v3.nif_view);
    assert!(m.edit.as_ref().unwrap().in_game.is_some());
    let g = shown(m, shape);
    assert_eq!(g.slots[0].raw, r"textures\pbr\white.dds");
    assert_ne!(g.slsf2 & nif::material::SLSF2_TRUE_PBR, 0);
    assert_eq!(
        g.params.specular_strength, 0.5,
        "A's x.json, through its default"
    );
    assert_eq!(g.params.uv_offset[0], 0.25, "the edit survives the sim");

    h.get_by_label("Source").click();
    h.run_steps(4);
    let m = h.state();
    assert!(m.view_source);
    assert!(m.edit.as_ref().unwrap().in_game.is_none());
    let s = shown(m, shape);
    assert_eq!(s.slots[0].raw, r"textures\white.dds");
    assert_eq!(s.params.uv_offset[0], 0.25);
}

/// Fix F2: the JSON is read on a worker ("Reading PGPatcher JSON…" until it lands) and kept across switches.
#[test]
fn a_second_switch_to_in_game_does_not_read_the_json_again() {
    let (_d, mut m) = pg_session();
    let (shape, _) = lit_shape(m.edit.as_ref().unwrap());
    m.select_edit(Selection::Shape(shape));
    m.set_view_source(false);
    assert!(m.pg_reading(), "the read runs on a worker");
    assert!(notes(&m, shape).is_empty(), "no line before the JSON lands");
    let mut h = view(m);
    h.run_steps(2);
    h.get_by_label(READING_JSON);
    wait_pg(h.state_mut());
    h.run_steps(3);
    assert!(h.query_by_label(READING_JSON).is_none());
    h.get_by_label(r"In game: PGPatcher applies PBRNIFPatcher\x.json.");
    let m = h.state_mut();
    let reads = m.pg_reads;
    m.set_view_source(true);
    m.set_view_source(false);
    assert!(!m.pg_reading());
    assert_eq!(m.pg_reads, reads, "a second switch does not read again");
    assert_eq!(
        notes(m, shape)[0],
        r"In game: PGPatcher applies PBRNIFPatcher\x.json."
    );
}

/// Note GH1 (review D5): two lit shapes, S0 naming `textures\white.dds` (A's x.json matches it; only the PBR copy
/// exists) and S1 naming `textures\nowhere.dds` (no file, no JSON entry).
fn supplied_and_missing() -> Vec<u8> {
    let mut b = builder::Nif::new(100);
    let names = [b.string("S0"), b.string("S1")];
    for (i, name) in names.iter().enumerate() {
        b.block("BSTriShape", one_tri(*name, 2 + i as i32));
    }
    for i in 0..2 {
        b.block("BSLightingShaderProperty", scenes::lsp(0, 4 + i));
    }
    for t in ["textures\\white.dds", "textures\\nowhere.dds"] {
        b.block("BSShaderTextureSet", scenes::texset(&[t]));
    }
    b.bytes()
}

const PG_SUPPLIED: &str =
    "1 texture path is replaced by PGPatcher at run time: switch to In game to see them";

/// Note GH1 (A26): the toggle reads `In game` / `Source` in and out of Edit.
#[test]
fn the_in_game_toggle_has_one_label_pair_in_and_out_of_edit() {
    let d = pg_files();
    let mut m = MeshesModel::new(Some(d.path().join("scan.db")), MeshesSettings::default());
    m.procs = sdc_gui::model::meshes::no_procs;
    scan_and_select(&mut m, d.path());
    assert!(m.pair.is_some(), "PGP's copy makes the pair");
    let mut h = view(m);
    h.run_steps(4);
    h.get_by_label("In game");
    h.get_by_label("Source");
    h.state_mut().open_edit();
    h.run_steps(4);
    assert!(h.state().edit.is_some());
    h.get_by_label("In game");
    h.get_by_label("Source");
    for old in ["In game (simulated)", "Edited source"] {
        assert!(h.query_by_label(old).is_none(), "{old}");
    }
}

/// Note GH1 (review D5, brief Review Focus 1): on the source side a path PGPatcher's JSON replaces is one grey line
/// naming the toggle's In game label, with the path on hover; a path nothing supplies stays a `Missing texture:` line.
#[test]
fn source_lists_paths_pgpatcher_supplies_on_one_line_and_keeps_missing_ones() {
    let d = pg_files();
    put(
        d.path(),
        "mods/A/meshes/x.nif",
        Some(&supplied_and_missing()),
    );
    let mut m = pg_session_in(d.path());
    assert!(m.view_source);
    // The source side reads the JSON too.
    wait_pg(&mut m);
    // What render3d lists for this content: neither source path is in the scan.
    m.v3.report = Some(render3d::LoadReport {
        missing_textures: vec!["textures/white.dds".into(), "textures/nowhere.dds".into()],
        ..Default::default()
    });
    m.poll();
    assert_eq!(m.edit.as_ref().unwrap().pg_supplied, ["textures/white.dds"]);
    assert_eq!(
        m.v3.report.as_ref().unwrap().missing_textures,
        ["textures/nowhere.dds"]
    );
    let mut h = view(m);
    h.run_steps(4);
    h.get_by_label("Missing texture: textures/nowhere.dds");
    assert!(h
        .query_by_label("Missing texture: textures/white.dds")
        .is_none());
    let paths = |h: &Harness<'_, MeshesModel>| h.query_all_by_label("textures/white.dds").count();
    let before = paths(&h);
    h.get_by_label(PG_SUPPLIED).hover();
    h.run_steps(30);
    assert_eq!(paths(&h), before + 1, "the path on hover");
    // In game shows PGPatcher's paths: no grey line there.
    h.state_mut().set_view_source(false);
    h.state_mut().poll();
    h.run_steps(4);
    assert!(h.query_by_label(PG_SUPPLIED).is_none());
}

/// `n` BSTriShapes, each with its own BSLSP and a texture set naming `textures\white.dds`.
fn lit_shapes(n: u32) -> Vec<u8> {
    let mut b = builder::Nif::new(100);
    let names: Vec<i32> = (0..n).map(|i| b.string(&format!("S{i}"))).collect();
    for (i, name) in names.iter().enumerate() {
        b.block("BSTriShape", one_tri(*name, (n as usize + i) as i32));
    }
    for i in 0..n {
        b.block(
            "BSLightingShaderProperty",
            scenes::lsp(0, (2 * n + i) as i32),
        );
    }
    for _ in 0..n {
        b.block(
            "BSShaderTextureSet",
            scenes::texset(&["textures\\white.dds"]),
        );
    }
    b.bytes()
}

/// Fix F1: rows are cached on the source material, the JSON generation and the scan. A drag tick In game on a
/// 20-shape mesh re-simulates the edited shape only; selection and switching back simulate nothing.
#[test]
fn a_drag_tick_in_game_resimulates_only_the_edited_shape() {
    let d = pg_files();
    put(d.path(), "mods/A/meshes/x.nif", Some(&lit_shapes(20)));
    let mut m = pg_session_in(d.path());
    in_game(&mut m);
    let e = m.edit.as_ref().unwrap();
    assert_eq!(e.sim_runs, 20, "one simulation per lit shape");
    let shape = e.model.shapes[3].block;
    let shader = e.model.shapes[3].shader.unwrap();
    let at = nif::edit::anchor_role(&e.model, Some(shape), Role::Shader).unwrap();
    for (i, v) in [0.1f32, 0.2].into_iter().enumerate() {
        m.edit_push(
            vec![Op::SetField {
                at: at.clone(),
                field: Field::UvOffset(0),
                value: Value::F32(v.to_bits()),
                base: Value::F32(0f32.to_bits()),
            }],
            Some(format!("uv-{shader}")),
        );
        assert_eq!(m.error, None);
        assert_eq!(m.edit.as_ref().unwrap().sim_runs, 21 + i, "tick {i}");
    }
    assert_eq!(shown(&m, shape).params.uv_offset[0], 0.2);
    assert_eq!(shown(&m, shape).slots[0].raw, r"textures\pbr\white.dds");
    m.select_edit(Selection::Shape(shape));
    m.set_view_source(true);
    m.set_view_source(false);
    assert_eq!(m.edit.as_ref().unwrap().sim_runs, 22, "nothing else");
}

/// H4 (fix F6): a file PGPatcher rejects for a shape (a missing texture) lets the next file win; when every file
/// is rejected, the first skip line shows.
#[test]
fn a_skipped_file_lets_the_next_one_win() {
    for renamed in [true, false] {
        let d = pg_files();
        std::fs::remove_file(d.path().join("mods/A/textures/pbr/white_rmaos.dds")).unwrap();
        if renamed {
            for t in ["other", "other_n", "other_rmaos"] {
                put(
                    d.path(),
                    &format!("mods/Low/textures/pbr/{t}.dds"),
                    Some(b"dds"),
                );
            }
            put(
                d.path(),
                "mods/Low/PBRNifPatcher/a.json",
                Some(br#"[{"match_diffuse": "white", "rename": "other", "roughness_scale": 0.9}]"#),
            );
        }
        let mut m = pg_session_in(d.path());
        let (shape, _) = lit_shape(m.edit.as_ref().unwrap());
        in_game(&mut m);
        let s = shown(&m, shape);
        if renamed {
            assert_eq!(
                notes(&m, shape),
                [r"In game: PGPatcher applies PBRNifPatcher\a.json."]
            );
            assert_eq!(s.slots[0].raw, r"textures\pbr\other.dds");
            assert_eq!(s.params.specular_strength, 0.9);
        } else {
            let n = notes(&m, shape);
            assert_eq!(n.len(), 1);
            assert!(
                n[0].starts_with(r"In game: PBRNIFPatcher\x.json is not applied ("),
                "{n:?}"
            );
            assert!(is_warning(&n[0]), "the skip line is amber");
            assert_eq!(s.slots[0].raw, r"textures\white.dds");
        }
    }
}

/// GE5: a wrong-typed match key stops PGPatcher 2.3's whole run, so no lower file wins: the shape stays as edited and
/// the line is red (`is_error`), not amber.
#[test]
fn wrong_typed_match_key_is_a_red_line() {
    let d = pg_files();
    put(
        d.path(),
        "mods/PGP/PBRNifPatcher/z.json",
        Some(br#"[{"match_diffuse": "white", "nif_filter": 5}]"#),
    );
    let mut m = pg_session_in(d.path());
    let (shape, _) = lit_shape(m.edit.as_ref().unwrap());
    m.select_edit(Selection::Shape(shape));
    in_game(&mut m);
    let line = r"In game: PGPatcher 2.3 crashes on this JSON: nif_filter is not a string (PBRNifPatcher\z.json).";
    assert_eq!(notes(&m, shape), [line]);
    assert!(is_error(line));
    assert!(!is_warning(line));
    assert_eq!(shown(&m, shape).slots[0].raw, r"textures\white.dds");
    let mut h = view(m);
    h.run_steps(4);
    h.get_by_label(line);
}

/// GE11: a crashing JSON anywhere in the load order stops the whole run (T1a, T1b), so every lit shape is red and
/// unpatched even when the file matches nothing or sits below the file that would patch it. One line per file.
#[test]
fn a_crashing_json_anywhere_makes_every_shape_red() {
    for (file, json, key) in [
        (
            "mods/PGP/PBRNifPatcher/z.json",
            &br#"[{"match_diffuse": 5}]"#[..],
            "match_diffuse",
        ),
        (
            "mods/Low/PBRNifPatcher/z.json",
            &br#"[{"match_diffuse": "other", "path_contains": 5}]"#[..],
            "path_contains",
        ),
    ] {
        let d = pg_files();
        put(d.path(), file, Some(json));
        let mut m = pg_session_in(d.path());
        let (shape, _) = lit_shape(m.edit.as_ref().unwrap());
        in_game(&mut m);
        let line = format!(
            r"In game: PGPatcher 2.3 crashes on this JSON: {key} is not a string (PBRNifPatcher\z.json)."
        );
        assert_eq!(notes(&m, shape), std::slice::from_ref(&line), "{file}");
        assert!(is_error(&line));
        assert_eq!(
            shown(&m, shape).slots[0].raw,
            r"textures\white.dds",
            "{file}"
        );
    }
}

/// One BSTriShape (0) whose BSLSP (1) has no texture set.
fn shape_without_texture_set() -> Vec<u8> {
    let mut n = builder::Nif::new(100);
    let s = n.string("S");
    n.block("BSTriShape", one_tri(s, 1));
    n.block("BSLightingShaderProperty", scenes::lsp(0, -1));
    n.bytes()
}

/// GE15: the crash lines come before the eligibility exits, so a shape with no texture set and a PG_IGNORE shape
/// get them too (PG_IGNORE keeps its own line after them).
#[test]
fn a_crashing_json_reaches_shapes_pgpatcher_would_not_patch() {
    let line = r"In game: PGPatcher 2.3 crashes on this JSON: match_diffuse is not a string (PBRNifPatcher\z.json).";
    for (nif, want) in [
        (shape_without_texture_set(), vec![line]),
        (shader_pg_ignore(1), vec![line, PG_IGNORED]),
    ] {
        let d = pg_files();
        put(d.path(), "mods/A/meshes/x.nif", Some(&nif));
        put(
            d.path(),
            "mods/PGP/PBRNifPatcher/z.json",
            Some(br#"[{"match_diffuse": 5}]"#),
        );
        let mut m = pg_session_in(d.path());
        let (shape, _) = lit_shape(m.edit.as_ref().unwrap());
        in_game(&mut m);
        assert_eq!(notes(&m, shape), want);
    }
}

/// GE14: the set preview reads the same whole-run crash lines: `pg_crashes` starts the JSON read and, once it
/// lands, returns one line per crashing file (an unrelated `[{"match_diffuse": 5}]` included).
#[test]
fn pg_crashes_are_the_meshes_views_crash_lines() {
    let d = pg_files();
    let mut m = pg_session_in(d.path());
    m.pg_crashes();
    wait_pg(&mut m);
    assert!(m.pg_crashes().is_empty(), "no crashing file");
    put(
        d.path(),
        "mods/PGP/PBRNifPatcher/z.json",
        Some(br#"[{"match_diffuse": 5}]"#),
    );
    let mut m = pg_session_in(d.path());
    m.pg_crashes();
    wait_pg(&mut m);
    assert_eq!(
        m.pg_crashes(),
        [
            r"In game: PGPatcher 2.3 crashes on this JSON: match_diffuse is not a string (PBRNifPatcher\z.json)."
        ]
    );
}

/// PG_IGNORE (value true) on the shape or its shader: In game keeps the edited material and says why; value 0
/// does not count.
#[test]
fn pg_ignore_shape_is_not_simulated() {
    for (role, v, ignored) in [(Role::Shape, 1u8, true), (Role::Shape, 0, false)] {
        let (_d, mut m) = pg_session();
        let e = m.edit.as_ref().unwrap();
        let (shape, _) = lit_shape(e);
        let at = nif::edit::anchor_role(&e.model, Some(shape), role.clone()).unwrap();
        m.edit_push(
            vec![Op::AddExtra {
                at,
                kind: "NiBooleanExtraData".into(),
                name: "PG_IGNORE".into(),
                value: ExtraValue::Bool(v),
            }],
            None,
        );
        assert_eq!(m.error, None);
        let edited = m.edit.as_ref().unwrap().materials();
        in_game(&mut m);
        let s = shown(&m, shape);
        if ignored {
            let rows = m.edit.as_ref().unwrap().in_game.clone().unwrap();
            let (_, mat, notes) = rows.iter().find(|(id, _, _)| id.0 == shape).unwrap();
            let before = &edited.iter().find(|(id, _)| id.0 == shape).unwrap().1;
            assert_eq!(mat, before, "{role:?}");
            assert_eq!(notes, &vec![PG_IGNORED.to_string()]);
            assert_eq!(s.slots[0].raw, r"textures\white.dds");
            assert_eq!(s.slsf2 & nif::material::SLSF2_TRUE_PBR, 0);
        } else {
            assert_eq!(
                s.slots[0].raw, r"textures\pbr\white.dds",
                "value 0 is simulated"
            );
        }
    }
}

/// One BSTriShape (0) whose BSLSP (1, texture set 2 naming `textures\white.dds`) carries NiBooleanExtraData
/// PG_IGNORE = `v` (3). The editor adds extra data to shapes and the root only, so the shader's is built in.
fn shader_pg_ignore(v: u8) -> Vec<u8> {
    let mut n = builder::Nif::new(100);
    let (s, pg) = (n.string("S"), n.string("PG_IGNORE"));
    n.block("BSTriShape", one_tri(s, 1));
    let lsp = scenes::lsp(0, 2);
    // shader type, name | extra count 1, the extra | controller and the rest
    let mut body = lsp[..8].to_vec();
    body.extend(W::default().u32(1).i32(3).0);
    body.extend(&lsp[12..]);
    n.block("BSLightingShaderProperty", body);
    n.block(
        "BSShaderTextureSet",
        scenes::texset(&["textures\\white.dds"]),
    );
    n.block("NiBooleanExtraData", W::default().i32(pg).u8(v).0);
    n.bytes()
}

/// One BSTriShape (0) carrying NiStringExtraData PG_STOCK (3), its BSLSP (1) and texture set (2) naming
/// `textures\white.dds`.
fn shape_pg_stock() -> Vec<u8> {
    let mut n = builder::Nif::new(100);
    let (s, pg, val) = (n.string("S"), n.string("PG_STOCK"), n.string("{}"));
    let tri = one_tri(s, 1);
    // name | extra count 1, the extra | the rest
    let mut body = tri[..4].to_vec();
    body.extend(W::default().u32(1).i32(3).0);
    body.extend(&tri[8..]);
    n.block("BSTriShape", body);
    n.block("BSLightingShaderProperty", scenes::lsp(0, 2));
    n.block(
        "BSShaderTextureSet",
        scenes::texset(&["textures\\white.dds"]),
    );
    n.block("NiStringExtraData", W::default().i32(pg).i32(val).0);
    n.bytes()
}

/// GE6 (R3): a shape with a PG_STOCK block gets the amber revert line; the simulation still runs on it as loaded.
#[test]
fn pg_stock_shape_gets_the_revert_line() {
    let d = pg_files();
    put(d.path(), "mods/A/meshes/x.nif", Some(&shape_pg_stock()));
    let mut m = pg_session_in(d.path());
    let (shape, _) = lit_shape(m.edit.as_ref().unwrap());
    in_game(&mut m);
    let n = notes(&m, shape);
    assert_eq!(
        n,
        [
            PG_STOCK_REVERT,
            r"In game: PGPatcher applies PBRNIFPatcher\x.json.",
            "Not previewed: smooth_angle."
        ]
    );
    assert!(is_warning(PG_STOCK_REVERT), "the revert line is amber");
    assert_eq!(shown(&m, shape).slots[0].raw, r"textures\pbr\white.dds");
}

/// GE16 (final review M2): a crashing run never reverts, so a PG_STOCK shape shows the crash line and no revert line.
#[test]
fn a_crashing_json_drops_the_pg_stock_revert_line() {
    let d = pg_files();
    put(d.path(), "mods/A/meshes/x.nif", Some(&shape_pg_stock()));
    put(
        d.path(),
        "mods/PGP/PBRNifPatcher/z.json",
        Some(br#"[{"match_diffuse": 5}]"#),
    );
    let mut m = pg_session_in(d.path());
    let (shape, _) = lit_shape(m.edit.as_ref().unwrap());
    in_game(&mut m);
    assert_eq!(
        notes(&m, shape),
        [
            r"In game: PGPatcher 2.3 crashes on this JSON: match_diffuse is not a string (PBRNifPatcher\z.json)."
        ]
    );
}

/// PG_IGNORE on the shader counts as on the shape (fix F6); value 0 does not.
#[test]
fn pg_ignore_on_the_shader_is_not_simulated() {
    for (v, ignored) in [(1u8, true), (0, false)] {
        let d = pg_files();
        put(d.path(), "mods/A/meshes/x.nif", Some(&shader_pg_ignore(v)));
        let mut m = pg_session_in(d.path());
        let e = m.edit.as_ref().unwrap();
        let (shape, shader) = lit_shape(e);
        assert_eq!(
            e.model.extras_of(&e.model.lighting[&shader].prop.net).len(),
            1,
            "the fixture's shader extra"
        );
        in_game(&mut m);
        let s = shown(&m, shape);
        if ignored {
            assert_eq!(notes(&m, shape), [PG_IGNORED]);
            assert_eq!(s.slots[0].raw, r"textures\white.dds");
        } else {
            assert_eq!(s.slots[0].raw, r"textures\pbr\white.dds");
        }
    }
}

/// A PG_IGNORE pushed while In game shows takes effect at once (the cache keeps the check outside).
#[test]
fn pg_ignore_pushed_in_game_takes_effect() {
    let (_d, mut m) = pg_session();
    let (shape, _) = lit_shape(m.edit.as_ref().unwrap());
    in_game(&mut m);
    assert_eq!(shown(&m, shape).slots[0].raw, r"textures\pbr\white.dds");
    let e = m.edit.as_ref().unwrap();
    let at = nif::edit::anchor_role(&e.model, Some(shape), Role::Shape).unwrap();
    m.edit_push(
        vec![Op::AddExtra {
            at,
            kind: "NiBooleanExtraData".into(),
            name: "PG_IGNORE".into(),
            value: ExtraValue::Bool(1),
        }],
        None,
    );
    assert_eq!(notes(&m, shape), [PG_IGNORED]);
    assert_eq!(shown(&m, shape).slots[0].raw, r"textures\white.dds");
}

/// A folder scan reads no JSON and says so (fix F6).
#[test]
fn a_folder_scan_says_in_game_needs_an_instance() {
    let (d, mut m) = pg_session();
    let (shape, _) = lit_shape(m.edit.as_ref().unwrap());
    m.target = Some(ScanTarget::Folder(d.path().into()));
    m.select_edit(Selection::Shape(shape));
    in_game(&mut m);
    assert_eq!(notes(&m, shape), [NO_MATCH]);
    let mut h = view(m);
    h.run_steps(4);
    h.get_by_label(NO_INSTANCE);
    h.get_by_label(NO_MATCH);
    assert!(h.query_by_label(IN_GAME_SCOPE).is_none());
}

/// Fix F3: an instance that cannot be read says so, in place of the scope line; no shape claims "no match".
#[test]
fn an_unreadable_instance_has_its_own_line() {
    let d = pg_files();
    let mut m = MeshesModel::new(Some(d.path().join("scan.db")), MeshesSettings::default());
    m.procs = sdc_gui::model::meshes::no_procs;
    scan_and_select(&mut m, d.path());
    // Before Edit: the JSON read starts when the edit opens (Note GH1).
    std::fs::remove_file(d.path().join("ModOrganizer.ini")).unwrap();
    m.open_edit();
    let (shape, _) = lit_shape(m.edit.as_ref().unwrap());
    m.select_edit(Selection::Shape(shape));
    in_game(&mut m);
    assert!(notes(&m, shape).is_empty());
    let mut h = view(m);
    h.run_steps(4);
    h.get_by_label_contains(INSTANCE_UNREADABLE);
    assert!(h.query_by_label(IN_GAME_SCOPE).is_none());
    assert!(h.query_by_label(NO_MATCH).is_none());
}

/// Fix F5: a rescan re-reads the JSON (a session reopened after it sees the new file).
#[test]
fn a_rescan_reads_the_json_again() {
    let (d, mut m) = pg_session();
    let (shape, _) = lit_shape(m.edit.as_ref().unwrap());
    in_game(&mut m);
    assert_eq!(shown(&m, shape).params.specular_strength, 0.5);
    put_x_json(d.path(), 0.3);
    m.set_view_source(true);
    in_game(&mut m);
    assert_eq!(
        shown(&m, shape).params.specular_strength,
        0.5,
        "not watched"
    );
    scan_and_select(&mut m, d.path());
    m.open_edit();
    in_game(&mut m);
    assert_eq!(shown(&m, shape).params.specular_strength, 0.3);
}

/// Fix F9: a mesh PGPatcher already made PBR (no entry matches, slot 5 is an existing `_rmaos`) says so.
#[test]
fn an_already_pbr_shape_says_so() {
    let (_d, mut m) = pg_session();
    let e = m.edit.as_ref().unwrap();
    let (shape, shader) = lit_shape(e);
    let set = e.model.lighting[&shader].texture_set.unwrap();
    let raw = |i: usize| {
        e.model.texsets[&set]
            .raw
            .slots
            .get(i)
            .map(|s| String::from_utf8_lossy(s).into_owned())
            .unwrap_or_default()
    };
    let at = nif::edit::anchor_shape(&e.model, shape);
    let ops = vec![
        Op::SetTexturePath {
            at: at.clone(),
            slot: 0,
            path: r"textures\stone.dds".into(),
            base: raw(0),
        },
        Op::SetTexturePath {
            at,
            slot: 5,
            path: r"textures\pbr\white_rmaos.dds".into(),
            base: raw(5),
        },
    ];
    m.edit_push(ops, None);
    assert!(
        m.edit
            .as_ref()
            .unwrap()
            .doc
            .ops
            .iter()
            .all(|o| o.status == OpStatus::Applied),
        "{:?}",
        m.edit.as_ref().unwrap().outcomes
    );
    in_game(&mut m);
    assert_eq!(notes(&m, shape), [ALREADY_PBR]);
}

/// Fix round 2 (B-P38): the first In game build of a 20-shape mesh against a 5000-entry JSON tests only each
/// shape's indexed candidates, never every entry, on the UI thread.
#[test]
fn the_first_in_game_build_tests_only_indexed_entries() {
    let d = pg_files();
    put(d.path(), "mods/A/meshes/x.nif", Some(&lit_shapes(20)));
    let mut big: Vec<String> = (0..5000)
        .map(|i| format!("{{\"match_diffuse\": \"set{i}\\\\tex{i}\"}}"))
        .collect();
    big.push("{\"match_diffuse\": \"white\", \"roughness_scale\": 0.2}".into());
    let json = format!("[{}]", big.join(","));
    put(
        d.path(),
        "mods/A/PBRNIFPatcher/big.json",
        Some(json.as_bytes()),
    );
    let mut m = pg_session_in(d.path());
    in_game(&mut m);
    let e = m.edit.as_ref().unwrap();
    assert_eq!(e.sim_runs, 20);
    // per shape: one candidate in each of big.json, x.json and Low's a.json (4 files read, bad.json dropped)
    assert_eq!(e.match_tests, 20 * 3, "a full scan would test 20 x 5003");
    let shape = e.model.shapes[0].block;
    assert_eq!(
        notes(&m, shape)[0],
        r"In game: PGPatcher applies PBRNIFPatcher\big.json."
    );
}

#[test]
fn controllers_tab_names_pbr_variables_as_the_shader_tab_does() {
    let mut m = direct(ctrl_scene_vars(0x82, [9, 10]));
    let s = select(&mut m, "S");
    // Tall, so the Add controller row is on screen.
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1400.0, 1800.0))
        .build_ui_state(
            |ui, m: &mut MeshesModel| sdc_gui::ui::meshes::show(ui, m),
            m,
        );
    h.run_steps(2);
    tab(&mut h, "Controllers");
    h.get_by_role_and_label(AkRole::ComboBox, "Glossiness variable");
    h.get_by_role_and_label(AkRole::ComboBox, "Specular strength variable");
    set_slsf2(&mut h, s, 1 << 23);
    assert!(h.query_by_label("Glossiness").is_none());
    h.get_by_role_and_label(AkRole::ComboBox, "Specular level variable");
    h.get_by_role_and_label(AkRole::ComboBox, "Roughness scale variable");
    // The Add controller list offers the PBR names too.
    h.get_by_role_and_label(AkRole::ComboBox, "Add controller variable")
        .click();
    h.run_steps(3);
    h.get_by_role_and_label(AkRole::Button, "Specular level (9)");
    h.get_by_role_and_label(AkRole::Button, "Roughness scale (10)");
}

#[test]
fn controllers_tab_names_lscc_specular_colour_by_pbr_material() {
    let cases = [
        (1u32 << 23, "Specular colour (no effect on PBR)"),
        ((1 << 23) | (1 << 24), "Coat colour"),
        ((1 << 23) | (1 << 26), "Subsurface colour"),
        (1 << 24, "Specular colour"), // not PBR: bits 24 and 26 mean nothing
    ];
    for (bits, want) in cases {
        let mut m = direct(ctrl_scene(0x82));
        let s = select(&mut m, "S");
        let e = m.edit.as_ref().unwrap();
        let at = nif::edit::anchor_role(&e.model, Some(s), Role::Shader).unwrap();
        let key = |time: f32, value: [f32; 3]| Key {
            time,
            value,
            forward: None,
            backward: None,
            tbc: None,
        };
        m.edit_push(
            vec![Op::AddController {
                at,
                class: "BSLightingShaderPropertyColorController".into(),
                var: Some(0),
                keys: KeyData::Pos(KeyGroup {
                    key_type: Some(1),
                    keys: vec![key(0.0, [0.2, 0.4, 0.6]), key(1.0, [0.8, 0.6, 0.4])],
                }),
                flags: 0x48,
                freq: 1.0,
                start: 0.0,
                stop: 1.0,
            }],
            None,
        );
        let mut h = view(m);
        h.run_steps(2);
        set_slsf2(&mut h, s, bits);
        tab(&mut h, "Controllers");
        h.get_by_role_and_label(AkRole::ComboBox, &format!("{want} variable"));
    }
}

// ---- B-P39: guards from the B9 in-game controller check ----

const ENV_PBR: &str = "Env map scale crashes the game on PBR materials.";

/// An exportable session on `b` (temp edit mod).
fn exportable(b: Vec<u8>) -> (tempfile::TempDir, NifEdit) {
    let d = tempfile::tempdir().unwrap();
    let edit_mod = d.path().join("mods").join(EDIT_MOD);
    let e = NifEdit::open(
        b,
        copy("A", 1, false),
        "meshes/x.nif",
        None,
        Some(&edit_mod),
        None,
    )
    .unwrap();
    (d, e)
}

fn pbr_on(e: &NifEdit) -> Op {
    let at = nif::edit::anchor_role(&e.model, Some(0), Role::Shader).unwrap();
    let base = e.model.lighting[&1].lighting.slsf2;
    Op::SetBits {
        at,
        field: Field::Slsf2,
        mask: 1 << 23,
        value: 1 << 23,
        base,
    }
}

#[test]
fn env_map_scale_on_pbr_blocks_export_after_a_switch_and_in_the_source() {
    // The shape turns PBR after the var-8 controller exists.
    let (_d, mut e) = exportable(ctrl_scene_vars(0x82, [8, 20]));
    assert!(e.warnings.iter().all(|w| w.text != ENV_PBR));
    let op = pbr_on(&e);
    e.push(vec![op], None);
    assert_eq!(e.export_block().as_deref(), Some(ENV_PBR));
    // A file that already has it: an unrelated edit still cannot export.
    let (_d2, mut e2) = exportable(e.patched.to_vec());
    let at = nif::edit::anchor_role(&e2.model, Some(0), Role::Shader).unwrap();
    e2.push(
        vec![Op::SetField {
            at,
            field: Field::Glossiness,
            value: Value::F32(50f32.to_bits()),
            base: Value::F32(80f32.to_bits()),
        }],
        None,
    );
    assert_eq!(e2.export_block().as_deref(), Some(ENV_PBR));
}

#[test]
fn controllers_tab_shows_the_b9_guards() {
    // Var 0 (refraction) and var 10 on a shape whose Specular flag is off.
    let mut m = direct(ctrl_scene_vars(0x82, [0, 10]));
    select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Controllers");
    h.get_by_label("Refraction strength has no visible effect in game.");
    h.get_by_label("This controller has no effect: Specular is off when the mesh loads.");
    // Two controllers on one variable.
    let mut m = direct(ctrl_scene_vars(0x82, [11, 11]));
    select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Controllers");
    h.get_by_label("Two controllers drive Emissive multiple: only the last one plays.");
    // Var 8 on PBR: the red error under the controller.
    let mut m = direct(ctrl_scene_vars(0x82, [8, 20]));
    let s = select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(2);
    set_slsf2(&mut h, s, 1 << 23);
    tab(&mut h, "Controllers");
    h.get_by_label(ENV_PBR);
}

#[test]
fn add_controller_offers_no_env_map_scale_on_pbr() {
    let mut m = direct(ctrl_scene(0x82));
    let s = select(&mut m, "S");
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1400.0, 1800.0))
        .build_ui_state(
            |ui, m: &mut MeshesModel| sdc_gui::ui::meshes::show(ui, m),
            m,
        );
    h.run_steps(2);
    tab(&mut h, "Controllers");
    h.get_by_role_and_label(AkRole::ComboBox, "Add controller variable")
        .click();
    h.run_steps(3);
    let env = h.get_by_label("Env map scale (8)");
    assert!(!env.accesskit_node().is_disabled());
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    set_slsf2(&mut h, s, 1 << 23);
    h.get_by_role_and_label(AkRole::ComboBox, "Add controller variable")
        .click();
    h.run_steps(3);
    let env = h.get_by_label("Env map scale (8)");
    assert!(env.accesskit_node().is_disabled());
    env.hover();
    h.run_steps(30);
    h.get_by_label(ENV_PBR);
}

/// B-P40: the Variable combo of an existing controller offers no env map scale on PBR either.
#[test]
fn variable_combo_offers_no_env_map_scale_on_pbr() {
    let mut m = direct(ctrl_scene(0x82));
    let s = select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Controllers");
    h.get_by_role_and_label(AkRole::ComboBox, "Emissive multiple variable")
        .click();
    h.run_steps(3);
    assert!(!h
        .get_by_label("Env map scale (8)")
        .accesskit_node()
        .is_disabled());
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    set_slsf2(&mut h, s, 1 << 23);
    h.get_by_role_and_label(AkRole::ComboBox, "Emissive multiple variable")
        .click();
    h.run_steps(3);
    let env = h.get_by_label("Env map scale (8)");
    assert!(env.accesskit_node().is_disabled());
    env.hover();
    h.run_steps(30);
    h.get_by_label(ENV_PBR);
    // A controller that already has var 8 still shows it, with its error.
    let mut m = direct(ctrl_scene_vars(0x82, [8, 20]));
    let s = select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(2);
    set_slsf2(&mut h, s, 1 << 23);
    tab(&mut h, "Controllers");
    h.get_by_role_and_label(AkRole::ComboBox, "Env map scale variable");
    h.get_by_label(ENV_PBR);
}

// ---- B-P41: the var-8 crash through PGPatcher (final review I1) ----

const PG_ENV: &str =
    "PGPatcher makes this shape PBR: its Env map scale controller will crash the game.";

/// x.nif is not PBR, so the editor adds var 8; A's x.json makes the shape PBR in game. In game shows a red line, the
/// edit view a Warning from the cached simulation, and export stays open (PGPatcher's output depends on play-time JSON).
#[test]
fn env_map_scale_warns_when_pgpatcher_makes_the_shape_pbr() {
    let (_d, mut m) = pg_session();
    let e = m.edit.as_ref().unwrap();
    let (shape, _) = lit_shape(e);
    let at = nif::edit::anchor_role(&e.model, Some(shape), Role::Shader).unwrap();
    m.edit_push(
        vec![Op::AddController {
            at,
            class: "BSLightingShaderPropertyFloatController".into(),
            var: Some(8),
            keys: KeyData::Float(KeyGroup {
                key_type: Some(1),
                keys: vec![fkey(0.0, 1.0, None), fkey(1.0, 0.5, None)],
            }),
            flags: 0x48,
            freq: 1.0,
            start: 0.0,
            stop: 1.0,
        }],
        None,
    );
    m.select_edit(Selection::Shape(shape));
    let e = m.edit.as_ref().unwrap();
    assert!(e.doc.ops.iter().all(|d| d.status == OpStatus::Applied));
    assert!(
        notes_or_none(&m, shape).is_none(),
        "Edited source: no In game rows"
    );
    in_game(&mut m);
    let n = notes(&m, shape);
    assert!(n.iter().any(|l| l == PG_ENV), "{n:?}");
    // Back on Edited source: the Warning under the controller comes from the cached simulation.
    let runs = m.edit.as_ref().unwrap().sim_runs;
    m.set_view_source(true);
    let e = m.edit.as_ref().unwrap();
    assert_eq!(e.sim_runs, runs, "no new simulation");
    assert_eq!(e.blocked, None, "export is not blocked");
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Controllers");
    h.get_by_label(PG_ENV);
    // Without the controller, the Warning goes.
    let m = h.state_mut();
    let role = Role::Ctrl {
        owner: nif::edit::Owner::Shader,
        class: "BSLightingShaderPropertyFloatController".into(),
        var: Some(8),
        nth: 0,
    };
    let at = nif::edit::anchor_role(&m.edit.as_ref().unwrap().model, Some(shape), role).unwrap();
    m.edit_push(vec![Op::RemoveController { at }], None);
    h.run_steps(3);
    assert!(h.query_by_label(PG_ENV).is_none());
}

/// Note CU7 (Phase B parked): a var-8 controller added after the In game simulation was cached still gets the In
/// game red line and the Edited source Warning, from the cache.
#[test]
fn env_map_scale_added_after_the_simulation_is_cached_still_warns() {
    let (_d, mut m) = pg_session();
    in_game(&mut m);
    m.set_view_source(true);
    let e = m.edit.as_ref().unwrap();
    assert!(e.sim_runs > 0, "the simulation is cached");
    let (shape, _) = lit_shape(e);
    let at = nif::edit::anchor_role(&e.model, Some(shape), Role::Shader).unwrap();
    m.edit_push(
        vec![Op::AddController {
            at,
            class: "BSLightingShaderPropertyFloatController".into(),
            var: Some(8),
            keys: KeyData::Float(KeyGroup {
                key_type: Some(1),
                keys: vec![fkey(0.0, 1.0, None), fkey(1.0, 0.5, None)],
            }),
            flags: 0x48,
            freq: 1.0,
            start: 0.0,
            stop: 1.0,
        }],
        None,
    );
    let e = m.edit.as_ref().unwrap();
    assert!(
        e.pg_warnings.iter().any(|w| w.text == PG_ENV),
        "{:?}",
        e.pg_warnings
    );
    in_game(&mut m);
    let n = notes(&m, shape);
    assert!(n.iter().any(|l| l == PG_ENV), "{n:?}");
}

fn notes_or_none(m: &MeshesModel, b: u32) -> Option<Vec<String>> {
    let rows = m.edit.as_ref().unwrap().in_game.clone()?;
    rows.into_iter().find(|(id, _, _)| id.0 == b).map(|r| r.2)
}

// ---- B-P41: key-drag cost on a many-controller owner NIF (final review minor 5) ----

// ---- Key fixes: stale key errors (Note CM33), Add key after the last key (B-P43), stop follows keys (B-P44) ----

fn ctrl_times(m: &MeshesModel, ctrl: u32) -> (f32, f32) {
    let c = &m.edit.as_ref().unwrap().model.ctrls[&ctrl];
    (c.start, c.stop)
}

/// keyed_pair's ctrl 2 (Emissive multiple: LINEAR (0, 0) (1, 1), start 0, stop 1) in the Controllers tab.
fn keyed_view() -> Harness<'static, MeshesModel> {
    let mut m = direct(scenes::keyed_pair(scenes::Share::No, [1, 2]));
    select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Controllers");
    h
}

/// A scanned session (export folder, real shapes, the clock) on `with_uv_ctrl`, in the Controllers tab of the
/// shape whose chain starts with the U offset controller (LINEAR (0, 0) (1, 1), start 0, stop 1).
fn uv_view() -> (tempfile::TempDir, Harness<'static, MeshesModel>) {
    let (d, mut m, _cmds) = session_on(&with_uv_ctrl(&fixture("TestNifFile_Skinned_SE.nif")));
    let (shape, _) = lit_shape(m.edit.as_ref().unwrap());
    m.select_edit(Selection::Shape(shape));
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Controllers");
    (d, h)
}

fn click_add_key(h: &mut Harness<'static, MeshesModel>) {
    h.get_by_role_and_label(AkRole::Button, "Add key to Emissive multiple")
        .click();
    h.run_steps(3);
}

/// A SetKeys that a later SetKeys replaced is not validated: fixing the times clears the error everywhere.
#[test]
fn stale_key_order_error_clears_when_keys_are_fixed() {
    const ERR: &str = "Key times must not decrease";
    let (_d, mut h) = uv_view();
    edit_number(&mut h, "U offset key 2 time", "-1");
    assert!(h.query_all_by_label(ERR).count() > 0);
    assert_eq!(
        h.state().edit.as_ref().unwrap().export_block().as_deref(),
        Some(ERR)
    );
    edit_number(&mut h, "U offset key 2 time", "2");
    h.run_steps(2);
    assert_eq!(h.query_all_by_label(ERR).count(), 0);
    let e = h.state().edit.as_ref().unwrap();
    assert!(e.op_warnings.is_empty(), "{:?}", e.op_warnings);
    assert_ne!(e.export_block().as_deref(), Some(ERR));
}

/// B-P43: Add key appends at the last time plus the average spacing, with the last key's value.
#[test]
fn add_key_appends_after_the_last_key() {
    let mut h = keyed_view();
    for _ in 0..3 {
        click_add_key(&mut h);
    }
    let g = float_keys(h.state(), 2);
    let got: Vec<(f32, f32)> = g.keys.iter().map(|k| (k.time, k.value)).collect();
    assert_eq!(
        got,
        [(0.0, 0.0), (1.0, 1.0), (2.0, 1.0), (3.0, 1.0), (4.0, 1.0)]
    );
}

/// B-P43/B-P44: past the stop, the same undo step moves the stop to the new key.
#[test]
fn add_key_past_stop_raises_stop_in_one_step() {
    let mut h = keyed_view();
    click_add_key(&mut h);
    assert_eq!(ctrl_times(h.state(), 2), (0.0, 2.0));
    let got = ops(h.state());
    assert_eq!(got.len(), 2, "{got:?}");
    assert!(
        matches!(&got[1], Op::SetField { field: Field::StopTime, value: Value::F32(v), .. } if f32::from_bits(*v) == 2.0),
        "{got:?}"
    );
    assert!(h.state_mut().undo_edit());
    assert!(ops(h.state()).is_empty());
    assert_eq!(ctrl_times(h.state(), 2), (0.0, 1.0));
    assert_eq!(float_keys(h.state(), 2).keys.len(), 2);
}

/// B-P43: the old rule (the cursor, or half-way between the last two keys) gave 1 on (0, 1, 1) and disabled Add
/// key with "A key is already at this time."; now the key lands after the last one.
#[test]
fn add_key_never_duplicates_a_time() {
    let mut h = keyed_view();
    {
        let m = h.state_mut();
        let e = m.edit.as_ref().unwrap();
        let mut g = float_keys(m, 2);
        let base = KeyData::Float(g.clone());
        g.keys.push(g.keys[1].clone());
        let role = Role::Ctrl {
            owner: nif::edit::Owner::Shader,
            class: "BSLightingShaderPropertyFloatController".into(),
            var: Some(11),
            nth: 0,
        };
        let at = nif::edit::anchor_role(&e.model, Some(0), role).unwrap();
        let op = Op::SetKeys {
            at,
            clip: None,
            clip_name: None,
            keys: KeyData::Float(g),
            base,
        };
        m.edit_push(vec![op], None);
    }
    h.run_steps(2);
    assert!(h.query_by_label("A key is already at this time.").is_none());
    click_add_key(&mut h);
    let t: Vec<f32> = float_keys(h.state(), 2)
        .keys
        .iter()
        .map(|k| k.time)
        .collect();
    assert_eq!(t.len(), 4, "{t:?}");
    assert!(t[3] > t[2], "{t:?}");
    assert!(t[..3].iter().all(|&x| x != t[3]), "{t:?}");
}

/// B-P44: dragging the last key moves the stop with it; one undo reverts both.
#[test]
fn stop_follows_the_last_key_on_drag() {
    let mut h = keyed_view();
    // The key 2 handle (t 1, the right end) dragged left: a time drag of several ticks, one merged step.
    let c = h.get_by_label("Emissive multiple key 2").rect().center();
    h.hover_at(c);
    h.run_steps(1);
    h.drag_at(c);
    h.run_steps(1);
    for k in 1..=3 {
        h.hover_at(c - egui::vec2(40.0 * k as f32 / 3.0, 0.0));
        h.run_steps(1);
    }
    h.drop_at(c - egui::vec2(40.0, 0.0));
    h.run_steps(3);
    let last = float_keys(h.state(), 2).keys[1].time;
    assert!(last < 1.0, "{last} {:?}", ops(h.state()));
    assert_eq!(ctrl_times(h.state(), 2), (0.0, last));
    assert_eq!(ops(h.state()).len(), 2, "{:?}", ops(h.state()));
    assert!(h.state_mut().undo_edit());
    assert!(ops(h.state()).is_empty());
    assert_eq!(ctrl_times(h.state(), 2), (0.0, 1.0));
}

/// B-P44: removing the last key brings the stop back to the new last key; a first key moved before the start
/// pulls the start with it.
#[test]
fn remove_last_key_shrinks_stop() {
    let mut h = keyed_view();
    click_add_key(&mut h);
    assert_eq!(ctrl_times(h.state(), 2).1, 2.0);
    h.get_by_label("Remove Emissive multiple key 3").click();
    h.run_steps(3);
    assert_eq!(ctrl_times(h.state(), 2), (0.0, 1.0));
    edit_number(&mut h, "Emissive multiple key 1 time", "-0.5");
    assert_eq!(ctrl_times(h.state(), 2), (-0.5, 1.0));
}

/// B-P44: a clip's keys never move its sequence's start and stop.
#[test]
fn clip_stop_is_not_changed() {
    let plain = with_uv_ctrl(&fixture("TestNifFile_Skinned_SE.nif"));
    let (nif, seq) = with_clip(&plain);
    let mut m = direct(nif);
    m.edit_with(|e| e.set_playing(Playing::Clip(seq)));
    let (shape, _) = lit_shape(m.edit.as_ref().unwrap());
    m.select_edit(Selection::Shape(shape));
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Controllers");
    h.get_by_role_and_label(AkRole::Button, "Add key to Emissive multiple in Glow")
        .click();
    h.run_steps(3);
    let got = ops(h.state());
    assert_eq!(got.len(), 1, "{got:?}");
    assert!(matches!(&got[0], Op::SetKeys { clip: Some(_), .. }));
    let e = h.state().edit.as_ref().unwrap();
    let s = e.model.sequences.iter().find(|s| s.block == seq).unwrap();
    assert_eq!((s.start, s.stop), (0.0, 1.0));
}

/// The owner's "end time stayed at 1": a typed Stop is stored, and the clock's range follows it without a reopen.
#[test]
fn typed_stop_is_stored_and_moves_the_clock_range() {
    let (_d, mut h) = uv_view();
    edit_number(&mut h, "U offset stop", "3");
    let e = h.state().edit.as_ref().unwrap();
    let stops: Vec<f32> = e.model.ctrls.values().map(|c| c.stop).collect();
    assert!(stops.contains(&3.0), "{stops:?}");
    assert_eq!(e.anim_range(), Some((0.0, 3.0)));
    h.state_mut().tick_anim(0.0);
    assert_eq!(h.state().v3.clock.map(|c| c.range), Some((0.0, 3.0)));
}

/// B-P44: the stop op comes and goes during one drag (the last key passes the stop and comes back); the drag stays
/// one undo step, the stop op keeps the stop from before the drag as its base and leaves when it is not needed.
#[test]
fn a_drag_whose_stop_op_comes_and_goes_is_one_step() {
    let mut m = direct(scenes::keyed_pair(scenes::Share::No, [1, 2]));
    let base = KeyData::Float(float_keys(&m, 2));
    let e = m.edit.as_mut().unwrap();
    let role = Role::Ctrl {
        owner: nif::edit::Owner::Shader,
        class: "BSLightingShaderPropertyFloatController".into(),
        var: Some(11),
        nth: 0,
    };
    let at = nif::edit::anchor_role(&e.model, Some(0), role).unwrap();
    let keys = |t: f32| {
        let KeyData::Float(mut g) = base.clone() else {
            unreachable!()
        };
        g.keys[1].time = t;
        Op::SetKeys {
            at: at.clone(),
            clip: None,
            clip_name: None,
            keys: KeyData::Float(g),
            base: base.clone(),
        }
    };
    let stop = |to: f32, from: f32| Op::SetField {
        at: at.clone(),
        field: Field::StopTime,
        value: Value::F32(to.to_bits()),
        base: Value::F32(from.to_bits()),
    };
    let merge = Some(nif::edit::merge_key(&keys(1.0)));
    e.push(vec![keys(1.0)], merge.clone());
    e.push(vec![keys(1.5), stop(1.5, 1.0)], merge.clone());
    // The UI reads the base from the drag's op: here a later tick names the current stop, which push replaces.
    e.push(vec![keys(2.0), stop(2.0, 1.5)], merge.clone());
    let got: Vec<Op> = e.doc.ops().to_vec();
    assert_eq!(got.len(), 2, "{got:?}");
    assert_eq!(got[1], stop(2.0, 1.0), "the base from before the drag");
    assert_eq!(e.model.ctrls[&2].stop, 2.0);
    e.push(vec![keys(1.0)], merge.clone());
    assert_eq!(e.doc.ops().len(), 1, "{:?}", e.doc.ops());
    assert_eq!(e.model.ctrls[&2].stop, 1.0);
    e.flush();
    assert!(e.undo());
    assert!(e.doc.ops().is_empty());
    assert!(!e.history.can_undo());
}

/// keyed_view with ctrl 2's keys replaced by LINEAR keys at `times` (value 1), as one pushed SetKeys.
fn keyed_view_at(times: &[f32]) -> Harness<'static, MeshesModel> {
    let mut h = keyed_view();
    let m = h.state_mut();
    let e = m.edit.as_ref().unwrap();
    let g = float_keys(m, 2);
    let base = KeyData::Float(g.clone());
    let keys = times
        .iter()
        .map(|&time| Key {
            time,
            value: 1.0,
            forward: None,
            backward: None,
            tbc: None,
        })
        .collect();
    let role = Role::Ctrl {
        owner: nif::edit::Owner::Shader,
        class: "BSLightingShaderPropertyFloatController".into(),
        var: Some(11),
        nth: 0,
    };
    let op = Op::SetKeys {
        at: nif::edit::anchor_role(&e.model, Some(0), role).unwrap(),
        clip: None,
        clip_name: None,
        keys: KeyData::Float(KeyGroup {
            key_type: g.key_type,
            keys,
        }),
        base,
    };
    m.edit_push(vec![op], None);
    h.run_steps(2);
    h
}

/// B-P45: at 2^21 a 0.1 step rounds back to the last time; Add key takes the next f32 above it instead.
#[test]
fn add_key_past_a_large_time_still_increases() {
    let mut h = keyed_view_at(&[2097152.0]);
    click_add_key(&mut h);
    click_add_key(&mut h);
    let t: Vec<f32> = float_keys(h.state(), 2)
        .keys
        .iter()
        .map(|k| k.time)
        .collect();
    assert_eq!(t.len(), 3, "{t:?}");
    assert!(t.windows(2).all(|w| w[1] > w[0]), "{t:?}");
    assert!(t.iter().all(|x| x.is_finite()), "{t:?}");
}

/// B-P45: no finite time is above f32::MAX: Add key is off and says why on hover.
#[test]
fn add_key_is_off_when_no_later_time_exists() {
    let mut h = keyed_view_at(&[0.0, f32::MAX]);
    let b = h.get_by_role_and_label(AkRole::Button, "Add key to Emissive multiple");
    assert!(b.accesskit_node().is_disabled());
    b.hover();
    h.run();
    h.get_by_label("No later time is possible for a new key.");
    assert_eq!(float_keys(h.state(), 2).keys.len(), 2);
}

// ---- v2 C5: clips in the Controllers tab ----

/// `animated(Some(1), 2)` with "S" selected: free LSCF var 11 "Emissive multiple" (4) and var 20 "U offset" (5).
fn clip_scene() -> MeshesModel {
    let mut m = direct(scenes::animated(Some(1), 2));
    select(&mut m, "S");
    m
}

/// `scroll_to_me` animates: step until the node has not moved for three frames (at most 60).
fn reveal(h: &mut Harness<'static, MeshesModel>, label: &str) {
    h.get_by_label(label).scroll_to_me();
    let (mut last, mut still) = (h.get_by_label(label).rect(), 0);
    for _ in 0..60 {
        h.run_steps(1);
        let r = h.get_by_label(label).rect();
        still = if r == last { still + 1 } else { 0 };
        if still == 3 {
            break;
        }
        last = r;
    }
}

/// Note CU2: every change of the edited bytes gives a new revision. `rev` keys the pending cache (Note GD23) and the
/// stale-block checks (Note GP8), so a state that reused an earlier `rev` would show that state's answers.
#[test]
fn every_apply_gives_a_new_revision() {
    let mut m = clip_scene();
    let mut seen = vec![m.edit.as_ref().unwrap().rev];
    let mut note = |m: &MeshesModel| {
        let r = m.edit.as_ref().unwrap().rev;
        assert!(!seen.contains(&r), "{r} seen before: {seen:?}");
        seen.push(r);
    };
    let s = shape_named(m.edit.as_ref().unwrap(), "S");
    add_colour_ctrl(&mut m, s, [0.0; 3], [1.0; 3]);
    note(&m);
    m.undo_edit();
    note(&m);
    m.redo_edit();
    note(&m);
    m.edit_with(|e| e.revert(0));
    note(&m);
    m.undo_edit();
    note(&m);
}

// ---- Note CS: SLSF1 bits that need a shader type (owner) ----

const SKIN_TINT: &str =
    "Skin tint flag needs the Skin Tint shader type: the game crashes otherwise.";
const FACEGEN_DETAIL: &str =
    "FaceGen detail flag needs the Face Tint or Skin Tint shader type: the mesh renders black otherwise.";

/// Shape "S" (0) on a BSLSP (1) of `shader_type` with SLSF1 = Zbuffer_Test | `bits`.
fn typed(shader_type: u32, bits: u32) -> Vec<u8> {
    let trailer: &[f32] = if shader_type == 5 { &[1.0; 3] } else { &[] };
    let mut b = scenes::lsp_shape(shader_type, trailer);
    let h = nif::header::Header::parse(&b).unwrap();
    let (rel, _) = nif::fields::locate(&h, &b, 1, Field::Slsf1).unwrap();
    let at = h.offsets()[1] + rel;
    b[at..at + 4].copy_from_slice(&((1u32 << 31) | bits).to_le_bytes());
    b
}

fn slsf1(m: &MeshesModel) -> u32 {
    m.edit.as_ref().unwrap().model.lighting[&1].lighting.slsf1
}

#[test]
fn a_type_bound_flag_shows_its_error_and_clears_in_one_undo_step() {
    let mut m = direct(typed(0, 1 << 21));
    select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(2);
    // Meshes UI T6: a flag list with a warning starts open (its Fix in sight).
    h.get_by_label(SKIN_TINT);
    h.get(
        egui_kittest::kittest::By::new()
            .role(AkRole::Button)
            .label_contains("Clear the flag: "),
    )
    .scroll_to_me();
    h.run();
    h.get(
        egui_kittest::kittest::By::new()
            .role(AkRole::Button)
            .label_contains("Clear the flag: "),
    )
    .click();
    h.run_steps(2);
    assert!(h.query_by_label(SKIN_TINT).is_none());
    let m = h.state_mut();
    assert_eq!(ops(m).len(), 1);
    assert_eq!(slsf1(m), 1 << 31);
    assert!(m.undo_edit());
    assert!(ops(m).is_empty(), "the fix is one undo step");
    // Face Tint without the detail flag offers "Set the flag".
    let mut m = direct(typed(4, 0));
    select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(2);
    h.get(
        egui_kittest::kittest::By::new()
            .role(AkRole::Button)
            .label_contains("Set the flag: "),
    )
    .scroll_to_me();
    h.run();
    h.get(
        egui_kittest::kittest::By::new()
            .role(AkRole::Button)
            .label_contains("Set the flag: "),
    )
    .click();
    h.run_steps(2);
    assert_eq!(slsf1(h.state()), (1 << 31) | (1 << 10));
}

#[test]
fn a_shader_type_change_clears_the_flag_in_the_same_undo_step() {
    let mut m = direct(typed(5, 1 << 21));
    select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(2);
    h.get_by_role_and_label(AkRole::ComboBox, "Shader type")
        .click();
    h.run_steps(3);
    h.get_by_label("Default").click();
    h.run_steps(3);
    h.get_by_label("Skin tint flag cleared: it needs the Skin Tint shader type.");
    assert!(h.query_by_label(SKIN_TINT).is_none());
    let m = h.state_mut();
    let got = ops(m);
    assert_eq!(got.len(), 2, "{got:?}");
    assert_eq!(slsf1(m), 1 << 31);
    assert_eq!(
        m.edit.as_ref().unwrap().model.lighting[&1]
            .lighting
            .shader_type,
        0
    );
    assert!(m.undo_edit());
    assert!(ops(m).is_empty(), "type and flag are one undo step");
    assert!(m.edit.as_ref().unwrap().notice.is_none());
}

#[test]
fn a_type_change_off_11_clears_the_mlp_flag_in_the_same_undo_step() {
    // Note DJ: type 11 with SLSF2 24, changed to Environment Map
    let mut b = scenes::lsp_shape(11, &[5.0, 1.0, 1.0, 1.0, 1.0]);
    let hd = nif::header::Header::parse(&b).unwrap();
    let (rel, _) = nif::fields::locate(&hd, &b, 1, Field::Slsf2).unwrap();
    let at = hd.offsets()[1] + rel;
    b[at..at + 4].copy_from_slice(&((1u32 << 24) | 1).to_le_bytes());
    let mut m = direct(b);
    select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(2);
    h.get_by_role_and_label(AkRole::ComboBox, "Shader type")
        .click();
    h.run_steps(3);
    h.get_by_label("Environment Map").click();
    h.run_steps(3);
    h.get_by_label("Multi-layer parallax flag cleared: it needs shader type 11.");
    let m = h.state_mut();
    let got = ops(m);
    assert_eq!(got.len(), 2, "{got:?}");
    let l = &m.edit.as_ref().unwrap().model.lighting[&1].lighting;
    assert_eq!((l.shader_type, l.slsf2), (1, 1));
    assert!(m.undo_edit());
    assert!(ops(m).is_empty(), "type and flag are one undo step");
}
#[test]
fn flags_that_need_another_type_are_greyed_with_the_reason() {
    let mut m = direct(typed(0, 0));
    select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(2);
    open_flags(&mut h); // Meshes UI T6: the flag lists start closed
    let skin = h.get_by_label("FaceGen_RGB_Tint");
    assert!(skin.accesskit_node().is_disabled());
    // Note DK
    assert!(h
        .get_by_label("Hair_Soft_Lighting")
        .accesskit_node()
        .is_disabled());
    assert!(h
        .get_by_label("Facegen_Detail_Map")
        .accesskit_node()
        .is_disabled());
    h.get_by_label("FaceGen_RGB_Tint").scroll_to_me();
    h.run();
    h.get_by_label("FaceGen_RGB_Tint").hover();
    h.run_steps(30);
    h.get_by_label("Needs the Skin Tint shader type.");
    // On its own type the flag is editable.
    let mut m = direct(typed(5, 1 << 21));
    select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(2);
    open_flags(&mut h);
    assert!(!h
        .get_by_label("FaceGen_RGB_Tint")
        .accesskit_node()
        .is_disabled());
    // CS3: Skin Tint may carry the detail flag too.
    assert!(!h
        .get_by_label("Facegen_Detail_Map")
        .accesskit_node()
        .is_disabled());
}

#[test]
fn a_type_bound_flag_blocks_export_even_in_an_untouched_source() {
    let (_d, mut e) = exportable(typed(0, 1 << 10));
    let at = nif::edit::anchor_role(&e.model, Some(0), Role::Shader).unwrap();
    e.push(
        vec![Op::SetField {
            at,
            field: Field::Glossiness,
            value: Value::F32(50f32.to_bits()),
            base: Value::F32(80f32.to_bits()),
        }],
        None,
    );
    assert_eq!(e.export_block().as_deref(), Some(FACEGEN_DETAIL));
}

// ---- Meshes UI T5b: heel height (Note CZ19) ----

/// The heel scene on the Extra data tab with shape `name` selected and the heel entry picked.
fn heel_view(name: &str, sibling: Option<&str>) -> Harness<'static, MeshesModel> {
    let mut m = direct(scenes::heel_scene());
    select(&mut m, name);
    m.edit.as_mut().unwrap().weight_sibling = sibling.map(String::from);
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Extra data");
    pick_known(&mut h, false, "Heel height (RaceMenu)");
    h
}

/// The extra-data ops of the last push, as (kind or "edit", name, value).
fn heel_ops(h: &Harness<'static, MeshesModel>) -> Vec<Op> {
    ops(h.state())
}

/// A shape with an SDTA and no HH_OFFSET: Set adds HH_OFFSET and moves the SDTA's NPC Z with it (one undo step); an
/// earlier shape (A) already has heel data, so the row says it wins.
#[test]
fn heel_height_adds_hh_offset_and_moves_the_sdta() {
    let mut h = heel_view("B", None);
    h.get_by_label("An earlier shape's heel height wins.");
    let v = h.get_by_role_and_label(AkRole::SpinButton, "Heel height (shape)");
    assert_eq!(
        v.accesskit_node().numeric_value(),
        Some(5.0),
        "starts at the SDTA's Z"
    );
    edit_number(&mut h, "Heel height (shape)", "8.5");
    h.get_by_role_and_label(AkRole::Button, "Set heel height")
        .click();
    h.run_steps(4);
    let got = heel_ops(&h);
    assert_eq!(got.len(), 2, "{got:?}");
    assert!(
        matches!(&got[0], Op::AddExtra { kind, name, value: ExtraValue::Float(b), .. }
            if kind == "NiFloatExtraData" && name == "HH_OFFSET" && f32::from_bits(*b) == 8.5),
        "{got:?}"
    );
    let Op::EditExtra {
        value: ExtraValue::Str(s),
        ..
    } = &got[1]
    else {
        panic!("{got:?}")
    };
    assert_eq!(sdta_npc_z(s), Some(8.5));
    // Final review Minor 3: both ops apply, and the patched bytes carry HH_OFFSET = 8.5 and the SDTA's 8.5 on B.
    let e = h.state().edit.as_ref().unwrap();
    assert!(
        e.doc.ops.iter().all(|d| d.status == OpStatus::Applied),
        "{:?}",
        e.outcomes
    );
    let pm = nif::model::read_model(&e.patched).unwrap();
    let b = pm
        .shapes
        .iter()
        .find(|s| s.name.as_deref() == Some("B"))
        .unwrap();
    let extras = pm.extras_of(&b.net);
    let hh = extras
        .iter()
        .find(|x| x.name.as_deref() == Some(&b"HH_OFFSET"[..]))
        .unwrap();
    assert_eq!(hh.value, Some(ExtraValue::Float(8.5f32.to_bits())));
    let sdta = extras
        .iter()
        .find(|x| x.name.as_deref() == Some(&b"SDTA"[..]))
        .unwrap();
    let Some(ExtraValue::Str(written)) = &sdta.value else {
        panic!("{sdta:?}")
    };
    assert_eq!(sdta_npc_z(written), Some(8.5));
    let m = h.state_mut();
    assert!(m.undo_edit());
    assert!(ops(m).is_empty(), "one undo step");
}

/// A shape with HH_OFFSET: Set edits it in place (a second one is ignored by the game) and adds no SDTA.
#[test]
fn heel_height_edits_an_existing_hh_offset() {
    let mut h = heel_view("A", None);
    assert!(h
        .query_by_label("An earlier shape's heel height wins.")
        .is_none());
    edit_number(&mut h, "Heel height (shape)", "9");
    h.get_by_role_and_label(AkRole::Button, "Set heel height")
        .click();
    h.run_steps(4);
    let got = heel_ops(&h);
    assert!(
        matches!(&got[..], [Op::EditExtra { value: ExtraValue::Float(b), base: ExtraValue::Float(was), .. }]
            if f32::from_bits(*b) == 9.0 && f32::from_bits(*was) == 7.0),
        "{got:?}"
    );
}

/// A shape with no heel data: HH_OFFSET only, or HH_OFFSET and a new SDTA when the box is ticked; the weight-file
/// sibling is named.
#[test]
fn heel_height_on_a_bare_shape_adds_hh_and_an_optional_sdta() {
    let mut h = heel_view("C", Some("meshes/x_1.nif"));
    h.get_by_label("An earlier shape's heel height wins.");
    h.get_by_label("Apply the same value to the other weight file (meshes\\x_1.nif).");
    h.get_by_label("Also write SDTA (OStim and others read it)")
        .click();
    h.run_steps(2);
    edit_number(&mut h, "Heel height (shape)", "6");
    h.get_by_role_and_label(AkRole::Button, "Set heel height")
        .click();
    h.run_steps(4);
    let got = heel_ops(&h);
    assert_eq!(got.len(), 2, "{got:?}");
    let Op::AddExtra {
        kind,
        name,
        value: ExtraValue::Str(s),
        ..
    } = &got[1]
    else {
        panic!("{got:?}")
    };
    assert_eq!(
        (kind.as_str(), name.as_str()),
        ("NiStringExtraData", "SDTA")
    );
    assert_eq!(sdta_npc_z(s), Some(6.0));
}

/// On the root the entry is off, and the root's own SDTA row says the game ignores it there.
#[test]
fn heel_height_is_off_on_the_root_and_root_heel_data_warns() {
    let mut m = direct(scenes::heel_scene());
    select(&mut m, "C");
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Extra data");
    assert!(known_entry_disabled(&mut h, true, "Heel height (RaceMenu)"));
    h.get_by_label("The game ignores heel height here. Put it on a shape.");
}

/// Calculate fills the field from the lowest bind-pose vertex (`heel_z_of`); on a shape with no geometry it says so.
#[test]
fn heel_height_calculate_fills_the_field() {
    let bytes = fixture("TestNifFile_Skinned_SE.nif");
    let mut m = direct(bytes.clone());
    let e = m.edit.as_ref().unwrap();
    let (shape, _) = lit_shape(e);
    let want = heel_z_of(&bytes, shape).expect("a measurable shape");
    m.select_edit(Selection::Shape(shape));
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Extra data");
    pick_known(&mut h, false, "Heel height (RaceMenu)");
    h.get_by_role_and_label(AkRole::Button, "Calculate").click();
    h.run_steps(3);
    let v = h.get_by_role_and_label(AkRole::SpinButton, "Heel height (shape)");
    assert_eq!(v.accesskit_node().numeric_value(), Some(want as f64));
    let mut h = heel_view("C", None);
    h.get_by_role_and_label(AkRole::Button, "Calculate").click();
    h.run_steps(3);
    h.get_by_label("This shape has no geometry to measure: type the height.");
}

/// The weight-file sibling of a data path: `_0` ↔ `_1` before `.nif`, any case; None for other names.
#[test]
fn weight_sibling_names_the_other_weight_file() {
    assert_eq!(
        weight_sibling_of("meshes/armor/boots_0.nif").as_deref(),
        Some("meshes/armor/boots_1.nif")
    );
    assert_eq!(
        weight_sibling_of("meshes/armor/Boots_1.NIF").as_deref(),
        Some("meshes/armor/Boots_0.NIF")
    );
    assert_eq!(weight_sibling_of("meshes/armor/boots.nif"), None);
    assert_eq!(weight_sibling_of("meshes/armor/boots_10.nif"), None);
}

// ---- T5b review fix round 1 ----

fn heel_field(h: &Harness<'static, MeshesModel>) -> Option<f64> {
    h.get_by_role_and_label(AkRole::SpinButton, "Heel height (shape)")
        .accesskit_node()
        .numeric_value()
}

/// Important 1: after Set and an undo the field shows the restored value, and a typed value or a heel pick never
/// carries to the same block of another session.
#[test]
fn heel_field_follows_undo_and_stays_in_its_session() {
    let mut h = heel_view("A", None);
    edit_number(&mut h, "Heel height (shape)", "9");
    h.get_by_role_and_label(AkRole::Button, "Set heel height")
        .click();
    h.run_steps(4);
    assert_eq!(heel_field(&h), Some(9.0));
    assert!(h.state_mut().undo_edit());
    h.run_steps(4);
    assert_eq!(heel_field(&h), Some(7.0), "the restored HH_OFFSET");
    edit_number(&mut h, "Heel height (shape)", "4");
    // Another session on a file with the same block numbers: no heel pick, no typed 4.
    let m = h.state_mut();
    m.edit = Some(
        NifEdit::open(
            scenes::heel_scene(),
            copy("A", 1, false),
            "meshes/y.nif",
            None,
            None,
            None,
        )
        .unwrap(),
    );
    let a = shape_named(m.edit.as_ref().unwrap(), "A");
    m.select_edit(Selection::Shape(a));
    h.run_steps(4);
    assert_eq!(
        h.get_by_role_and_label(AkRole::ComboBox, "Known extra data (shape)")
            .accesskit_node()
            .value()
            .as_deref(),
        Some("Custom…")
    );
    pick_known(&mut h, false, "Heel height (RaceMenu)");
    assert_eq!(heel_field(&h), Some(7.0));
}

/// Important 2: HH_OFFSET and the SDTA's NPC Z get the same number (2.25, not 2.25 and 2.3).
#[test]
fn heel_hh_and_sdta_get_the_same_value() {
    let mut h = heel_view("B", None);
    edit_number(&mut h, "Heel height (shape)", "2.25");
    h.get_by_role_and_label(AkRole::Button, "Set heel height")
        .click();
    h.run_steps(4);
    let got = heel_ops(&h);
    let (
        Op::AddExtra {
            value: ExtraValue::Float(b),
            ..
        },
        Op::EditExtra {
            value: ExtraValue::Str(s),
            ..
        },
    ) = (&got[0], &got[1])
    else {
        panic!("{got:?}")
    };
    assert_eq!(f32::from_bits(*b), 2.25);
    assert_eq!(sdta_npc_z(s), Some(2.25));
}

/// Minor 5: an SDTA whose NPC Z already equals the height is left alone (no reformatting edit).
#[test]
fn heel_leaves_an_sdta_that_already_has_the_height() {
    let mut h = heel_view("B", None);
    assert_eq!(heel_field(&h), Some(5.0));
    h.get_by_role_and_label(AkRole::Button, "Set heel height")
        .click();
    h.run_steps(4);
    let got = heel_ops(&h);
    assert!(matches!(&got[..], [Op::AddExtra { .. }]), "{got:?}");
}

/// Minors 1 and 6: the "no geometry" note stays in its session; a negative height says it lowers the actor.
#[test]
fn heel_notes_for_no_geometry_and_a_negative_height() {
    let mut h = heel_view("C", None);
    h.get_by_role_and_label(AkRole::Button, "Calculate").click();
    h.run_steps(3);
    h.get_by_label("This shape has no geometry to measure: type the height.");
    edit_number(&mut h, "Heel height (shape)", "-1");
    h.get_by_label("A negative height lowers the actor.");
    let m = h.state_mut();
    m.edit = Some(
        NifEdit::open(
            scenes::heel_scene(),
            copy("A", 1, false),
            "meshes/y.nif",
            None,
            None,
            None,
        )
        .unwrap(),
    );
    let c = shape_named(m.edit.as_ref().unwrap(), "C");
    m.select_edit(Selection::Shape(c));
    h.run_steps(4);
    pick_known(&mut h, false, "Heel height (RaceMenu)");
    assert!(h
        .query_by_label("This shape has no geometry to measure: type the height.")
        .is_none());
}

/// Minor 3: an HH_OFFSET the editor cannot set (another type) blocks Set with a note instead of adding a second one.
#[test]
fn heel_refuses_an_hh_offset_of_another_type() {
    let mut m = direct(scenes::heel_scene());
    let c = select(&mut m, "C");
    let at = nif::edit::anchor_shape(&m.edit.as_ref().unwrap().model, c);
    m.edit_push(
        vec![Op::AddExtra {
            at,
            kind: "NiStringExtraData".into(),
            name: "HH_OFFSET".into(),
            value: ExtraValue::Str("7".into()),
        }],
        None,
    );
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Extra data");
    pick_known(&mut h, false, "Heel height (RaceMenu)");
    h.get_by_label(HEEL_HH_UNSET_TEXT);
    assert!(h
        .get_by_role_and_label(AkRole::Button, "Set heel height")
        .accesskit_node()
        .is_disabled());
}

/// The note for an HH_OFFSET that is not a readable float.
const HEEL_HH_UNSET_TEXT: &str =
    "This shape's HH_OFFSET is not a number the editor can set: remove that row first.";

/// T5b re-review gap: the heel row's "earlier shape" note counts only shapes the root reaches. Worn's own HH_OFFSET
/// is removed first; the only earlier heel data is then on the unlinked "Orphan" shape, so no note.
#[test]
fn heel_row_ignores_an_earlier_orphan_shape() {
    let mut m = direct(scenes::heel_orphan_scene());
    let worn = select(&mut m, "Worn");
    let at = Anchor {
        block: 4,
        kind: "NiFloatExtraData".into(),
        shape: Some("Worn".into()),
        role: Role::Extra {
            on_root: false,
            name: "HH_OFFSET".into(),
            kind: "NiFloatExtraData".into(),
            nth: 0,
        },
    };
    m.edit_push(vec![Op::RemoveExtra { at }], None);
    let e = m.edit.as_ref().unwrap();
    assert!(
        e.model
            .extras_of(&e.model.shape(worn).unwrap().net)
            .is_empty(),
        "{:?}",
        e.outcomes
    );
    let mut h = view(m);
    h.run_steps(2);
    tab(&mut h, "Extra data");
    pick_known(&mut h, false, "Heel height (RaceMenu)");
    assert!(h
        .query_by_label("An earlier shape's heel height wins.")
        .is_none());
}

// ---- Meshes UI T6: panel orientation (Note CZ21) ----

/// The panel names what the tabs act on, under the tab strip: the selected shape and its type, or the root.
#[test]
fn the_panel_says_which_shape_it_edits() {
    let mut m = direct(ctrl_scene(0x82));
    select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(2);
    let line = h.get_by_label("Editing: S").rect();
    let tabs = h.get_by_label("Extra data tab").rect();
    assert!(
        line.min.y >= tabs.max.y,
        "under the tab strip: {line:?} {tabs:?}"
    );
    h.state_mut().select_edit(Selection::Root);
    h.run_steps(2);
    h.get_by_label("Editing: Root");
}

/// The Shader tab puts the texture slots first, and the two shader-flag lists start closed.
#[test]
fn shader_tab_shows_slots_first_and_flags_closed() {
    let mut m = direct(scenes::shared_alpha_and_set());
    select(&mut m, "A");
    let mut h = view(m);
    h.run_steps(2);
    let slot = h
        .get_by_role_and_label(AkRole::TextInput, "Diffuse path (slot 1)")
        .rect();
    let ty = h
        .get_by_role_and_label(AkRole::ComboBox, "Shader type")
        .rect();
    assert!(slot.max.y <= ty.min.y, "slots first: {slot:?} {ty:?}");
    assert!(
        h.query_by_label("Specular").is_none(),
        "the first flag list starts closed"
    );
    h.get_by_label_contains("Shader flags 1 (0x").click();
    h.run_steps(3);
    h.get_by_label("Specular");
}

/// Meshes UI T6: opens the Shader tab's two shader-flag lists (closed by default). T6 review Minor 4: a list
/// already open (its first flag is drawn) is left open.
fn open_flags(h: &mut Harness<'static, MeshesModel>) {
    use nif::material::{SLSF1_NAMES, SLSF2_NAMES};
    for (flags, first) in [
        ("Shader flags 1 (0x", SLSF1_NAMES[0]),
        ("Shader flags 2 (0x", SLSF2_NAMES[0]),
    ] {
        if h.query_by_role_and_label(AkRole::CheckBox, first).is_some() {
            continue;
        }
        h.get_by_label_contains(flags).scroll_to_me();
        h.run_steps(12);
        h.get_by_label_contains(flags).click();
        h.run_steps(3);
    }
}

// ---- Meshes UI T7: rule fixes (Note CZ22) ----

/// The app in the Meshes view at `w` × `hgt` with x.nif's edit open on its first lit shape; `gpu` renders with wgpu.
fn app_edit(w: f32, hgt: f32, gpu: bool) -> (tempfile::TempDir, AppH) {
    use sdc_gui::app::SdcApp;
    app_data();
    let d = files(false);
    let model = sdc_gui::model::AppModel::new(
        sdc_gui::settings::GuiSettings::default(),
        None,
        support::session(),
    );
    let mut model = model;
    // Note GD5: the app hands the chosen export folder to the meshes each frame
    model.set_export_folder(d.path().join("mods").join(EDIT_MOD));
    let mut app = SdcApp::new(model);
    app.meshes.procs = sdc_gui::model::meshes::no_procs;
    app.meshes.store_path = Some(d.path().join("scan.db"));
    scan_and_select(&mut app.meshes, d.path());
    app.meshes.open_edit();
    let (shape, _) = lit_shape(app.meshes.edit.as_ref().unwrap());
    app.meshes.select_edit(Selection::Shape(shape));
    let mut frame = eframe::Frame::_new_kittest();
    let b = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(w, hgt));
    let b = if gpu { b.wgpu() } else { b };
    let mut h = b.build_ui_state(
        move |ui, app: &mut SdcApp| eframe::App::ui(app, ui, &mut frame),
        app,
    );
    sdc_gui::ui::header::set_meshes_view(&h.ctx, true);
    h.run_steps(6);
    (d, h)
}

fn panel_rect(h: &AppH, id: &str) -> egui::Rect {
    support::panel(&h.ctx, id)
}

/// T7: at 1100 px with an edit open the left column gives way, so the 3D view keeps its 240 px minimum.
#[test]
fn the_3d_view_keeps_its_minimum_at_1100_with_an_edit_open() {
    let (_d, h) = app_edit(1100.0, 800.0, false);
    let (tree, edit) = (panel_rect(&h, "meshes-tree"), panel_rect(&h, "meshes-edit"));
    let frame3d = h
        .get_by_label(sdc_gui::model::viewport3d::FRAME_LABEL)
        .rect();
    assert!(
        frame3d.min.x >= tree.max.x && frame3d.max.x <= edit.min.x,
        "{tree:?} {frame3d:?} {edit:?}"
    );
    assert!(
        frame3d.width() >= 240.0 - 1.0,
        "3D view {frame3d:?} at 1100 px"
    );
    // T7 review Minor 1 (controller ruling): the column defaults to ~420 px and is capped near 40% of the width, so
    // the 3D view sits above its minimum at 1100 and gets the slack at 1280.
    assert!(frame3d.width() > 260.0, "3D view {frame3d:?} at 1100 px");
    assert!(tree.width() <= 0.4 * 1100.0 + 1.0, "tree {tree:?}");
    let (_d, h) = app_edit(1280.0, 800.0, false);
    let frame3d = h
        .get_by_label(sdc_gui::model::viewport3d::FRAME_LABEL)
        .rect();
    assert!(frame3d.width() > 380.0, "3D view {frame3d:?} at 1280 px");
}

/// T5 open item, checked in T7: the Known combo with the longest entry picked stays inside the edit panel at 1100 px.
#[test]
fn the_known_combo_fits_the_edit_panel_at_1100() {
    let (_d, mut h) = app_edit(1100.0, 800.0, false);
    h.get_by_label("Extra data tab").click();
    h.run_steps(4);
    h.get_by_role_and_label(AkRole::ComboBox, "Known extra data")
        .scroll_to_me();
    h.run_steps(12);
    h.get_by_role_and_label(AkRole::ComboBox, "Known extra data")
        .click();
    h.run_steps(4);
    h.get_by_role_and_label(AkRole::Button, "HDT Skinned Mesh Physics Object")
        .click();
    h.run_steps(4);
    let edit = panel_rect(&h, "meshes-edit");
    let combo = h
        .get_by_role_and_label(AkRole::ComboBox, "Known extra data")
        .rect();
    assert!(
        combo.max.x <= edit.max.x + 0.5,
        "combo {combo:?} past the panel {edit:?}"
    );
}

/// T7: the edit tabs are named as tabs, distinct from the fields and headings with the same words.
#[test]
fn the_edit_tabs_are_named_as_tabs() {
    let mut h = view(direct(ctrl_scene(0x82)));
    h.run_steps(2);
    for t in [
        "Shader tab",
        "Alpha tab",
        "Controllers tab",
        "Extra data tab",
    ] {
        h.get_by_label(t);
    }
}

/// T7: a repeated fix button is named after its warning; Take source / Keep mine after their edit.
#[test]
fn repeated_fix_and_conflict_buttons_have_unique_names() {
    let mut m = direct(typed(0, 1 << 21));
    select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(2);
    h.get_by_role_and_label(
        AkRole::Button,
        &format!("Clear the flag: {SKIN_TINT} (Shader flags 1)"),
    );
    // T7 review Minor 5: two fix buttons on one tab have two different names.
    let mut m = direct(typed(0, (1 << 21) | (1 << 10)));
    select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(2);
    let names: Vec<String> = h
        .query_all_by_role(AkRole::Button)
        .filter_map(|n| n.accesskit_node().label())
        .filter(|l| l.starts_with("Clear the flag: "))
        .collect();
    assert_eq!(names.len(), 2, "{names:?}");
    assert_ne!(names[0], names[1]);
    // A slot path outside Data: its "Not relative to Data" warning carries a fix.
    let mut m = direct(scenes::shared_alpha_and_set());
    let a = select(&mut m, "A");
    let e = m.edit.as_ref().unwrap();
    let set = e.model.lighting[&e.model.shape(a).unwrap().shader.unwrap()]
        .texture_set
        .unwrap();
    let base = String::from_utf8_lossy(&e.model.texsets[&set].raw.slots[0]).into_owned();
    let at = nif::edit::anchor_shape(&e.model, a);
    m.edit_push(
        vec![Op::SetTexturePath {
            at,
            slot: 0,
            path: r"C:\Game\Data\textures\a.dds".into(),
            base,
        }],
        None,
    );
    let mut h = view(m);
    h.run_steps(2);
    h.get_by_role_and_label(AkRole::Button, "Fix slot 1: Not relative to Data");
    // A conflict: the source's own glossiness changes on disk under the edit.
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    let e = m.edit.as_ref().unwrap();
    let (shape, shader) = lit_shape(e);
    let cur = nif::model::read_model(&e.source).unwrap().lighting[&shader]
        .lighting
        .params
        .glossiness;
    let theirs = nif::edit::apply(
        &e.source,
        &[Op::SetField {
            at: nif::edit::anchor_role(&e.model, Some(shape), Role::Shader).unwrap(),
            field: Field::Glossiness,
            value: Value::F32(7.0f32.to_bits()),
            base: Value::F32(cur.to_bits()),
        }],
    )
    .unwrap()
    .bytes;
    std::fs::write(d.path().join("mods/A/meshes/x.nif"), &theirs).unwrap();
    let _ = export(&mut m);
    assert!(m.edit.as_ref().unwrap().doc.has_conflicts());
    let mut h = view(m);
    h.run_steps(3);
    h.get(
        egui_kittest::kittest::By::new()
            .role(AkRole::Button)
            .label_contains("Take source 1: "),
    );
    h.get(
        egui_kittest::kittest::By::new()
            .role(AkRole::Button)
            .label_contains("Keep mine 1: "),
    );
}

/// T6 review Minor 3: the Editing line for an unnamed shape and for a selection that is not in the file.
#[test]
fn the_editing_line_names_unnamed_and_missing_shapes() {
    let mut m = direct(scenes::managed_clip_and_bare_shape());
    m.select_edit(Selection::Shape(12));
    let mut h = view(m);
    h.run_steps(2);
    h.get_by_label("Editing: (unnamed)");
    h.state_mut().select_edit(Selection::Shape(999));
    h.run_steps(2);
    h.get_by_label("Editing: block 999 (not in this NIF)");
}

/// T7 screenshots (GPU, run with the render_ gate): the Meshes page with an edit open at 1100, 1280 and 1600 px, saved
/// to %TEMP% (never the repo) for the report.
#[test]
fn render_meshes_page_with_an_edit_at_three_widths() {
    for w in [1100.0f32, 1280.0, 1600.0] {
        let (_d, mut h) = app_edit(w, 900.0, true);
        h.run_steps(6);
        let img = h.render().expect("render");
        let out = std::env::temp_dir().join(format!("tessalith-meshes-edit-{w}.png"));
        img.save(&out).expect("save png");
        println!("screenshot: {}", out.display());
    }
}

// ---- Meshes UI T8: the Controllers line outside Edit (Note CZ25) ----

/// Steps the view until `label` shows (the controller summary is read off the UI thread), at most ~5 s.
fn wait_for_label(h: &mut Harness<'static, MeshesModel>, label: &str) -> bool {
    for _ in 0..100 {
        h.run_steps(1);
        if h.query_by_label(label).is_some() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

/// T8: a selected NIF's cards show their Controllers line without Edit, read from the file once.
#[test]
fn cards_show_controllers_without_edit() {
    let bytes = ctrl_scene(0x82);
    let d = files_pair(&bytes, &bytes);
    let mut m = MeshesModel::new(Some(d.path().join("scan.db")), MeshesSettings::default());
    m.procs = sdc_gui::model::meshes::no_procs;
    scan_and_select(&mut m, d.path());
    m.shapes_col = true;
    let mut h = view(m);
    assert!(
        wait_for_label(&mut h, "2: Emissive multiple, U offset"),
        "no Controllers line outside Edit"
    );
    assert!(h.state().edit.is_none());
}

/// T8: a NIF whose file cannot be read any more (gone since the scan) still shows its cards, with no Controllers line
/// and no error.
#[test]
fn cards_stay_when_the_controller_read_fails() {
    let bytes = ctrl_scene(0x82);
    let d = files_pair(&bytes, &bytes);
    let mut m = MeshesModel::new(Some(d.path().join("scan.db")), MeshesSettings::default());
    m.procs = sdc_gui::model::meshes::no_procs;
    // The scan reads the good bytes; both copies then turn into junk before the NIF is selected (and read again).
    m.activate_with(None);
    m.use_instance(d.path().to_path_buf(), "Default".into());
    m.request_scan();
    assert!(m.take_request());
    m.start();
    let t = Instant::now();
    m.poll();
    while m.scanning() {
        assert!(t.elapsed() < Duration::from_secs(60), "scan did not end");
        std::thread::sleep(Duration::from_millis(10));
        m.poll();
    }
    for rel in ["mods/A/meshes/x.nif", "mods/PGP/meshes/x.nif"] {
        std::fs::write(d.path().join(rel), b"not a nif").unwrap();
    }
    let a = m.mods.iter().find(|r| r.name == "A").unwrap().id;
    m.select_mod(a);
    let c = m.nifs[0].copy_id;
    m.select_nif(c);
    assert!(!m.shapes.is_empty(), "the cards come from the scan");
    m.shapes_col = true;
    let mut h = view(m);
    for _ in 0..20 {
        h.run_steps(1);
        std::thread::sleep(Duration::from_millis(25));
    }
    h.get_by_label_contains("Select shape");
    assert!(h.query_by_label("Controllers:").is_none());
    assert_eq!(h.state().error, None);
}

// ---- Meshes UI final-review fixes (Note CZ24) ----

/// Final review Minor 1: the flag lists' open state belongs to one session, so a second NIF whose shader has the
/// same block number still opens a list with a warning.
#[test]
fn a_flag_warning_opens_its_list_in_the_next_session() {
    let mut m = direct(typed(0, 0));
    select(&mut m, "S");
    let mut h = view(m);
    h.run_steps(2);
    assert!(h.query_by_label("Specular").is_none(), "closed, no warning");
    let m = h.state_mut();
    m.edit = Some(
        NifEdit::open(
            typed(0, 1 << 21),
            copy("A", 1, false),
            "meshes/y.nif",
            None,
            None,
            None,
        )
        .unwrap(),
    );
    let s = shape_named(m.edit.as_ref().unwrap(), "S");
    m.select_edit(Selection::Shape(s));
    h.run_steps(3);
    h.get(
        egui_kittest::kittest::By::new()
            .role(AkRole::Button)
            .label_contains("Clear the flag: "),
    );
}

/// Final review Minor 2: a Root pick inside Edit is where the next Edit opens.
#[test]
fn a_root_pick_inside_edit_carries_to_the_next_edit() {
    let (_d, mut m) = session(false);
    m.open_edit();
    let shape = lit_shape(m.edit.as_ref().unwrap()).0;
    m.select_edit(Selection::Shape(shape));
    m.shapes_col = true;
    let mut h = view(m);
    h.run_steps(4);
    let name = h
        .state()
        .edit
        .as_ref()
        .unwrap()
        .model
        .shape(shape)
        .unwrap()
        .name
        .clone();
    let tail = format!(" {}", name.unwrap());
    h.query_all_by_role(AkRole::Button)
        .find(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Select shape") && l.ends_with(&tail))
        })
        .unwrap()
        .click();
    h.run_steps(3);
    h.get_by_label("Root").click();
    h.run_steps(3);
    let m = h.state_mut();
    m.close_edit();
    m.open_edit();
    assert_eq!(m.edit.as_ref().unwrap().selection, Selection::Root);
}

/// Final review Minor 8: closing Edit puts the column back where it was (Mods and meshes).
#[test]
fn closing_edit_restores_the_mods_and_meshes_column() {
    let (_d, mut m) = session(false);
    m.shapes_col = false;
    let mut h = view(m);
    h.run_steps(4);
    h.get_by_label("Edit").click();
    h.run_steps(4);
    assert!(h.state().shapes_col, "Shapes while editing");
    h.get_by_label("Edit").click();
    h.run_steps(4);
    assert!(h.state().edit.is_none());
    assert!(!h.state().shapes_col, "back on Mods and meshes");
}

/// Note GB3: `ExtraValue::Str` holds an `Arc<str>`, which is written to the edit document as the plain string it
/// was when it held a `String`, and read back as one.
#[test]
fn string_extra_value_keeps_its_document_shape() {
    use nif::extra::ExtraValue;
    let mut d = doc();
    d.ops.push(DocOp {
        op: Op::EditExtra {
            at: Anchor {
                block: 4,
                kind: "NiStringExtraData".into(),
                shape: None,
                role: Role::Extra {
                    on_root: true,
                    name: "SDTA".into(),
                    kind: "NiStringExtraData".into(),
                    nth: 0,
                },
            },
            value: ExtraValue::Str("[1, 2]".into()),
            base: ExtraValue::Str("".into()),
        },
        status: OpStatus::Applied,
    });
    let t = d.to_text();
    assert!(t.contains("\nStr = \"[1, 2]\"\n"), "{t}");
    assert!(t.contains("\nStr = \"\"\n"), "{t}");
    assert_eq!(EditDoc::from_text(&t).unwrap(), d);
}

// ---- Note GD (DECISIONS review fixes, NIF team) ----

/// GD7: "In game (simulated)" says what it simulates, from the one const: on hover and as the first line under it.
#[test]
fn in_game_label_reads_simulated_as() {
    // GE7: the text names 2.3.1.
    assert_eq!(
        pgpsim::SIMULATED_AS,
        "Simulated as PGPatcher 2.3.1: loose JSON only, no modrules priority, read at the last scan"
    );
    let (_d, mut m) = pg_session();
    let (shape, _) = lit_shape(m.edit.as_ref().unwrap());
    m.select_edit(Selection::Shape(shape));
    let mut h = view(m);
    h.run_steps(4);
    h.get_by_label("In game").click();
    h.run_steps(4);
    wait_pg(h.state_mut());
    h.run_steps(4);
    h.get_by_label(pgpsim::SIMULATED_AS);
    h.get_by_label("In game").hover();
    h.run_steps(30);
    assert_eq!(
        h.query_all_by_label(pgpsim::SIMULATED_AS).count(),
        2,
        "the line and the hover"
    );
}

/// GD4: the shared Undo / Redo name the NIF step they act on (CI23's target, now visible).
#[test]
fn undo_target_carries_the_label() {
    let (_d, mut m) = session(false);
    m.open_edit();
    let e = m.edit.as_ref().unwrap();
    let (shape, shader) = lit_shape(e);
    let t = e.model.lighting[&shader].lighting.shader_type;
    let at = nif::edit::anchor_role(&e.model, Some(shape), Role::Shader).unwrap();
    m.edit_push(
        vec![Op::SetField {
            at,
            field: Field::ShaderType,
            value: Value::U32(if t == 1 { 0 } else { 1 }),
            base: Value::U32(t),
        }],
        None,
    );
    let mut h = view(m);
    sdc_gui::ui::header::set_meshes_view(&h.ctx, true);
    h.run_steps(4);
    let u = sdc_gui::ui::meshes::undo_target(&h.ctx).expect("the NIF history");
    assert!(u.can_undo && !u.can_redo);
    assert_eq!(u.undo_label.as_deref(), Some("Set shader type"));
    assert_eq!(u.redo_label, None);
    h.state_mut().undo_edit();
    h.run_steps(4);
    let u = sdc_gui::ui::meshes::undo_target(&h.ctx).unwrap();
    assert!(!u.can_undo && u.can_redo);
    assert_eq!(u.undo_label, None);
    assert_eq!(u.redo_label.as_deref(), Some("Set shader type"));
}

/// GD6 (CM27): a PBR material whose slot 5 (RMAOS) is empty renders fully metallic in game: an amber slot warning
/// (an item-1 row), never blocking.
#[test]
fn empty_rmaos_slot_is_a_row() {
    let mut m = direct(scenes::shared_alpha_and_set());
    let a = select(&mut m, "A");
    let rmaos = |m: &MeshesModel| {
        m.edit
            .as_ref()
            .unwrap()
            .warnings
            .iter()
            .filter(|w| w.text == nif::edit::validate::EMPTY_RMAOS)
            .map(|w| (w.level, w.target.clone()))
            .collect::<Vec<_>>()
    };
    assert!(rmaos(&m).is_empty(), "not PBR: no warning");
    let mut h = view(m);
    h.run_steps(2);
    set_slsf2(&mut h, a, 1 << 23);
    // The set has one slot: the warning shows under the Shader tab's slot rows.
    h.get_by_label(nif::edit::validate::EMPTY_RMAOS);
    let e = h.state().edit.as_ref().unwrap();
    let set = e.model.lighting[&e.model.shape(a).unwrap().shader.unwrap()]
        .texture_set
        .unwrap();
    assert_eq!(
        rmaos(h.state()),
        [(
            nif::edit::validate::Level::Warn,
            Target::Slot { set, slot: 5 }
        )]
    );
    assert_eq!(
        nif::edit::validate::EMPTY_RMAOS,
        "Slot 6 (RMAOS) is empty: in game the mesh renders fully metallic"
    );
    // GD1: the worklist row; the slot is not a `Field`, so the row names it and has no field.
    use sdc_gui::model::meshes::{NifLevel, NifTab};
    let rows: Vec<_> = h
        .state()
        .nif_problems()
        .iter()
        .filter(|p| p.text == nif::edit::validate::EMPTY_RMAOS)
        .map(|p| (p.level, p.tab, p.field))
        .collect();
    assert_eq!(rows, [(NifLevel::Warning, NifTab::Shader, None)]);
}

/// Note GD16: a trailer op's row names the field of the shader type in effect when it applied: edit Env map scale,
/// then change the type to Skin Tint, and the earlier op still reads "Env map scale".
#[test]
fn a_trailer_op_keeps_its_type_name_after_a_type_change() {
    let mut t = direct(scenes::lsp_shape(1, &[1.0]));
    let sh = select(&mut t, "S");
    let at =
        nif::edit::anchor_role(&t.edit.as_ref().unwrap().model, Some(sh), Role::Shader).unwrap();
    t.edit_push(
        vec![Op::SetField {
            at: at.clone(),
            field: Field::Trailer(0),
            value: Value::F32(2f32.to_bits()),
            base: Value::F32(1f32.to_bits()),
        }],
        None,
    );
    t.edit_push(
        vec![Op::SetField {
            at,
            field: Field::ShaderType,
            value: Value::U32(5),
            base: Value::U32(1),
        }],
        None,
    );
    let mut h = view(t);
    h.run_steps(3);
    h.get_by_label("Pending edits (2)").click();
    h.run_steps(3);
    h.get(row("S: Env map scale"));
    assert!(h.query(row("S: Skin tint R")).is_none());
}

/// Note GD22: after a source update a type change can conflict and leave the type as it is; a later trailer op is
/// named by the type really in effect, not by the failed change's.
#[test]
fn a_trailer_op_after_a_conflicting_type_change_is_named_by_the_real_type() {
    let mut t = direct(scenes::lsp_shape(1, &[1.0]));
    let sh = select(&mut t, "S");
    let at =
        nif::edit::anchor_role(&t.edit.as_ref().unwrap().model, Some(sh), Role::Shader).unwrap();
    t.edit_push(
        vec![Op::SetField {
            at: at.clone(),
            field: Field::ShaderType,
            value: Value::U32(5),
            base: Value::U32(1),
        }],
        None,
    );
    t.edit_push(
        vec![Op::SetField {
            at,
            field: Field::Trailer(0),
            value: Value::F32(0.5f32.to_bits()),
            base: Value::F32(0f32.to_bits()),
        }],
        None,
    );
    assert_eq!(t.edit.as_ref().unwrap().op_label_at(1), "S: Skin tint R");
    // the source is now Hair tint: the change from Env map (base 1) conflicts and the type stays 6
    t.edit_with(|e| {
        e.reload_source(scenes::lsp_shape(6, &[0.1, 0.2, 0.3]))
            .unwrap()
    });
    let e = t.edit.as_ref().unwrap();
    assert!(
        matches!(e.outcomes[0], nif::edit::Outcome::Conflict(_)),
        "{:?}",
        e.outcomes
    );
    assert_eq!(e.op_label_at(1), "S: Hair tint R");
}

/// Note GD15: two texture sets with an empty RMAOS are two worklist rows; only repeats of one target merge.
#[test]
fn two_sets_with_empty_rmaos_are_two_rows() {
    let mut m = direct(scenes::shared_alpha_and_set());
    let (a, b) = (select(&mut m, "A"), select(&mut m, "B"));
    let set_of = |m: &MeshesModel, s: u32| {
        let e = m.edit.as_ref().unwrap();
        e.model.lighting[&e.model.shape(s).unwrap().shader.unwrap()]
            .texture_set
            .unwrap()
    };
    for s in [a, b] {
        let e = m.edit.as_ref().unwrap();
        let at = nif::edit::anchor_role(&e.model, Some(s), Role::Shader).unwrap();
        let shader = e.model.shape(s).unwrap().shader.unwrap();
        let base = e.model.lighting[&shader].lighting.slsf2;
        let op = Op::SetBits {
            at,
            field: Field::Slsf2,
            mask: 1 << 23,
            value: 1 << 23,
            base,
        };
        m.edit_push(vec![op], None);
    }
    let rows = |m: &MeshesModel| {
        m.nif_problems()
            .iter()
            .filter(|p| p.text == nif::edit::validate::EMPTY_RMAOS)
            .count()
    };
    assert_eq!(rows(&m), 1, "one shared set: one row");
    // A path edit on B copies the shared set for B: two sets, each with its own empty slot 5.
    let e = m.edit.as_ref().unwrap();
    let base = String::from_utf8_lossy(&e.model.texsets[&set_of(&m, b)].raw.slots[0]).into_owned();
    let at = nif::edit::anchor_shape(&e.model, b);
    m.edit_push(
        vec![Op::SetTexturePath {
            at,
            slot: 0,
            path: "textures\\b.dds".into(),
            base,
        }],
        None,
    );
    assert_ne!(set_of(&m, a), set_of(&m, b), "B has its own set");
    assert_eq!(rows(&m), 2);
}

/// GD1: every warning and blocker of the open NIF edit is one worklist entry; the idle and no-folder reasons are not.
#[test]
fn nif_warnings_are_problems() {
    use nif::edit::Owner;
    use sdc_gui::model::meshes::{NifLevel, NifProblem, NifTab};
    // A read-only session (NO_EXPORT_FOLDER) and an untouched one (NO_EDITS) list nothing.
    let mut m = direct(scenes::shader_ctrl(false));
    assert!(m.nif_problems().is_empty(), "{:?}", m.nif_problems());
    let (_d, mut s) = session(false);
    s.open_edit();
    assert_eq!(s.edit.as_ref().unwrap().blocked.as_deref(), Some(NO_EDITS));
    assert!(s.nif_problems().iter().all(|p| p.level != NifLevel::Blocks));
    let idle = s.nif_problems();
    assert!(
        std::sync::Arc::ptr_eq(&idle, &s.nif_problems()),
        "cached on the edit's state (rule 9.1)"
    );

    // One controller warning (Warn) and one validation error that is also a blocker (Error, ALWAYS_BLOCK).
    let sh = select(&mut m, "S");
    let e = m.edit.as_ref().unwrap();
    let shader = e.model.shape(sh).unwrap().shader.unwrap();
    let lt = e.model.lighting[&shader].lighting.clone();
    let ctrl_at = nif::edit::anchor_role(
        &e.model,
        Some(sh),
        Role::Ctrl {
            owner: Owner::Shader,
            class: "BSLightingShaderPropertyFloatController".into(),
            var: Some(11),
            nth: 0,
        },
    )
    .unwrap();
    let freq = e.model.ctrls[&ctrl_at.block].frequency;
    let shader_at = nif::edit::anchor_role(&e.model, Some(sh), Role::Shader).unwrap();
    m.edit_push(
        vec![
            Op::SetField {
                at: ctrl_at,
                field: Field::Frequency,
                value: Value::F32(0f32.to_bits()),
                base: Value::F32(freq.to_bits()),
            },
            Op::SetBits {
                at: shader_at,
                field: Field::Slsf1,
                mask: 1 << 21,
                value: 1 << 21,
                base: lt.slsf1,
            },
        ],
        None,
    );
    let p = m.nif_problems();
    let path = "meshes/x.nif".to_string();
    assert_eq!(
        *p,
        [
            NifProblem {
                level: NifLevel::Blocks,
                path: path.clone(),
                text: nif::edit::validate::SKIN_TINT_TYPE.into(),
                tab: NifTab::Shader,
                field: Some(Field::Slsf1),
            },
            NifProblem {
                level: NifLevel::Warning,
                path,
                text: "Frequency ≤ 0 freezes the controller".into(),
                tab: NifTab::Controllers,
                field: Some(Field::Frequency),
            },
        ],
        "one row each, the blocker once"
    );

    // A trailer op's row names the field from the shader type it applies to.
    let mut t = direct(scenes::lsp_shape(1, &[1.0]));
    let sh = select(&mut t, "S");
    let at =
        nif::edit::anchor_role(&t.edit.as_ref().unwrap().model, Some(sh), Role::Shader).unwrap();
    t.edit_push(
        vec![Op::SetField {
            at,
            field: Field::Trailer(0),
            value: Value::F32(2f32.to_bits()),
            base: Value::F32(5f32.to_bits()), // not the file's 1.0: a conflict
        }],
        None,
    );
    let p = t.nif_problems();
    assert_eq!(p.len(), 1, "{p:?}");
    assert_eq!(p[0].level, NifLevel::Blocks);
    assert!(
        p[0].text
            .starts_with("S: Env map scale: changed in the source"),
        "{}",
        p[0].text
    );
    assert_eq!(
        (p[0].tab, p[0].field),
        (NifTab::Shader, Some(Field::Trailer(0)))
    );

    // The worklist's fix: the row's tab and field. A lit shape is selected, so the Shader tab would show "Shader type".
    let (shape, _) = lit_shape(s.edit.as_ref().unwrap());
    s.select_edit(Selection::Shape(shape));
    s.show_nif_tab("meshes/x.nif", NifTab::Alpha, Some(Field::AlphaThreshold));
    assert_eq!(
        s.nif_tab,
        Some((NifTab::Alpha, Some(Field::AlphaThreshold)))
    );
    let mut h = view(s);
    h.run_steps(4);
    assert!(h.state().nif_tab.is_none(), "the panel took the request");
    assert!(
        h.query_by_label("Texture slots").is_none() && h.query_by_label("Shader type").is_none(),
        "the Alpha tab shows, not the Shader tab"
    );
}

/// Note GD14: a worklist fix focuses its field whatever widget draws it: the Alpha threshold slider, the shader type
/// combo, a flag list (opened for it) and a colour's first channel.
#[test]
fn show_nif_tab_focuses_each_field_kind() {
    use sdc_gui::model::meshes::NifTab;
    let mut s = direct(scenes::shared_alpha_and_set());
    let (shape, _) = lit_shape(s.edit.as_ref().unwrap());
    s.select_edit(Selection::Shape(shape));
    let mut h = view(s);
    h.run_steps(3);
    for (tab, field, role, label) in [
        (
            NifTab::Alpha,
            Field::AlphaThreshold,
            AkRole::Slider,
            "Threshold",
        ),
        (
            NifTab::Shader,
            Field::ShaderType,
            AkRole::ComboBox,
            "Shader type",
        ),
        (NifTab::Shader, Field::Slsf1, AkRole::CheckBox, "Specular"),
        (
            NifTab::Shader,
            Field::EmissiveColor(0),
            AkRole::SpinButton,
            "Emissive colour R",
        ),
    ] {
        h.state_mut().show_nif_tab("meshes/x.nif", tab, Some(field));
        h.run_steps(4);
        let n = h.get_by_role_and_label(role, label);
        assert!(
            n.accesskit_node().is_focused(),
            "{field:?}: {label} has the focus"
        );
    }
}

/// GD1: the Problems list's fix sets `AppModel::nif_tab_request`; the app switches to the Meshes view and opens the tab.
#[test]
fn nif_tab_request_switches_to_the_meshes_view() {
    use sdc_gui::model::meshes::NifTab;
    let (_d, mut h) = app_edit(1400.0, 900.0, false);
    sdc_gui::ui::header::set_meshes_view(&h.ctx, false);
    h.run_steps(4);
    h.state_mut().model.nif_tab_request = Some(("meshes/x.nif".into(), NifTab::Alpha, None));
    h.run_steps(6);
    assert!(sdc_gui::ui::header::meshes_view_active(&h.ctx));
    assert!(h.state().model.nif_tab_request.is_none());
    assert!(h.state().meshes.nif_tab.is_none(), "the panel took it");
    assert!(
        h.query_by_label("Texture slots").is_none() && h.query_by_label("Shader type").is_none()
    );
}

// ---- Note GD2: Export NIF is a Review row ----

/// The one Review row (today only the open edit has one).
fn review_row(m: &MeshesModel) -> sdc_gui::model::meshes::NifReviewRow {
    let rows = m.nif_review_rows();
    assert_eq!(rows.len(), 1, "{rows:?}");
    rows.into_iter().next().unwrap()
}

fn sha(b: &[u8]) -> [u8; 32] {
    use sha2::Digest;
    sha2::Sha256::digest(b).into()
}

/// Note GD17: SHA-256 of `meshes\x.nif` as the footer's Export NIF wrote it at c42c2454, for
/// `export_session` plus `add_then_remove_ctrl(.., true)` (log: `r1-golden-capture-c42c2454.txt` in the ledger).
const EXPORT_GOLDEN_SHA256: &str =
    "30da078ae7184525c8427204a3e0bd133f86409533e1645329680a91032b89ac";

#[test]
fn nif_review_rows_list_the_pending_edit() {
    use sdc_gui::model::meshes::NifReviewRow;
    let (d, mut m) = export_session();
    m.open_edit();
    assert!(m.nif_review_rows().is_empty(), "no edits, no row");
    add_then_remove_ctrl(&mut m, true);
    let e = m.edit.as_ref().unwrap();
    let n = nif::header::Header::parse(&e.source).unwrap().sizes.len() as u32;
    let blocks = |b: &[u8]| nif::header::Header::parse(b).unwrap().sizes.len();
    let out = nif::edit::compact(&e.patched, n).unwrap();
    let dropped = blocks(&e.patched) - blocks(&out);
    assert!(dropped >= 2, "{dropped}");
    assert_eq!(
        review_row(&m),
        NifReviewRow {
            draft: autosave(&d),
            draft_unsaved: false,
            kept_backup: None,
            path: "meshes/x.nif".into(),
            mod_name: EDIT_MOD.into(),
            blocked: None,
            foreign: None,
            answer: None,
            compaction: Some(dropped),
        }
    );
    assert_eq!(m.export_nif_edits(), Ok(1));
    assert_eq!(std::fs::read(exported(&d)).unwrap(), out);
    // Note GD17: the fixed baseline, captured from the footer's export at c42c2454; a change in the shared writer or
    // the compaction changes it
    assert_eq!(
        sha256_hex(&std::fs::read(exported(&d)).unwrap()),
        EXPORT_GOLDEN_SHA256,
        "the Review export writes the footer's bytes"
    );
    assert!(m.nif_review_rows().is_empty(), "exported: nothing pending");
    // after the write: Re-run PGPatcher is a worklist row
    use sdc_gui::model::meshes::NifLevel;
    assert!(m
        .nif_problems()
        .iter()
        .any(|p| p.level == NifLevel::Warning && p.text == RERUN_PGPATCHER));
}

/// Note GD30 (final review M2): a malformed edit document's error names its file, as a read error does.
#[test]
fn a_malformed_edit_document_names_its_file() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("bad.toml");
    std::fs::write(&p, b"not = [toml").unwrap();
    let e = load(&p).unwrap_err();
    assert!(e.starts_with(&format!("{}: ", p.display())), "{e}");
}

/// Note GD30 (final review M3): a block reason no row covers gets its row even when another row already blocks: a
/// refused op and a failed folder change are two rows.
#[test]
fn a_failed_folder_change_is_a_row_beside_a_refused_op() {
    let (d, mut m) = export_session();
    m.open_edit();
    let (shape, _) = lit_shape(m.edit.as_ref().unwrap());
    let at =
        nif::edit::anchor_role(&m.edit.as_ref().unwrap().model, Some(shape), Role::Shader).unwrap();
    m.edit_push(
        vec![Op::SetBits {
            at,
            field: Field::ShaderType,
            mask: 1,
            value: 1,
            base: 0,
        }],
        None,
    );
    use sdc_gui::model::meshes::NifLevel;
    // Note GD33: the refused op's own row, never export_block's "1 edit cannot be applied." summary beside it
    let refused = |rows: &[sdc_gui::model::meshes::NifProblem]| {
        rows.iter()
            .filter(|p| p.level == NifLevel::Blocks && p.text.contains("not a bit field"))
            .count()
    };
    let rows = m.nif_problems();
    assert_eq!((rows.len(), refused(&rows)), (1, 1), "{rows:?}");
    let old = d.path().join("mods").join(EDIT_MOD);
    let bad = d.path().join(r"mods\Bad");
    std::fs::create_dir_all(nif_doc(&bad).parent().unwrap()).unwrap();
    std::fs::write(nif_doc(&bad), b"not = [toml").unwrap();
    m.set_nif_export_root(Some(bad.clone()));
    let rows = m.nif_problems();
    let head = format!("could not open the edit document in {}: ", bad.display());
    assert_eq!((rows.len(), refused(&rows)), (2, 1), "{rows:?}");
    assert!(
        rows.iter()
            .any(|p| p.level == NifLevel::Blocks && p.text.starts_with(&head)),
        "{rows:?}"
    );
    m.set_nif_export_root(Some(old));
    let rows = m.nif_problems();
    assert_eq!((rows.len(), refused(&rows)), (1, 1), "{rows:?}");
}

/// Note GD29: a Review answer does not outlive its Review: a folder change, closing the edit or `clear_nif_answers`
/// (Review closed, project changed) drops it, and the row asks again, even for the same bytes.
#[test]
fn a_review_answer_is_dropped_by_a_folder_change_or_a_close() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    let junk = |p: &Path| {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"junk").unwrap();
    };
    junk(&exported(&d));
    m.set_nif_answer("meshes/x.nif", GuardChoice::Overwrite, sha(b"junk"));
    assert_eq!(review_row(&m).answer, Some(GuardChoice::Overwrite));
    let good = d.path().join(r"mods\Good");
    junk(&good.join(r"meshes\x.nif"));
    m.set_nif_export_root(Some(good.clone()));
    assert_eq!(m.nif_export_root.as_deref(), Some(good.as_path()));
    assert_eq!(review_row(&m).answer, None, "a folder change asks again");
    m.set_nif_answer("meshes/x.nif", GuardChoice::Overwrite, sha(b"junk"));
    m.close_edit();
    m.open_edit();
    assert_eq!(review_row(&m).answer, None, "closing the edit asks again");
    m.set_nif_answer("meshes/x.nif", GuardChoice::Overwrite, sha(b"junk"));
    m.clear_nif_answers();
    assert_eq!(review_row(&m).answer, None);
}

#[test]
fn a_foreign_nif_waits_for_an_answer_on_its_row() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    let p = exported(&d);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, b"junk").unwrap();
    let r = review_row(&m);
    assert_eq!(
        (r.foreign, r.answer, r.blocked),
        (Some(sha(b"junk")), None, None)
    );
    let skipped = |why: &str| Err(format!("NIF edits skipped: meshes/x.nif: {why}"));
    assert_eq!(m.export_nif_edits(), skipped(NIF_UNANSWERED));
    assert_eq!(std::fs::read(&p).unwrap(), b"junk");
    m.set_nif_answer("meshes/x.nif", GuardChoice::KeepTheirs, sha(b"junk"));
    assert_eq!(review_row(&m).answer, Some(GuardChoice::KeepTheirs));
    assert_eq!(m.export_nif_edits(), skipped(NIF_KEPT));
    assert_eq!(std::fs::read(&p).unwrap(), b"junk");
    assert_eq!(review_row(&m).answer, None, "an answer is used once");
    // an Overwrite given for bytes that changed since is not applied
    m.set_nif_answer("meshes/x.nif", GuardChoice::Overwrite, sha(b"junk"));
    std::fs::write(&p, b"junk2").unwrap();
    let r = review_row(&m);
    assert_eq!(r.blocked.as_deref(), Some(NIF_CHANGED_SINCE));
    assert_eq!(r.answer, None);
    assert_eq!(m.export_nif_edits(), skipped(NIF_CHANGED_SINCE));
    assert_eq!(std::fs::read(&p).unwrap(), b"junk2");
    m.set_nif_answer("meshes/x.nif", GuardChoice::Overwrite, sha(b"junk2"));
    assert_eq!(m.export_nif_edits(), Ok(1));
    let e = m.edit.as_ref().unwrap();
    assert_eq!(std::fs::read(&p).unwrap(), *e.patched);
}

/// Note GD2: the block diff the guard dialog showed is the model's `nif_diff`, for the Review row's Diff…; only a row
/// with a foreign file has one.
#[test]
fn a_foreign_nif_row_has_the_block_diff() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    assert_eq!(m.nif_diff("meshes/x.nif"), None, "nothing on disk");
    let p = exported(&d);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    let source = m.edit.as_ref().unwrap().source.to_vec();
    std::fs::write(&p, &source).unwrap();
    let diff = m
        .nif_diff("meshes/x.nif")
        .expect("a foreign file has a diff");
    assert!(!diff.is_empty(), "the unedited file differs in the edit");
    assert!(!diff[0].contains("does not read as a NIF"), "{diff:?}");
    std::fs::write(&p, b"junk").unwrap();
    let diff = m.nif_diff("meshes/x.nif").unwrap();
    assert!(diff[0].contains("does not read as a NIF"), "{diff:?}");
    assert_eq!(m.nif_diff("meshes/other.nif"), None);
}

/// Note GD18: `show_nif_diff` (Review's Diff…, FC5) opens the edit panel's Diff view with `nif_diff`'s lines; Close
/// shuts it, and the footer's own Diff… opens it again.
#[test]
fn show_nif_diff_opens_the_diff_view() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    let p = exported(&d);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, m.edit.as_ref().unwrap().source.to_vec()).unwrap();
    let want = m
        .nif_diff("meshes/x.nif")
        .expect("a foreign file has a diff");
    m.show_nif_diff("meshes/x.nif");
    assert_eq!(m.nif_diff_view, Some(("meshes/x.nif".into(), want.clone())));
    let mut h = view(m);
    h.run_steps(3);
    let shown = |h: &Harness<'static, MeshesModel>| {
        want.iter().all(|l| {
            h.query_all_by_label(&format!("Diff line: {l}"))
                .next()
                .is_some()
        })
    };
    assert!(shown(&h), "every line of nif_diff is in the view");
    h.get_by_role_and_label(AkRole::Button, "Close the diff")
        .click();
    h.run_steps(3);
    assert!(!shown(&h));
    assert_eq!(h.state().nif_diff_view, None);
    h.get_by_role_and_label(AkRole::Button, "Diff…").click();
    h.run_steps(3);
    assert!(shown(&h), "the footer's Diff… opens the same view");
}

#[test]
fn a_blocked_nif_row_is_skipped_by_the_export() {
    let (_d, mut m) = export_session();
    m.set_nif_export_root(None);
    m.open_edit();
    gloss(&mut m, 12.5);
    let r = review_row(&m);
    assert_eq!(r.blocked.as_deref(), Some(NO_EXPORT_FOLDER));
    assert_eq!(r.mod_name, "");
    assert_eq!(
        m.export_nif_edits(),
        Err(format!(
            "NIF edits skipped: meshes/x.nif: {NO_EXPORT_FOLDER}"
        ))
    );
}

// ---- Note GF: the app wires the NIF edit to Review (app.rs): rows, answers, Export, Diff… and the footer's Review… ----

/// The app in the Meshes view with one pending NIF edit, its export folder the Textures one (`support::opened`).
fn review_app() -> (tempfile::TempDir, tempfile::TempDir, AppH) {
    app_data();
    let t = tempfile::tempdir().unwrap();
    let model = support::opened(&t.path().join("mod"));
    let (d, h) = in_the_meshes_view(model, 1);
    (t, d, h)
}

fn nif_out(h: &AppH) -> (String, std::path::PathBuf, Vec<u8>) {
    let e = h.state().meshes.edit.as_ref().unwrap();
    (e.data_path.clone(), e.export_path.clone(), e.out.to_vec())
}

/// Review's Export as the user does it (its model call), then frames until the run and the NIF export after it end.
fn export_from_review(h: &mut AppH) {
    h.state_mut().model.export_clicked();
    assert!(h.state().model.review.is_some(), "NIF edits open Review");
    h.state_mut().model.export_from_review();
    let start = Instant::now();
    while !(h.state().model.export.summary.is_some() && h.state().model.review.is_none()) {
        assert!(
            start.elapsed() < Duration::from_secs(120),
            "{}",
            h.state().model.status_line
        );
        h.run_steps(1);
    }
    h.run_steps(4);
}

#[test]
fn reviews_export_writes_the_nif_edit() {
    let (_t, _d, mut h) = review_app();
    let (path, out_path, out) = nif_out(&h);
    assert!(
        !out_path.as_os_str().is_empty(),
        "the edit has an export path"
    );
    let rows = h.state().model.nif_review_rows().to_vec();
    assert_eq!(
        rows.len(),
        1,
        "the app hands the NIF rows to Review: {rows:?}"
    );
    assert_eq!(rows[0].path, path);
    export_from_review(&mut h);
    assert_eq!(
        std::fs::read(&out_path).ok(),
        Some(out),
        "Review's Export wrote the edit"
    );
    assert!(!h.state().meshes.edit.as_ref().unwrap().doc.pending());
    assert!(h
        .state()
        .model
        .messages
        .entries()
        .any(|e| e.text == "NIF edits written: 1 file"));
}

#[test]
fn queued_review_export_stops_when_a_kept_nif_disappears() {
    let (_t, _d, mut h) = review_app();
    let (path, out, _) = nif_out(&h);
    let p = draft(h.state().model.root.as_deref().unwrap());
    let record = nif_doc(&h.state().model.nif_export_root().unwrap());
    h.state_mut().meshes.close_edit();
    let mut legacy = load(&p).unwrap().unwrap();
    legacy.id = None;
    save(&p, &legacy).unwrap();
    let before = std::fs::read(&p).unwrap();
    h.state_mut().meshes.open_edit();
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    std::fs::write(&out, b"theirs").unwrap();
    h.run_steps(4);
    h.state_mut().model.open_review();
    h.state_mut()
        .model
        .answer_review_nif(&path, false, sha(b"theirs"));
    h.run_steps(4);
    let m = &mut h.state_mut().model;
    assert_eq!(m.nif_review_rows()[0].answer, Some(GuardChoice::KeepTheirs));
    assert!(m.nif_review_rows()[0].draft_unsaved);
    assert!(!m
        .review_writes()
        .also_updated
        .iter()
        .any(|w| [&out, &record, &p].contains(&&w.path)));
    m.export_from_review();
    assert!(
        m.export.running.is_some(),
        "texture export queued the NIF phase"
    );
    std::fs::remove_file(&out).unwrap();
    let start = Instant::now();
    while h.state().model.export.summary.is_none() {
        assert!(start.elapsed() < Duration::from_secs(120));
        h.run_steps(1);
    }
    h.run_steps(4);
    assert!(
        !out.exists(),
        "queued export wrote a NIF that Review skipped"
    );
    assert!(!record.exists());
    assert_eq!(std::fs::read(p).unwrap(), before);
    assert!(h
        .state()
        .model
        .export
        .notes
        .iter()
        .any(|s| s.contains("Open Review again.")));
}

#[test]
fn review_export_creates_no_unlisted_backup_of_the_winning_kept_edit() {
    let (_t, d, mut h) = review_app();
    let p = draft(h.state().model.root.as_deref().unwrap());
    h.state_mut().meshes.close_edit();
    let doc = load(&p).unwrap().unwrap();
    let k = kept(&d, doc.id.as_ref().unwrap());
    save(&k, &with_value(doc, 9.5)).unwrap();
    ahead(&k, 3600);
    h.state_mut().meshes.open_edit();
    assert_eq!(vals(&h.state().meshes), [9.5]);
    h.run_steps(4);
    let (_, out, bytes) = nif_out(&h);
    export_from_review(&mut h);
    assert_eq!(std::fs::read(out).unwrap(), bytes);
    assert_eq!(doc_values(&p), [9.5]);
    let writes = h.state().model.review_writes_at_open();
    let listed = support::listed(&writes);
    for backup in moved_aside_from(&k) {
        assert!(
            listed.iter().any(|w| w.path == backup),
            "unlisted kept backup {}",
            backup.display()
        );
    }
}

#[test]
fn review_export_lists_the_backup_of_a_losing_kept_edit() {
    let (_t, d, mut h) = review_app();
    let p = draft(h.state().model.root.as_deref().unwrap());
    h.state_mut().meshes.close_edit();
    let doc = load(&p).unwrap().unwrap();
    let k = kept(&d, doc.id.as_ref().unwrap());
    save(&k, &with_value(doc.clone(), 8.5)).unwrap();
    ahead(&k, 3600);
    let plain = autosave(&d);
    save(&plain, &with_value(doc, 9.5)).unwrap();
    ahead(&plain, 7200);
    h.state_mut().meshes.open_edit();
    assert_eq!(vals(&h.state().meshes), [9.5]);
    h.run_steps(4);
    // A second process takes the offered name before the write. The next Review can offer another one.
    let offered = h.state().model.nif_review_rows()[0]
        .kept_backup
        .clone()
        .unwrap();
    std::fs::write(&offered, b"already taken").unwrap();
    assert!(
        export(&mut h.state_mut().meshes).is_err(),
        "a taken backup name needs a new Review"
    );
    assert_eq!(std::fs::read(&offered).unwrap(), b"already taken");
    assert_eq!(doc_values(&k), [8.5]);
    assert_eq!(doc_values(&plain), [9.5]);
    h.run_steps(4);
    export_from_review(&mut h);
    let mut backups = moved_aside_from(&k);
    backups.retain(|p| p != &offered);
    assert_eq!(backups.len(), 1);
    assert_eq!(doc_values(&backups[0]), [8.5]);
    assert_eq!(doc_values(&p), [9.5]);
    let writes = h.state().model.review_writes_at_open();
    assert!(
        support::listed(&writes)
            .iter()
            .any(|w| w.path == backups[0]),
        "Review omitted the losing kept edit's backup"
    );
}

#[test]
fn a_foreign_nifs_overwrite_answer_reaches_the_export() {
    let (_t, _d, mut h) = review_app();
    let (path, out_path, out) = nif_out(&h);
    std::fs::create_dir_all(out_path.parent().unwrap()).unwrap();
    let theirs = h.state().meshes.edit.as_ref().unwrap().source.to_vec();
    std::fs::write(&out_path, &theirs).unwrap();
    h.run_steps(4);
    // Unanswered, Review's gate waits and nothing is written.
    h.state_mut().model.export_clicked();
    h.run_steps(4);
    assert!(h.state().model.review_gate().is_err());
    h.state_mut().model.export_from_review();
    h.run_steps(8);
    assert_eq!(
        std::fs::read(&out_path).unwrap(),
        theirs,
        "skipped without an answer"
    );
    h.get_by_label(&format!("{path} was not written by Tessalith"));
    h.get_by_label("Overwrite it").click();
    h.run_steps(4);
    let rows = h.state().model.nif_review_rows().to_vec();
    assert_eq!(rows[0].answer, Some(GuardChoice::Overwrite), "{rows:?}");
    export_from_review(&mut h);
    assert_eq!(
        std::fs::read(&out_path).unwrap(),
        out,
        "the answer overwrote it"
    );
}

#[test]
fn reviews_diff_opens_the_edit_panels_diff_view() {
    let (_t, _d, mut h) = review_app();
    let (path, out_path, _) = nif_out(&h);
    std::fs::create_dir_all(out_path.parent().unwrap()).unwrap();
    let theirs = h.state().meshes.edit.as_ref().unwrap().source.to_vec();
    std::fs::write(&out_path, &theirs).unwrap();
    h.run_steps(4);
    h.state_mut().model.open_review();
    h.run_steps(4);
    assert_eq!(h.state().meshes.nif_diff_view, None);
    h.get_by_role_and_label(AkRole::Button, &format!("Diff… {path}"))
        .click();
    h.run_steps(4);
    let want = h
        .state()
        .meshes
        .nif_diff(&path)
        .expect("a foreign file has a diff");
    assert_eq!(h.state().meshes.nif_diff_view, Some((path, want.clone())));
    assert!(sdc_gui::ui::header::meshes_view_active(&h.ctx));
    assert!(h
        .query_all_by_label(&format!("Diff line: {}", want[0]))
        .next()
        .is_some());
}

#[test]
fn the_footers_review_opens_review() {
    let (_t, _d, mut h) = review_app();
    assert!(h.state().model.review.is_none());
    h.get(review_button()).click();
    h.run_steps(4);
    assert!(
        h.state().model.review.is_some(),
        "the footer's Review… opens Review"
    );
}

/// Note GF9: a cancelled run takes its Overwrite answer with it: the reopened Review asks again, and an Export
/// without a new answer leaves the foreign file as it is.
#[test]
fn a_cancelled_export_drops_the_overwrite_answer() {
    let (_t, _d, mut h) = review_app();
    let (path, out_path, _) = nif_out(&h);
    std::fs::create_dir_all(out_path.parent().unwrap()).unwrap();
    let theirs = h.state().meshes.edit.as_ref().unwrap().source.to_vec();
    std::fs::write(&out_path, &theirs).unwrap();
    h.run_steps(4);
    h.state_mut().model.export_clicked();
    h.run_steps(4);
    h.get_by_label("Overwrite it").click();
    h.run_steps(4);
    assert_eq!(
        h.state().model.nif_review_rows()[0].answer,
        Some(GuardChoice::Overwrite)
    );
    h.state_mut().model.export_from_review();
    h.state_mut().model.cancel_export();
    let start = Instant::now();
    while h.state().model.export.running.is_some() {
        assert!(start.elapsed() < Duration::from_secs(120));
        h.run_steps(1);
    }
    h.run_steps(4);
    assert_eq!(
        std::fs::read(&out_path).unwrap(),
        theirs,
        "the cancel wrote nothing"
    );
    // Reopened, the row asks again.
    h.state_mut().model.export_clicked();
    h.run_steps(4);
    let rows = h.state().model.nif_review_rows().to_vec();
    assert_eq!(rows[0].answer, None, "{rows:?}");
    assert!(h.state().model.review_gate().is_err());
    h.get_by_label(&format!("{path} was not written by Tessalith"));
    h.state_mut().model.export_from_review();
    h.run_steps(8);
    assert_eq!(
        std::fs::read(&out_path).unwrap(),
        theirs,
        "no Export without a new answer"
    );
}

/// Plan 3c Task 4, ruling 3c-51 (b): pending edits recovered from a newer scan-store autosave, and Review's Export is
/// their first save. With dirty texture settings, the export removes both the NIF autosave and the settings recovery.
/// Each removal counts under its own listed document.
#[test]
fn reviews_export_of_a_recovered_autosave_lists_what_it_changes() {
    app_data();
    let t = tempfile::tempdir().unwrap();
    let model = support::opened(&t.path().join("mod"));
    let d = files(false);
    let mut app = sdc_gui::app::SdcApp::new(model);
    app.meshes.procs = sdc_gui::model::meshes::no_procs;
    app.meshes.store_path = Some(d.path().join("scan.db"));
    scan_and_select(&mut app.meshes, d.path());
    // An edit made with no export folder autosaves beside the scan store (Note GD5)…
    app.meshes.set_nif_export_root(None);
    app.meshes.open_edit();
    tick(&mut app.meshes, 40.5);
    app.meshes.edit.as_mut().unwrap().flush();
    app.meshes.close_edit();
    assert!(autosave(&d).is_file());
    // …and the next session, in the Textures export folder, recovers it unsaved (Note GD11).
    let root = app.model.nif_export_root();
    app.meshes.set_nif_export_root(root);
    app.meshes.set_project_root(app.model.root.as_deref());
    app.meshes.open_edit();
    assert_eq!(op_values(&app.meshes).len(), 1, "recovered");
    let mut frame = eframe::Frame::_new_kittest();
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1400.0, 900.0))
        .build_ui_state(
            move |ui, app: &mut sdc_gui::app::SdcApp| eframe::App::ui(app, ui, &mut frame),
            app,
        );
    sdc_gui::ui::header::set_meshes_view(&h.ctx, true);
    h.run_steps(4);
    assert!(autosave(&d).is_file(), "guard: nothing saved before Export");
    let m = &mut h.state_mut().model;
    m.decide(
        "landscape/leaf.png",
        sdc_gui::model::Answer::Alpha(engine::config::AlphaPolicy::Blend),
    );
    assert!(m.wait_idle(Duration::from_secs(60)));
    let settings = m.project.as_ref().unwrap();
    assert!(settings.is_dirty());
    let recovery = sdc_gui::crash_log::recovery_path(settings.path());
    std::fs::write(&recovery, "pending texture settings").unwrap();
    let (store, mods) = (d.path().join("nif-edits"), d.path().join("mods"));
    let roots = [t.path(), store.as_path(), mods.as_path()];
    let before = support::tree_hashes(&roots);
    let pending = support::PendingRemovals::before_export(&h.state().model);
    export_from_review(&mut h);
    let after = support::tree_hashes(&roots);
    assert!(!autosave(&d).exists(), "the export's save removed it");
    assert!(before.contains_key(&recovery) && !after.contains_key(&recovery));
    let model = &h.state().model;
    let w = model.review_writes_at_open();
    let removals = pending.after_export(model);
    let bad = support::compare_listing(&w, &before, &after, Some(d.path()), &removals).violations;
    assert!(bad.is_empty(), "{bad:#?}");
}

// ---- Note GH5, GH8: the footer before any edit, and Review's NIF export with no project ----

/// Note GH5: nothing edited is the idle state with no export folder too (the folder check used to come first and
/// showed a red `Blocked: No export folder chosen` over an untouched edit).
#[test]
fn an_untouched_edit_with_no_folder_is_idle_not_blocked() {
    let (_d, mut m) = export_session();
    m.set_nif_export_root(None);
    m.open_edit();
    assert_eq!(
        m.edit.as_ref().unwrap().blocked.as_deref(),
        Some(NO_EDITS),
        "NO_EDITS comes before NO_EXPORT_FOLDER"
    );
    let mut h = view(m);
    h.run_steps(3);
    h.get_by_label(NO_EDITS);
    h.get_by_label("idle");
    assert!(h.query_by_label_contains("Blocked").is_none());
}

/// Note GH5: one edit and no folder is Review's own case (a weak `skipped` line, not a block): the footer says the
/// same, and the Review row still carries `NO_EXPORT_FOLDER`.
#[test]
fn an_edit_with_no_folder_reads_skipped_until_a_folder_is_chosen() {
    let (_d, mut m) = export_session();
    m.set_nif_export_root(None);
    m.open_edit();
    gloss(&mut m, 12.5);
    assert_eq!(
        m.edit.as_ref().unwrap().blocked.as_deref(),
        Some(NO_EXPORT_FOLDER)
    );
    assert_eq!(review_row(&m).blocked.as_deref(), Some(NO_EXPORT_FOLDER));
    let mut h = view(m);
    h.run_steps(3);
    h.get_by_label("Skipped until an export folder is chosen");
    h.get_by_label("idle");
    assert!(h.query_by_label_contains("Blocked").is_none());
}

/// The app in the Meshes view with no project open, gui.toml naming `export` (or nothing), and `ops` pending edits.
fn no_project_app(export: Option<&Path>, ops: usize) -> (tempfile::TempDir, AppH) {
    app_data();
    let settings = sdc_gui::settings::GuiSettings {
        export_folder: export.map(Path::to_path_buf),
        ..Default::default()
    };
    let model = sdc_gui::model::AppModel::new(settings, None, support::session());
    assert!(model.root.is_none());
    in_the_meshes_view(model, ops)
}

/// Every file under `p` except the scan store, which sqlite may touch when only read.
fn tree(p: &Path) -> Vec<std::path::PathBuf> {
    let mut v = vec![];
    let mut todo = vec![p.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let q = e.path();
            if q.is_dir() {
                todo.push(q);
            } else if !e.file_name().to_string_lossy().starts_with("scan.db") {
                v.push(q);
            }
        }
    }
    v.sort();
    v
}

/// Note GH8: with no project and gui.toml's folder, Review's Export is on, reads `Export 1 NIF edit`, and writes the
/// NIF alone under that folder: no texture run, no `sdc-out`.
#[test]
fn meshes_only_review_exports_the_nif_edit() {
    let out = tempfile::tempdir().unwrap();
    let (_d, mut h) = no_project_app(Some(out.path()), 0);
    assert!(h.state().meshes.selected_nif.is_some());
    assert!(h.state().model.nif_review_rows().is_empty());
    assert_eq!(support::header_export_action(&h), None);
    assert!(h.query_by_label("Files").is_none());
    tick(&mut h.state_mut().meshes, 40.5);
    h.state_mut().meshes.edit.as_mut().unwrap().flush();
    h.run_steps(4);
    assert_eq!(h.state().model.nif_review_rows().len(), 1);
    support::header_undo(&h, "Undo");
    support::header_undo(&h, "Redo");
    assert_eq!(support::header_export_label(&h), "Review 1 NIF edit…");
    let review = h.get_by_role_and_label(AkRole::Button, "Review 1 NIF edit…");
    assert!(review.accesskit_node().is_disabled());
    review.hover();
    h.ctx.global_style_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
    h.run_steps(4);
    h.get_by_label("Open Review from the NIF edit panel (Review…)");
    h.get_by_label("Files");
    h.get_by_label_contains("Problems");
    h.get_by_label_contains("Messages");
    assert!(h.query_by_label("Save").is_none(), "texture Save is hidden");
    let (_path, out_path, bytes) = nif_out(&h);
    assert!(out_path.starts_with(out.path()), "{out_path:?}");
    h.state_mut().model.open_review();
    h.run_steps(4);
    assert_eq!(h.state().model.review_gate(), Ok((0, 0)));
    let b = h.get_by_role_and_label(AkRole::Button, "Export 1 NIF edit");
    assert!(!b.accesskit_node().is_disabled());
    // Ruling 3c-43: no project, nothing to set the key for: the opt-out box is not drawn (Review is, as the button shows).
    assert!(h
        .query_by_label("Export without review next time")
        .is_none());
    h.get_by_role_and_label(AkRole::Button, "Export 1 NIF edit")
        .click();
    h.run_steps(6);
    assert_eq!(std::fs::read(&out_path).ok(), Some(bytes));
    assert!(out_path.starts_with(out.path().join("meshes")));
    assert!(h.state().model.review.is_none(), "Review closes");
    assert!(h.state().model.export.running.is_none(), "no texture run");
    assert!(h
        .state()
        .model
        .messages
        .entries()
        .any(|e| e.text == "NIF edits written: 1 file"));
    assert!(!out.path().join("sdc-out").exists());
    assert!(!Path::new("sdc-out").exists());
}

/// Note GH8 / Review Focus 2: no project and no folder chosen: the gate says so, Export is off, nothing is written.
#[test]
fn meshes_only_without_a_folder_writes_nothing() {
    let (d, mut h) = no_project_app(None, 1);
    h.state_mut().model.open_review();
    h.run_steps(4);
    assert_eq!(
        h.state().model.review_gate(),
        Err("No export folder chosen".to_string())
    );
    assert!(h.query_all_by_label("No export folder chosen").count() >= 1);
    let before = tree(d.path());
    assert!(h
        .get_by_role_and_label(AkRole::Button, "Export 0 NIF edits")
        .accesskit_node()
        .is_disabled());
    h.state_mut().model.export_from_review();
    h.run_steps(6);
    assert!(h.state().model.review.is_some(), "Review stays open");
    assert!(!h
        .state()
        .model
        .messages
        .entries()
        .any(|e| e.text.starts_with("NIF edits")));
    assert_eq!(tree(d.path()), before);
    assert!(!d.path().join("sdc-out").exists());
    assert!(!Path::new("sdc-out").exists());
}

/// Note GH8: a file Tessalith did not write at the export path is never overwritten without the row's answer.
#[test]
fn meshes_only_waits_for_a_foreign_answer() {
    let out = tempfile::tempdir().unwrap();
    let (_d, mut h) = no_project_app(Some(out.path()), 1);
    let (_path, out_path, bytes) = nif_out(&h);
    std::fs::create_dir_all(out_path.parent().unwrap()).unwrap();
    std::fs::write(&out_path, b"theirs").unwrap();
    h.run_steps(4);
    h.state_mut().model.open_review();
    h.run_steps(4);
    assert_eq!(
        h.state().model.review_gate(),
        Err("Answer Keep or Overwrite for x.nif".to_string())
    );
    h.state_mut().model.export_from_review();
    h.run_steps(6);
    assert_eq!(std::fs::read(&out_path).unwrap(), b"theirs");
    h.get_by_label("Overwrite it").click();
    h.run_steps(4);
    assert_eq!(h.state().model.review_gate(), Ok((0, 0)));
    h.get_by_role_and_label(AkRole::Button, "Export 1 NIF edit")
        .click();
    h.run_steps(6);
    assert_eq!(std::fs::read(&out_path).unwrap(), bytes);
}

// ---- Note GH9: right-click `Export this NIF` on the Edit-mode 3D view ----

const EXPORT_THIS_NIF: &str = "Export this NIF";

/// Steps from `hover_at` until a hover text is in the AccessKit tree, with kittest's 0.25 s step and egui's 0.5 s
/// tooltip delay. The delay ends on step 3, where the tooltip's area runs its first frame as an invisible sizing
/// pass (`egui-0.37.0\src\containers\area.rs:691-692`, as in 0.36.2); it is painted on step 4 in both versions.
/// egui 0.36.2 put that invisible frame's text in the tree on step 3; 0.37 leaves an invisible widget out of it
/// (`egui-0.37.0\src\context.rs:655` through `is_accesskit_visible`, `:4957`). Measured: 3 steps fail, 4 pass
/// (0.36.2's tests used 3).
const TOOLTIP_STEPS: usize = 4;

/// Right-click the Meshes view's 3D image (the click lands on the image centre) and let the menu open.
fn right_click_3d<S>(h: &mut Harness<'static, S>) {
    h.get_by_label(sdc_gui::model::viewport3d::FRAME_LABEL)
        .click_secondary();
    h.run_steps(4);
}

fn is_disabled<S>(h: &Harness<'static, S>, label: &str) -> bool {
    h.get_by_role_and_label(AkRole::Button, label)
        .accesskit_node()
        .is_disabled()
}

/// Q1, Q2, Q5: a plain right-click offers `Export this NIF`; it writes the open NIF alone and reports as Review does.
#[test]
fn right_click_on_the_3d_view_offers_export_this_nif() {
    let (_t, _d, mut h) = review_app();
    let (_path, out_path, out) = nif_out(&h);
    assert!(h.query_by_label(EXPORT_THIS_NIF).is_none());
    right_click_3d(&mut h);
    assert!(!is_disabled(&h, EXPORT_THIS_NIF));
    h.get_by_role_and_label(AkRole::Button, EXPORT_THIS_NIF)
        .click();
    h.run_steps(6);
    assert_eq!(
        std::fs::read(&out_path).ok(),
        Some(out),
        "the NIF is written"
    );
    assert!(h.state().model.review.is_none(), "no Review");
    assert!(h.state().model.export.running.is_none(), "no texture run");
    assert!(
        h.query_by_label(EXPORT_THIS_NIF).is_none(),
        "the menu closes"
    );
    assert!(
        h.state()
            .model
            .messages
            .entries()
            .any(|e| e.text == "NIF edits written: 1 file"),
        "the result goes through nif_export_result"
    );
    assert!(!h.state().meshes.edit.as_ref().unwrap().doc.pending());
}

#[test]
fn exporting_the_last_nif_keeps_its_unseen_message_unread() {
    let out = tempfile::tempdir().unwrap();
    let (_d, mut h) = no_project_app(Some(out.path()), 1);
    let (_, path, bytes) = nif_out(&h);
    assert_eq!(h.state().model.nif_review_rows().len(), 1);
    h.get_by_label_contains("Messages").click();
    h.run_steps(4);
    h.get_by_label("Clear");
    assert_eq!(h.state().model.messages.unread(), 0);

    right_click_3d(&mut h);
    let read = h.state().model.messages.len();
    h.get_by_role_and_label(AkRole::Button, EXPORT_THIS_NIF)
        .click();
    h.run_steps(6);
    assert_eq!(std::fs::read(path).unwrap(), bytes);
    let model = &h.state().model;
    assert!(model.nif_review_rows().is_empty());
    assert!(h.query_by_label_contains("Messages").is_none());
    assert_eq!(
        model.messages.entries().next_back().unwrap().text,
        "NIF edits written: 1 file"
    );
    let unread = model.messages.unread();
    assert_eq!(
        unread,
        model.messages.len() - read,
        "the tab never drew these entries"
    );
    assert_eq!(
        model
            .messages
            .entries()
            .rev()
            .take(unread)
            .filter(|e| e.text == "NIF edits written: 1 file")
            .count(),
        1,
        "the export result is unread"
    );
}

/// Q5: the model keeps the result for the app to hand over.
#[test]
fn quick_export_keeps_its_result_for_the_app() {
    let (d, mut m) = export_session();
    m.open_edit();
    assert_eq!(m.quick_export_block().as_deref(), Some(NO_EDITS));
    gloss(&mut m, 12.5);
    assert_eq!(m.quick_export_block(), None);
    m.quick_export();
    assert_eq!(m.quick_export_result, Some(Ok(1)));
    assert!(exported(&d).exists());
    assert_eq!(m.quick_export_block().as_deref(), Some(NO_EDITS));
}

/// Review Focus 3: a right-drag orbits and opens no menu.
#[test]
fn a_right_drag_orbits_and_opens_no_menu() {
    let (_t, _d, mut h) = review_app();
    let yaw = h.state().meshes.v3.orbit.yaw;
    let c = h
        .get_by_label(sdc_gui::model::viewport3d::FRAME_LABEL)
        .rect()
        .center();
    let btn = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Secondary,
        pressed,
        modifiers: Default::default(),
    };
    h.hover_at(c);
    h.run_steps(1);
    h.event(btn(c, true));
    h.run_steps(1);
    for k in 1..=4 {
        h.hover_at(c + egui::vec2(10.0 * k as f32, 0.0));
        h.run_steps(1);
    }
    h.event(btn(c + egui::vec2(40.0, 0.0), false));
    h.run_steps(4);
    assert_ne!(h.state().meshes.v3.orbit.yaw, yaw, "the drag orbited");
    assert!(h.query_by_label(EXPORT_THIS_NIF).is_none(), "no menu");
}

/// Review Focus 1 (Q3): a file Tessalith did not write is never overwritten by the one-click export: it opens
/// Review on that row; after Overwrite there, the next one-click writes.
#[test]
fn quick_export_never_overwrites_a_foreign_file() {
    let (_t, _d, mut h) = review_app();
    let (path, out_path, out) = nif_out(&h);
    std::fs::create_dir_all(out_path.parent().unwrap()).unwrap();
    std::fs::write(&out_path, b"theirs").unwrap();
    h.run_steps(4);
    right_click_3d(&mut h);
    assert!(
        !is_disabled(&h, EXPORT_THIS_NIF),
        "a foreign file is Review's question"
    );
    h.get_by_role_and_label(AkRole::Button, EXPORT_THIS_NIF)
        .click();
    h.run_steps(6);
    assert_eq!(
        std::fs::read(&out_path).unwrap(),
        b"theirs",
        "nothing written"
    );
    assert!(h.state().model.review.is_some(), "Review opens on the row");
    h.get_by_label(&format!("{path} was not written by Tessalith"));
    assert!(h
        .state()
        .model
        .messages
        .entries()
        .all(|e| !e.text.starts_with("NIF edits")));
    h.state_mut()
        .meshes
        .set_nif_answer(&path, GuardChoice::Overwrite, sha(b"theirs"));
    h.run_steps(2);
    right_click_3d(&mut h);
    h.get_by_role_and_label(AkRole::Button, EXPORT_THIS_NIF)
        .click();
    h.run_steps(6);
    assert_eq!(
        std::fs::read(&out_path).unwrap(),
        out,
        "the answer let it write"
    );
}

/// Final review Minor 2: with a foreign file at the path the enabled item's hover says the click opens Review.
#[test]
fn quick_export_hover_says_a_foreign_file_opens_review() {
    let (_t, _d, mut h) = review_app();
    let (_path, out_path, _out) = nif_out(&h);
    std::fs::create_dir_all(out_path.parent().unwrap()).unwrap();
    std::fs::write(&out_path, b"theirs").unwrap();
    h.run_steps(4);
    assert!(h.state().meshes.quick_export_asks());
    right_click_3d(&mut h);
    let c = h
        .get_by_role_and_label(AkRole::Button, EXPORT_THIS_NIF)
        .rect()
        .center();
    h.hover_at(c);
    h.run_steps(TOOLTIP_STEPS);
    h.get_by_label(
        "The file on disk was not written by Tessalith: opens Review to answer Keep or Overwrite",
    );
    assert!(h
        .query_by_label("Write this NIF to the export folder now, without Review")
        .is_none());
}

/// Q4: with no export folder the item is off with the reason, and `Choose export folder…` asks for the picker.
#[test]
fn quick_export_is_off_without_a_folder_and_offers_the_picker() {
    let (_d, mut m) = export_session();
    m.set_nif_export_root(None);
    m.open_edit();
    gloss(&mut m, 12.5);
    assert_eq!(m.quick_export_block().as_deref(), Some(NO_EXPORT_FOLDER));
    let mut h = view(m);
    h.run_steps(3);
    assert_eq!(h.query_all_by_label("No export folder chosen").count(), 0);
    right_click_3d(&mut h);
    assert!(is_disabled(&h, EXPORT_THIS_NIF));
    let c = h
        .get_by_role_and_label(AkRole::Button, EXPORT_THIS_NIF)
        .rect()
        .center();
    h.hover_at(c);
    h.run_steps(TOOLTIP_STEPS);
    assert!(
        h.query_all_by_label("No export folder chosen").count() >= 1,
        "the hover says why"
    );
    assert!(!h.state().pick_export_folder_request);
    h.get_by_role_and_label(AkRole::Button, "Choose export folder…")
        .click();
    h.run_steps(2);
    assert!(h.state().pick_export_folder_request);
}

/// Q2: while a texture export runs the item is off, with the reason on hover.
#[test]
fn quick_export_is_off_while_an_export_runs() {
    let (_d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    m.export_running = true;
    assert_eq!(
        m.quick_export_block().as_deref(),
        Some("An export is running")
    );
    let mut h = view(m);
    h.run_steps(3);
    right_click_3d(&mut h);
    assert!(is_disabled(&h, EXPORT_THIS_NIF));
    let c = h
        .get_by_role_and_label(AkRole::Button, EXPORT_THIS_NIF)
        .rect()
        .center();
    h.hover_at(c);
    h.run_steps(TOOLTIP_STEPS);
    h.get_by_label("An export is running");
    h.get_by_role_and_label(AkRole::Button, EXPORT_THIS_NIF)
        .click();
    h.run_steps(3);
    assert_eq!(
        h.state().quick_export_result,
        None,
        "a disabled item writes nothing"
    );
}

/// Q1: the set preview's 3D view (the same `image()`) gets no menu.
#[test]
fn the_set_preview_has_no_export_menu() {
    use sdc_gui::model::set_preview::{SetPreview, DEFAULT_CAP};
    let d = tempfile::tempdir().unwrap();
    let mut m = support::pbr_opened_plain(d.path());
    let key = sdc_gui::model::pbr_export::json_entries(&m)
        .into_iter()
        .find(|(v, _)| v.id == "landscape/rock01")
        .unwrap()
        .0
        .key;
    m.select_set(&key);
    m.settings.set_view_3d = true;
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(520.0, 1000.0))
        .build_ui_state(
            |ui, (m, s): &mut (sdc_gui::model::AppModel, SetPreview)| {
                sdc_gui::ui::set_preview::draw(ui, m, s)
            },
            (m, SetPreview::new(None, DEFAULT_CAP)),
        );
    for _ in 0..3 {
        h.run_steps(4);
        let (m, s) = h.state_mut();
        s.poll(&*m);
    }
    h.run_steps(4);
    right_click_3d(&mut h); // finds the 3D image, so the view is drawn
    assert!(h.query_by_label(EXPORT_THIS_NIF).is_none());
}

// ---- Note GH2, GH6, GH7: one Undo pair, the preview's minimum height, 1-based slot numbers ----

/// GH2 (N2): the panel has no Undo / Redo of its own; the header's pair and Ctrl+Z step the NIF history.
#[test]
fn the_edit_panel_has_no_undo_redo_of_its_own() {
    let (_t, _d, mut h) = both_histories(1);
    let edit = support::panel(&h.ctx, "meshes-edit");
    for l in ["Undo NIF edit", "Redo NIF edit"] {
        assert!(h.query_by_label(l).is_none(), "{l} is in the panel");
    }
    let in_panel = h
        .query_all_by_role(AkRole::Button)
        .filter(|n| edit.contains(n.rect().center()))
        .filter(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Undo") || l.starts_with("Redo"))
        })
        .count();
    assert_eq!(in_panel, 0, "an Undo or Redo button inside the edit panel");
    assert!(!support::header_undo(&h, "Undo")
        .accesskit_node()
        .is_disabled());
    let target = sdc_gui::ui::meshes::undo_target(&h.ctx).unwrap();
    let step = target.undo_label.as_ref().unwrap();
    let hdr = sdc_gui::ui::header::undo_button(Some(&target), true, None);
    assert_eq!(hdr.text, "Undo NIF edit");
    assert_eq!(hdr.name, format!("Undo (NIF edit history): {step}"));
    assert!(hdr.hover.contains(&format!("Undo NIF edit: {step}")));
    press(&mut h, egui::Modifiers::COMMAND, egui::Key::Z);
    assert_eq!(nif(&h), (0, true), "Ctrl+Z undid the NIF step");
}

/// GH6 (N6): the lines above the 3D frame give way, so the frame keeps 160 px, fully on screen, while the window has
/// the room (Review dragged up to 400 px at 1536x864). Taller than that the room is gone: the bottom panel's own cap is
/// in a main-team file, so past it the frame shrinks (the report's hand-off), but the lines above are already at their
/// least.
#[test]
fn the_preview_keeps_160_px_with_review_open() {
    let (w, hgt) = (1536.0, 864.0);
    app_data();
    let t = tempfile::tempdir().unwrap();
    let model = support::opened(&t.path().join("mod"));
    let (_d, mut h) = in_the_meshes_view_sized(model, 1, egui::vec2(w, hgt));
    // A tall head: the open Plugin overrides list gives the lines above the frame something to give up (Note GJ5; the
    // Meshes bar's button row used to).
    h.state_mut().meshes.plugin_uses = (0..8)
        .map(|i| modscan::plugin_store::MeshUse {
            formid: i,
            edid: Some(format!("Rock{i}")),
            sig: "STAT".into(),
            plugin: "Skyrim.esm".into(),
            model_slot: 0,
            n_overrides: 0,
            swaps: vec![],
        })
        .collect();
    h.run_steps(2);
    h.get_by_label("Plugin overrides (8)").click();
    h.run_steps(2);
    h.state_mut().model.open_review();
    h.run_steps(8);
    let mut tops = vec![];
    for review_h in [345.0, 380.0, 400.0, 440.0, 480.0] {
        let r = set_review_rect(&h, (w, hgt), review_h);
        h.run_steps(8);
        let frame = h
            .get_by_label(sdc_gui::model::viewport3d::FRAME_LABEL)
            .rect();
        if review_h <= 400.0 {
            assert!(
                frame.height() >= 159.5,
                "Review {review_h}: preview {frame:?}"
            );
            assert!(frame.max.y <= r.min.y, "{frame:?} under Review {r:?}");
        }
        tops.push(frame.min.y);
    }
    // The lines above give way first: the frame's top stops at the head's least once the room runs short.
    assert!(
        tops[0] > tops[4] + 20.0,
        "the head did not give way: {tops:?}"
    );
    assert_eq!(tops[3], tops[4], "the head is at its least: {tops:?}");
}

/// Note FI18 (1; HANDOFF 2026-11-01 owed item 1, GH3): the app hands the opened folder's output game paths to the
/// Meshes side each frame. Idle frames build nothing (the rev is unchanged); a newly added output reaches it on the
/// next frame.
#[test]
fn the_meshes_side_gets_a_new_output_path_on_the_next_frame() {
    app_data();
    let t = tempfile::tempdir().unwrap();
    let png = |p: &Path| {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        image::RgbImage::from_pixel(64, 64, image::Rgb([90, 120, 30]))
            .save(p)
            .unwrap();
    };
    let root = t.path().join("mod");
    png(&root.join("textures/xmead/ale.png"));
    let model = support::opened_as_the_app(root.clone(), t.path().join("export"));
    let mut app = sdc_gui::app::SdcApp::new(model);
    app.meshes.procs = sdc_gui::model::meshes::no_procs;
    let mut frame = eframe::Frame::_new_kittest();
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1400.0, 900.0))
        .build_ui_state(
            move |ui, app: &mut sdc_gui::app::SdcApp| eframe::App::ui(app, ui, &mut frame),
            app,
        );
    h.run_steps(4);
    let rev = h.state().model.output_game_paths().0;
    let builds = h.state().meshes.project_builds;
    h.run_steps(5);
    assert_eq!(h.state().model.output_game_paths().0, rev, "idle: same rev");
    assert_eq!(
        h.state().meshes.project_builds,
        builds,
        "idle frames build nothing"
    );
    png(&root.join("textures/xmead/mead.png"));
    h.state_mut().model.replan();
    assert!(h
        .state_mut()
        .model
        .wait_idle(std::time::Duration::from_secs(60)));
    let (rev2, paths) = h.state().model.output_game_paths();
    assert!(rev2 > rev);
    assert!(
        paths.iter().any(|p| p == "textures/xmead/mead.dds"),
        "{paths:?}"
    );
    h.step();
    assert!(
        h.state().meshes.project_builds > builds,
        "the Meshes side did not get the new paths"
    );
}

/// Note FI18 (2; HANDOFF owed item 2, GH6/N6; ruling R3): in the Meshes view with a NIF selected, the bottom panel
/// stops where the 3D frame keeps its 160 px (GH6). A Review dragged to 600 px at 1536x864 is cut down so the frame
/// keeps 160 px, fully above the panel; in the Textures view (NIF still selected) it stays 600.
#[test]
fn the_bottom_panel_is_capped_only_in_the_meshes_view_with_a_nif() {
    let (w, hgt) = (1536.0, 864.0);
    app_data();
    let t = tempfile::tempdir().unwrap();
    let model = support::opened(&t.path().join("mod"));
    let (_d, mut h) = in_the_meshes_view_sized(model, 1, egui::vec2(w, hgt));
    assert!(h.state().meshes.selected_nif.is_some());
    h.state_mut().model.open_review();
    h.run_steps(8);
    let frame = h
        .get_by_label(sdc_gui::model::viewport3d::FRAME_LABEL)
        .rect();
    assert!(frame.height() >= 159.5, "40 % Review: preview {frame:?}");
    let tall = |h: &mut AppH| {
        set_review_rect(h, (w, hgt), 600.0);
        h.run_steps(8);
    };
    tall(&mut h);
    let capped = review_height(&h);
    assert!(capped < 600.0, "Meshes view: Review {capped}");
    let frame = h
        .get_by_label(sdc_gui::model::viewport3d::FRAME_LABEL)
        .rect();
    let top = hgt - capped;
    assert!(frame.height() >= 159.5, "600 px Review: preview {frame:?}");
    assert!(frame.max.y <= top + 0.5, "{frame:?} under Review at {top}");
    // The Textures view, the NIF still selected: the panel is not capped.
    sdc_gui::ui::header::set_meshes_view(&h.ctx, false);
    tall(&mut h);
    assert!(h.state().meshes.selected_nif.is_some());
    assert_eq!(
        review_height(&h),
        600.0,
        "the Textures view's Review is uncapped"
    );
}

/// UI-D final review Minor 7: `NIF_ROOM`'s 211 px was measured at 1536x864; at 1280x720 a capped Review still leaves
/// the 3D frame its 160 px, fully above the panel.
#[test]
fn the_meshes_cap_keeps_160_px_at_1280x720() {
    let (w, hgt) = (1280.0, 720.0);
    app_data();
    let t = tempfile::tempdir().unwrap();
    let model = support::opened(&t.path().join("mod"));
    let (_d, mut h) = in_the_meshes_view_sized(model, 1, egui::vec2(w, hgt));
    h.state_mut().model.open_review();
    h.run_steps(8);
    set_review_rect(&h, (w, hgt), 600.0);
    h.run_steps(8);
    let capped = review_height(&h);
    assert!(capped < 600.0, "Review {capped}");
    let frame = h
        .get_by_label(sdc_gui::model::viewport3d::FRAME_LABEL)
        .rect();
    assert!(frame.height() >= 159.5, "preview {frame:?}");
    assert!(frame.max.y <= hgt - capped + 0.5, "{frame:?} under Review");
}

/// The app in the Meshes view at `size`, the lit shape selected, five pending edits and the pending list open, and the
/// bottom panel on `tab` (Review opens at its default height).
fn slot_list_session(
    size: egui::Vec2,
    tab: sdc_gui::model::bottom::BottomTab,
) -> (tempfile::TempDir, AppH) {
    use sdc_gui::model::bottom::BottomTab;
    app_data();
    let t = tempfile::tempdir().unwrap();
    let model = support::opened(&t.path().join("mod"));
    // Built tall, so the head has room for the click on its header; the window shrinks to `size` after it.
    let (d, mut h) = in_the_meshes_view_sized(model, 5, egui::vec2(size.x, size.y.max(900.0)));
    let (shape, _) = lit_shape(h.state().meshes.edit.as_ref().unwrap());
    h.state_mut().meshes.select_edit(Selection::Shape(shape));
    h.run_steps(4);
    // The pending list is opened while the head still has room (no bottom panel yet): once Review is open the head is
    // cut to 48 px and its clipped header lies under the tab strip, where a click would hit a tab.
    let head = sdc_gui::ui::meshes::layout_rects(&h.ctx).unwrap().top;
    let header = h.get_by_label("Pending edits (5)").rect();
    assert!(
        head.contains_rect(header),
        "the pending header {header:?} is not reachable in the head {head:?}: the replay cannot open it"
    );
    h.get_by_label("Pending edits (5)").click();
    h.run_steps(8);
    assert!(
        h.query_by_label_contains("Revert 1:").is_some(),
        "no pending row is shown after the click on the header"
    );
    h.set_size(size);
    if tab == BottomTab::Review {
        h.state_mut().model.open_review();
    } else {
        h.state_mut().model.show_tab(tab);
    }
    h.run_steps(8);
    let header = h.get_by_label("Pending edits (5)");
    assert_eq!(
        header.accesskit_node().data().is_expanded(),
        Some(true),
        "the pending list is not expanded in the replay"
    );
    assert!(
        h.query_by_label_contains("Revert 1:").is_some(),
        "no pending row after the window shrank"
    );
    (d, h)
}

/// Every slot field whose rect reaches below `limit` is clipped there (its `interact_rect` ends at `limit`, or is
/// empty); the replay must really have such a field.
/// The list is scrolled in 9 px steps until a field straddles the boundary (a field wholly under it has no node).
fn assert_slot_fields_clipped_below(h: &mut AppH, limit: f32) {
    let list = sdc_gui::ui::meshes::layout_rects(&h.ctx).unwrap().tab_area;
    let crossing_fields = |h: &AppH| -> Vec<_> {
        h.query_all_by_label_contains("path (slot ")
            .filter(|n| n.accesskit_node().role() == AkRole::TextInput)
            .map(|n| (n.accesskit_node().id(), n.rect()))
            .filter(|(_, r)| r.max.y > limit)
            .collect()
    };
    let mut crossing = crossing_fields(h);
    h.event(egui::Event::PointerMoved(list.center()));
    for _ in 0..30 {
        if !crossing.is_empty() {
            break;
        }
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -9.0),
            source: egui::MouseWheelSource::Wheel,
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
        });
        h.run_steps(2);
        crossing = crossing_fields(h);
    }
    assert!(
        !crossing.is_empty(),
        "no slot field crosses or lies below the footer's top at {limit}"
    );
    for (node, rect) in crossing {
        let clipped = h.ctx.viewport(|v| {
            v.this_pass
                .widgets
                .layers()
                .flat_map(|(_, ws)| ws)
                .find(|w| w.id.accesskit_id().0 == (u128::from(node) >> 64) as u64)
                .map(|w| w.interact_rect)
        });
        let clipped = clipped.unwrap_or_else(|| panic!("no widget for the field at {rect:?}"));
        // Partly hidden: cut at the area's edge; wholly hidden: an empty (even negative) intersection.
        assert!(
            clipped.max.y <= limit + 0.5 || clipped.height() <= 0.0,
            "a slot field {rect:?} reaches below the footer at {limit} unclipped: {clipped:?}"
        );
    }
}

/// Review replay 109: with Review open at its default height at 1280x720 the slot list scrolls in its own area, which
/// ends above the footer; it no longer runs under it.
#[test]
fn with_review_open_the_slot_list_ends_above_the_footer_at_1280x720() {
    use sdc_gui::model::bottom::BottomTab;
    let (_d, mut h) = slot_list_session(egui::vec2(1280.0, 720.0), BottomTab::Review);
    let r = sdc_gui::ui::meshes::layout_rects(&h.ctx).unwrap();
    assert!(r.tab_area.max.y <= r.footer.min.y + 0.5, "{r:?}");
    assert_slot_fields_clipped_below(&mut h, r.footer.min.y);
}

/// Review replay 109: with the Problems panel open at 1536x864 the slot list keeps the 200 px of `TAB_MIN_H`.
#[test]
fn with_problems_open_the_slot_list_keeps_200_px_at_1536x864() {
    use sdc_gui::model::bottom::BottomTab;
    let (_d, mut h) = slot_list_session(egui::vec2(1536.0, 864.0), BottomTab::Problems);
    let r = sdc_gui::ui::meshes::layout_rects(&h.ctx).unwrap();
    assert!(r.tab_area.height() >= 200.0 - 0.5, "{r:?}");
    assert!(r.tab_area.max.y <= r.footer.min.y + 0.5, "{r:?}");
    assert_slot_fields_clipped_below(&mut h, r.footer.min.y);
}

/// Fix round 1, Important 1: egui applies a scroll area's scrolled minimum (64 by default) after its maximum, so the
/// head's floor is `TOP_MIN_H` = 48 only when it is set. At 1280x720 with Review open the head is at its floor, and
/// the slot list gets what the 48 leaves (the default 64 would take 16 px more).
#[test]
fn the_head_keeps_its_own_48_px_floor_at_1280x720() {
    use sdc_gui::model::bottom::BottomTab;
    let (_d, h) = slot_list_session(egui::vec2(1280.0, 720.0), BottomTab::Review);
    let r = sdc_gui::ui::meshes::layout_rects(&h.ctx).unwrap();
    assert!(r.top.height() <= 48.0 + 0.5, "head consumed {r:?}");
    assert!(r.top.max.y <= r.tab_area.min.y, "head over the tabs: {r:?}");
    // 254 above the footer's top: the 48 head, its spacing, the strip, separators and editing line (about 60 and 3),
    // leaves the rest to the slot list.
    let room = r.footer.min.y - r.top.min.y;
    let below_head = r.tab_area.height() + (r.tab_area.min.y - r.top.max.y);
    assert!(
        (room - r.top.height() - below_head).abs() <= 0.5,
        "{r:?}: the panel's room {room} is not head + tabs + slot list"
    );
    assert!(r.tab_area.height() >= 138.0, "slot list {r:?}");
}

/// Fix round 1, Important 2: a slot list wide enough for a horizontal bar (the edit panel dragged to 300 px) keeps
/// its whole OUTER rect, the bar included, above the footer. The inner viewport alone would end 6 px lower.
#[test]
fn a_horizontal_bar_stays_above_the_footer_at_1280x720() {
    use sdc_gui::model::bottom::BottomTab;
    let (_d, mut h) = slot_list_session(egui::vec2(1280.0, 720.0), BottomTab::Review);
    sdc_gui::ui::panel_ids::restore_size(&h.ctx, "meshes-edit", 300.0);
    h.run_steps(6);
    let r = sdc_gui::ui::meshes::layout_rects(&h.ctx).unwrap();
    let bars: Vec<_> = h
        .query_all_by_role(AkRole::ScrollBar)
        .map(|n| n.rect())
        .filter(|b| b.width() > b.height() && r.tab_area.contains_rect(b.shrink(0.5)))
        .collect();
    assert_eq!(bars.len(), 1, "the slot list shows a horizontal bar: {r:?}");
    assert!(
        bars[0].max.y <= r.footer.min.y + 0.5,
        "bar {:?} under {r:?}",
        bars[0]
    );
    assert!(r.tab_area.max.y <= r.footer.min.y + 0.5, "outer rect {r:?}");
    // Task 10 (E6-Y4): both footer buttons stay whole inside the footer at this width
    for name in ["Review…", "Show in Explorer: the draft"] {
        let b = h.get_by_label(name).rect();
        assert!(r.footer.contains_rect(b), "{name} {b:?} outside {r:?}");
    }
}

/// Fix round 1, Important 3: an open Diff (a 160 px list) at 1280x720 with Review open would push the tab strip and
/// the slot list into the footer; its list gives way instead, down to 40 px, and the editor keeps its floors.
#[test]
fn an_open_diff_gives_way_before_the_editor_loses_its_floors_at_1280x720() {
    use sdc_gui::model::bottom::BottomTab;
    let (_d, mut h) = slot_list_session(egui::vec2(1280.0, 720.0), BottomTab::Review);
    let path = h.state().meshes.edit.as_ref().unwrap().data_path.clone();
    let lines: Vec<String> = (0..20).map(|i| format!("block {i}: changed")).collect();
    h.state_mut().meshes.nif_diff_view = Some((path, lines));
    h.run_steps(8);
    let r = sdc_gui::ui::meshes::layout_rects(&h.ctx).unwrap();
    assert!(
        r.top.max.y <= r.tab_area.min.y + 0.5,
        "head over the slot list: {r:?}"
    );
    assert!(
        r.tab_area.max.y <= r.footer.min.y + 0.5,
        "slot list under the footer: {r:?}"
    );
    assert!(r.tab_area.height() >= 48.0 - 0.5, "slot list {r:?}");
    for tab in [
        "Shader tab",
        "Alpha tab",
        "Controllers tab",
        "Extra data tab",
    ] {
        let b = h.get_by_label(tab).rect();
        assert!(b.max.y <= r.footer.min.y + 0.5, "{tab} {b:?} under {r:?}");
        assert!(
            b.min.y >= r.top.max.y - 0.5,
            "{tab} {b:?} inside the head {r:?}"
        );
    }
    // The Diff list scrolls in its own bar: 40 to 160 px, strictly under its 160 px cap here.
    let bars: Vec<_> = h
        .query_all_by_role(AkRole::ScrollBar)
        .map(|n| n.rect())
        .filter(|b| b.height() > b.width() && r.footer.contains_rect(b.shrink(0.5)))
        .collect();
    assert_eq!(bars.len(), 1, "the Diff list's bar: {bars:?} in {r:?}");
    let list = bars[0].height();
    assert!(
        (40.0 - 0.5..160.0 - 0.5).contains(&list),
        "Diff list {list} px"
    );
}

/// The interact rect (clipped) of the widget behind an AccessKit node, from the last drawn pass.
fn interact_rect_of(h: &AppH, node: egui::accesskit::NodeId) -> Option<egui::Rect> {
    h.ctx.viewport(|v| {
        v.this_pass
            .widgets
            .layers()
            .flat_map(|(_, ws)| ws)
            .find(|w| w.id.accesskit_id() == node)
            .map(|w| w.interact_rect)
    })
}

/// Fix round 2, Important (new): below the height where the editor's floors fit above the footer, the footer stays
/// whole and clickable and nothing of the editor (head, tab strip) interacts inside it. Reserving a bottom panel only
/// moves the parent's cursor boundary; the editor above is clipped to the rect over the footer.
#[test]
fn under_the_limit_no_editor_widget_interacts_inside_the_footer() {
    use sdc_gui::model::bottom::BottomTab;
    let mut crossed = 0;
    for hgt in [480.0, 440.0, 400.0, 360.0] {
        let (_d, mut h) = slot_list_session(egui::vec2(1280.0, hgt), BottomTab::Review);
        let path = h.state().meshes.edit.as_ref().unwrap().data_path.clone();
        let lines: Vec<String> = (0..20).map(|i| format!("block {i}: changed")).collect();
        h.state_mut().meshes.nif_diff_view = Some((path, lines));
        h.run_steps(8);
        let r = sdc_gui::ui::meshes::layout_rects(&h.ctx).unwrap();
        // The panel is too short for the floors: the strip (about 24 px) does not fit above the footer.
        if r.footer.min.y > r.top.max.y + 24.0 {
            continue;
        }
        // Every expected widget must be found (a missing node fails, naming it). A widget that is clipped has an
        // interact rect that ends at the footer's top or is empty; one with none counts as contained only when its
        // layout rect lies wholly under the footer's top.
        for name in [
            "Shader tab",
            "Alpha tab",
            "Controllers tab",
            "Extra data tab",
            "Show only this shape",
            "Pending edits (5)",
            "Revert 1:",
        ] {
            let n = h
                .query_all_by_label_contains(name)
                .next()
                .unwrap_or_else(|| panic!("{hgt}: no node for {name}: {r:?}"));
            let layout = n.rect();
            let id = n.accesskit_node().id();
            let id = egui::accesskit::NodeId((u128::from(id) >> 64) as u64);
            match interact_rect_of(&h, id) {
                Some(i) => assert!(
                    i.max.y <= r.footer.min.y + 0.5 || i.height() <= 0.0,
                    "{hgt}: {name} interacts under the footer: {i:?}, {r:?}"
                ),
                None => assert!(
                    layout.min.y >= r.footer.min.y - 0.5,
                    "{hgt}: {name} at {layout:?} has no interact rect and is not under the footer: {r:?}"
                ),
            }
        }
        crossed += 1;
        // The footer is whole: its Close button still takes the click.
        h.get_by_label("Close the diff").click();
        h.run_steps(4);
        assert!(
            h.state().meshes.nif_diff_view.is_none(),
            "{hgt}: the footer's Close took no click"
        );
    }
    assert!(crossed >= 1, "no window height reached the limit");
}

/// Fix round 2, Important 3: egui's 64 px scrolled minimum applies to the Diff list too, so a cap between 40 and 64
/// px was not obeyed (a cap of 60 gave 64 and the slot list fell to 44). At 1280x560 with Review open the panel leaves
/// 312 px of room (323 at 720 and above): the list is its cap, under 64, and the slot list keeps its 48.
#[test]
fn a_diff_cap_under_64_px_is_obeyed_at_1280x560() {
    use sdc_gui::model::bottom::BottomTab;
    let (_d, mut h) = slot_list_session(egui::vec2(1280.0, 560.0), BottomTab::Review);
    let path = h.state().meshes.edit.as_ref().unwrap().data_path.clone();
    let lines: Vec<String> = (0..20).map(|i| format!("block {i}: changed")).collect();
    h.state_mut().meshes.nif_diff_view = Some((path, lines));
    h.run_steps(8);
    let r = sdc_gui::ui::meshes::layout_rects(&h.ctx).unwrap();
    assert!(r.tab_area.height() >= 48.0 - 0.5, "slot list {r:?}");
    assert!(
        r.tab_area.max.y <= r.footer.min.y + 0.5,
        "slot list under the footer: {r:?}"
    );
    let bars: Vec<_> = h
        .query_all_by_role(AkRole::ScrollBar)
        .map(|n| n.rect())
        .filter(|b| b.height() > b.width() && r.footer.contains_rect(b.shrink(0.5)))
        .collect();
    assert_eq!(bars.len(), 1, "the Diff list's bar: {bars:?} in {r:?}");
    // The list is its cap: the panel's room is 312 px here (footer bottom 430 - head top 118), less the editor's floors
    // (48 head + 3 spacing + 60 strip + 48 slot list) and the 93 px the footer takes besides its list: 60 px.
    let list = bars[0].height();
    assert!(
        (list - 60.0).abs() <= 0.5,
        "Diff list {list} px, its cap is 60 here: {r:?}"
    );
}

/// Fix round 1: after one pass of a first frame (the cached tab height still its first-frame 28 px) no part of the
/// editor reaches below the footer's top, and the head is over the slot list, not on it.
#[test]
fn the_first_pass_puts_no_part_of_the_editor_under_the_footer() {
    app_data();
    let t = tempfile::tempdir().unwrap();
    let model = support::opened(&t.path().join("mod"));
    let (_d, mut h) = in_the_meshes_view_steps(model, 5, egui::vec2(1280.0, 720.0));
    let (shape, _) = lit_shape(h.state().meshes.edit.as_ref().unwrap());
    h.state_mut().meshes.select_edit(Selection::Shape(shape));
    h.state_mut().model.open_review();
    // A frame can run more than one pass, and a later pass would have measured the strip: allow one.
    h.ctx
        .options_mut(|o| o.max_passes = std::num::NonZeroUsize::new(1).unwrap());
    h.step();
    let r =
        sdc_gui::ui::meshes::layout_rects(&h.ctx).expect("the edit panel drew in the first pass");
    // The rects are those of a pass that split the room with the 28 px estimate.
    assert_eq!(r.tabs_h, 28.0, "not the first pass of the panel: {r:?}");
    assert!(
        r.top.max.y <= r.tab_area.min.y + 0.5,
        "head over the slot list: {r:?}"
    );
    assert!(
        r.tab_area.max.y <= r.footer.min.y + 0.5,
        "slot list under the footer: {r:?}"
    );
    assert!(r.tab_area.height() > 0.0, "{r:?}");
}

fn review_height(h: &AppH) -> f32 {
    support::panel(&h.ctx, "bottom-review").height()
}

/// Writes Review's panel rect, `review_h` px tall at the foot of a `w` x `hgt` window, as a drag of its edge would;
/// returns that rect.
fn set_review_rect(h: &AppH, (w, hgt): (f32, f32), review_h: f32) -> egui::Rect {
    use egui::containers::panel::PanelState;
    let r = egui::Rect::from_min_size(
        egui::pos2(8.0, hgt - review_h),
        egui::vec2(w - 16.0, review_h),
    );
    let review_id = sdc_gui::ui::panel_ids::panel_id(&h.ctx, "bottom-review")
        .expect("the Review panel was built");
    h.ctx
        .data_mut(|d| d.insert_persisted(review_id, PanelState { outer_rect: r }));
    r
}

/// Note FI18 (2): Review's 40 % default obeys the cap too. At 1536x600 the default (240) is over it.
#[test]
fn reviews_default_height_obeys_the_meshes_cap() {
    app_data();
    let t = tempfile::tempdir().unwrap();
    let model = support::opened(&t.path().join("mod"));
    let (_d, mut h) = in_the_meshes_view_sized(model, 1, egui::vec2(1536.0, 600.0));
    h.state_mut().model.open_review();
    h.run_steps(8);
    let r = review_height(&h);
    assert!((120.0..239.0).contains(&r), "Review opened at {r}");
}

/// Note FI18 (2; review minor 1): the cap is the Meshes view's limit, not the user's height. A Review set to 600 px in
/// the Textures view is capped for frames in the Meshes view and is 600 px again back in the Textures view (the
/// panel state is written once, as a drag would, and not re-injected).
#[test]
fn the_meshes_cap_keeps_the_users_review_height() {
    let (w, hgt) = (1536.0, 864.0);
    app_data();
    let t = tempfile::tempdir().unwrap();
    let model = support::opened(&t.path().join("mod"));
    let (_d, mut h) = in_the_meshes_view_sized(model, 1, egui::vec2(w, hgt));
    h.state_mut().model.open_review();
    sdc_gui::ui::header::set_meshes_view(&h.ctx, false);
    h.run_steps(4);
    set_review_rect(&h, (w, hgt), 600.0);
    h.run_steps(4);
    assert_eq!(review_height(&h), 600.0, "Textures view: Review");
    sdc_gui::ui::header::set_meshes_view(&h.ctx, true);
    h.run_steps(4);
    assert!(review_height(&h) < 600.0, "Meshes view: capped");
    h.get_by_label("Settings…").click();
    h.run_steps(4);
    h.get_by_label("Back to meshes");
    assert_eq!(review_height(&h), 600.0, "Settings covers the NIF preview");
    h.get_by_label("Back to meshes").click();
    h.run_steps(4);
    assert!(
        review_height(&h) < 600.0,
        "Back restores the NIF preview cap"
    );
    sdc_gui::ui::header::set_meshes_view(&h.ctx, false);
    h.run_steps(4);
    assert_eq!(review_height(&h), 600.0, "back in the Textures view");
}

/// UI-D final review Minor 8: a drag of Review's edge in the Meshes view that ends at the cap is the user's height
/// too; the Textures view keeps it and does not restore the older 600 px.
#[test]
fn a_drag_to_the_cap_is_the_users_height() {
    let (w, hgt) = (1536.0, 864.0);
    app_data();
    let t = tempfile::tempdir().unwrap();
    let model = support::opened(&t.path().join("mod"));
    let (_d, mut h) = in_the_meshes_view_sized(model, 1, egui::vec2(w, hgt));
    h.state_mut().model.open_review();
    sdc_gui::ui::header::set_meshes_view(&h.ctx, false);
    h.run_steps(4);
    set_review_rect(&h, (w, hgt), 600.0);
    h.run_steps(4);
    sdc_gui::ui::header::set_meshes_view(&h.ctx, true);
    h.run_steps(4);
    let capped = review_height(&h);
    assert!(capped < 600.0, "Meshes view: capped");
    // Drag the top edge up: the panel stays at the cap.
    let edge = egui::pos2(w / 2.0, hgt - capped);
    h.event(egui::Event::PointerMoved(edge));
    h.run_steps(2);
    h.event(egui::Event::PointerButton {
        pos: edge,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.run_steps(1);
    for dy in [10.0, 20.0, 30.0] {
        h.event(egui::Event::PointerMoved(edge - egui::vec2(0.0, dy)));
        h.run_steps(1);
    }
    h.event(egui::Event::PointerButton {
        pos: edge - egui::vec2(0.0, 30.0),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run_steps(4);
    assert!((review_height(&h) - capped).abs() < 1.0, "still at the cap");
    sdc_gui::ui::header::set_meshes_view(&h.ctx, false);
    h.run_steps(4);
    assert!(
        (review_height(&h) - capped).abs() < 1.0,
        "the Textures view restored {} over the user's {capped}",
        review_height(&h)
    );
}

/// GH7 (N7): the shader form numbers slots from 1, and the NIF's own index is on hover.
#[test]
fn texture_slots_are_numbered_from_one_with_the_nif_index_on_hover() {
    let mut m = direct(scenes::shared_alpha_and_set());
    select(&mut m, "A");
    let mut h = view(m);
    h.run_steps(2);
    h.get_by_role_and_label(AkRole::TextInput, "Diffuse path (slot 1)");
    h.get_by_role_and_label(AkRole::Button, "Browse for Diffuse (slot 1)");
    assert!(h.query_by_label_contains("(slot 0)").is_none());
    let edit = support::panel(&h.ctx, "meshes-edit");
    let name = h
        .query_all_by_label("Diffuse")
        .find(|n| edit.contains(n.rect().center()))
        .expect("the slot's name in the form");
    name.hover();
    h.run_steps(30);
    h.get_by_label_contains("NIF texture index 0");
}

/// GH7: the form shows the set editor's name for a True PBR slot, and keeps its own where the set editor has none.
#[test]
fn true_pbr_slots_use_the_set_editors_names() {
    const PBR: u32 = 1 << 23;
    assert_eq!(edit_slot_shown(0, 0, PBR, 2), "Emissive / Skin");
    assert_eq!(edit_slot_shown(0, 0, PBR, 6), "Coat normal / Fuzz");
    assert_eq!(edit_slot_shown(0, 0, PBR, 5), "RMAOS");
    // No set editor label outside True PBR: the form's own names stay.
    assert_eq!(edit_slot_shown(0, 0, 0, 2), "Rim / back-light");
    assert_eq!(edit_slot_shown(1, 0, 0, 4), "Env cube");
}

/// GH2 (N2, the brief's test): Ctrl+Z after Add alpha takes the alpha op back out (the header's step is named for it).
#[test]
fn ctrl_z_undoes_add_alpha() {
    let (_t, _d, mut h) = both_histories(0);
    let m = &mut h.state_mut().meshes;
    let e = m.edit.as_ref().unwrap();
    let (shape, _) = lit_shape(e);
    assert!(
        e.model.shape(shape).unwrap().alpha.is_none(),
        "the fixture's lit shape has no alpha"
    );
    let at = nif::edit::anchor_shape(&e.model, shape);
    m.edit_push(
        vec![Op::AddAlpha {
            at,
            flags: 0x00ED,
            threshold: 128,
        }],
        None,
    );
    m.edit.as_mut().unwrap().flush();
    h.run_steps(4);
    let add_alpha = |h: &AppH| {
        let e = h.state().meshes.edit.as_ref().unwrap();
        e.doc
            .ops
            .iter()
            .filter(|d| matches!(d.op, Op::AddAlpha { .. }))
            .count()
    };
    assert_eq!(add_alpha(&h), 1);
    assert_eq!(
        h.state().meshes.edit.as_ref().unwrap().history.undo_label(),
        Some("Add alpha")
    );
    press(&mut h, egui::Modifiers::COMMAND, egui::Key::Z);
    assert_eq!(add_alpha(&h), 0, "Ctrl+Z took the Add alpha back");
}

/// GH7: a pending texture row ("Texture slot 2") gives the NIF's own index on hover.
#[test]
fn a_pending_texture_row_gives_the_nif_index_on_hover() {
    let (_d, mut m) = export_session();
    m.open_edit();
    let e = m.edit.as_ref().unwrap();
    let (shape, shader) = lit_shape(e);
    let set = e.model.lighting[&shader].texture_set.unwrap();
    let base = String::from_utf8_lossy(&e.model.texsets[&set].raw.slots[1]).into_owned();
    let on_shape = nif::edit::anchor_shape(&e.model, shape);
    // A path outside Windows-1252 is refused: a red row, so the pending list is open.
    m.edit_push(
        vec![Op::SetTexturePath {
            at: on_shape,
            slot: 1,
            path: "textures\\\u{4e2d}.dds".into(),
            base,
        }],
        None,
    );
    let mut h = view(m);
    h.run_steps(3);
    h.get_by_label_contains(": Texture slot 2: ").hover();
    h.run_steps(30);
    h.get_by_label("NIF texture index 1");
}

// ---- Review E6: the footer names the draft, with Show in Explorer ----

#[test]
fn the_footer_names_the_draft_with_show_in_explorer() {
    app_data();
    let t = tempfile::tempdir().unwrap();
    // the project is the opened folder's textures folder
    let p = t.path().join("mod").join("textures");
    let model = support::opened(&t.path().join("mod"));
    let (_d, mut h) = in_the_meshes_view(model, 1);
    h.get_by_label_contains(&format!("Draft saved in {}", draft(&p).display()));
    h.state_mut().shell_log = Some(vec![]);
    h.get_by_label("Show in Explorer: the draft").click();
    h.run_steps(4);
    assert_eq!(
        h.state().shell_log,
        Some(vec![sdc_gui::model::shell::ShellRequest::Reveal(draft(&p))])
    );
}

/// The app with no project open and one NIF edit open (a plain autosave holds it).
fn app_with_no_project() -> (tempfile::TempDir, AppH) {
    app_data();
    let d = files(false);
    let mut model = sdc_gui::model::AppModel::new(
        sdc_gui::settings::GuiSettings::default(),
        None,
        support::session(),
    );
    model.set_export_folder(d.path().join("mods").join(EDIT_MOD));
    let mut app = sdc_gui::app::SdcApp::new(model);
    app.meshes.procs = sdc_gui::model::meshes::no_procs;
    app.meshes.store_path = Some(d.path().join("scan.db"));
    scan_and_select(&mut app.meshes, d.path());
    app.meshes.open_edit();
    gloss(&mut app.meshes, 12.5);
    assert_eq!(app.meshes.error, None);
    let mut frame = eframe::Frame::_new_kittest();
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1400.0, 900.0))
        .build_ui_state(
            move |ui, app: &mut sdc_gui::app::SdcApp| eframe::App::ui(app, ui, &mut frame),
            app,
        );
    sdc_gui::ui::header::set_meshes_view(&h.ctx, true);
    h.run_steps(4);
    (d, h)
}

#[test]
fn with_no_project_the_footer_names_the_autosave() {
    let (d, mut h) = app_with_no_project();
    assert!(autosave(&d).is_file(), "the plain autosave is written");
    h.get_by_label_contains(&format!("Draft saved in {}", autosave(&d).display()));
    h.state_mut().shell_log = Some(vec![]);
    h.get_by_label("Show in Explorer: the draft").click();
    h.run_steps(4);
    assert_eq!(
        h.state().shell_log,
        Some(vec![sdc_gui::model::shell::ShellRequest::Reveal(autosave(
            &d
        ))])
    );
}

/// Ruling E6-Y2: the saved line's text is built once for one path, width and font, not on every frame; another path
/// builds it again.
#[test]
fn the_saved_line_builds_its_text_once_for_one_path_and_width() {
    let (_d, mut h) = app_with_no_project();
    let first = sdc_gui::ui::meshes::draft_text_builds();
    assert!(first >= 1, "the line was built");
    h.run_steps(10);
    assert_eq!(
        sdc_gui::ui::meshes::draft_text_builds(),
        first,
        "no rebuild"
    );
    // another width cuts the line again, the same path does not rebuild its full text
    sdc_gui::ui::panel_ids::restore_size(&h.ctx, "meshes-edit", 300.0);
    h.run_steps(6);
    assert!(
        sdc_gui::ui::meshes::draft_text_builds() > first,
        "a new width"
    );
}

/// Ruling E6-Y9: A is saved, its draft cannot be replaced (read only), and an edit to B fails to save: the line does not
/// name the draft, which holds A, not B; undoing back to A names it again with no write.
#[test]
fn an_unsaved_edit_hides_the_draft_and_an_undo_to_the_written_ops_names_it() {
    let (_d, mut m, p) = session_in_project("p", None);
    m.open_edit();
    gloss(&mut m, 12.5);
    let before = std::fs::read(draft(&p)).unwrap();
    read_only(&draft(&p), true);
    gloss(&mut m, 13.5);
    assert_eq!(
        std::fs::read(draft(&p)).unwrap(),
        before,
        "the draft still holds A"
    );
    assert_eq!(
        m.edit.as_ref().unwrap().draft_off_reason(),
        Some("The last save failed. Make another edit to try again.")
    );
    let mut h = view(m);
    h.run_steps(4);
    h.get_by_label_contains("Draft not saved yet");
    assert!(h
        .get_by_label("Show in Explorer: the draft")
        .accesskit_node()
        .is_disabled());
    assert!(h.state_mut().edit.as_mut().unwrap().undo());
    h.run_steps(4);
    read_only(&draft(&p), false);
    h.get_by_label_contains(&format!("Draft saved in {}", draft(&p).display()));
    assert!(!h
        .get_by_label("Show in Explorer: the draft")
        .accesskit_node()
        .is_disabled());
    assert_eq!(h.state().edit.as_ref().unwrap().draft_off_reason(), None);
}

/// Ruling E6-Y8: the disabled button's hover follows the state.
#[test]
fn the_disabled_draft_button_says_why() {
    // a fresh session
    let (_d, mut m) = export_session();
    m.open_edit();
    assert_eq!(
        m.edit.as_ref().unwrap().draft_off_reason(),
        Some("The draft is saved with your first edit.")
    );
    // a drag that waits for its save
    tick(&mut m, 40.5);
    assert_eq!(
        m.edit.as_ref().unwrap().draft_off_reason(),
        Some("Finish the edit to save the draft.")
    );
    // no draft location
    let m = direct(fixture("TestNifFile_Animated_LE.nif"));
    assert_eq!(
        m.edit.as_ref().unwrap().draft_off_reason(),
        Some("This edit has no draft location.")
    );
}

/// Ruling E6-Y8: the sentence is the button's hover on screen.
#[test]
fn the_disabled_draft_button_shows_its_reason_on_hover() {
    let (_d, mut m) = export_session();
    m.open_edit();
    let mut h = view(m);
    h.run_steps(4);
    h.get_by_label("Show in Explorer: the draft").hover();
    h.run_steps(30);
    h.get_by_label("The draft is saved with your first edit.");
}

/// A session in project `p` whose draft cannot be written (`_sdc` is a file), with one edit that failed to save.
fn session_with_a_failed_first_save() -> (tempfile::TempDir, MeshesModel) {
    let (d, mut m) = export_session();
    let p = project_folder(&d, "p");
    std::fs::write(p.join("_sdc"), b"a file where the folder goes").unwrap();
    m.set_project_root(Some(&p));
    m.open_edit();
    gloss(&mut m, 12.5);
    assert_eq!(
        m.edit.as_ref().unwrap().draft_off_reason(),
        Some("The last save failed. Make another edit to try again.")
    );
    (d, m)
}

/// The footer's hover text of the disabled Show in Explorer, on screen.
fn hover_reason(h: &mut Harness<'static, MeshesModel>, text: &str) {
    h.run_steps(4);
    h.get_by_label("Show in Explorer: the draft").hover();
    h.run_steps(30);
    h.get_by_label(text);
}

/// Ruling E6-Y8, on screen: a failed save.
#[test]
fn the_failed_save_hover_is_on_screen() {
    let (_d, m) = session_with_a_failed_first_save();
    hover_reason(
        &mut view(m),
        "The last save failed. Make another edit to try again.",
    );
}

/// Ruling E6-Y10: after a failed save the next drag has made the edit the hover asked for.
#[test]
fn a_drag_after_a_failed_save_says_finish_the_edit() {
    let (_d, mut m) = session_with_a_failed_first_save();
    tick(&mut m, 40.5);
    hover_reason(&mut view(m), "Finish the edit to save the draft.");
}

/// Ruling E6-Y8, on screen: a session with no draft location.
#[test]
fn the_no_location_hover_is_on_screen() {
    let m = direct(fixture("TestNifFile_Animated_LE.nif"));
    hover_reason(&mut view(m), "This edit has no draft location.");
}

/// Ruling E6-Y10: A is saved, the save of B fails, and a drag to C is a pending save that has not failed: the line
/// keeps naming the draft, which holds A.
#[test]
fn a_drag_after_a_failed_save_keeps_naming_the_draft() {
    let (_d, mut m, p) = session_in_project("p", None);
    m.open_edit();
    gloss(&mut m, 12.5);
    read_only(&draft(&p), true);
    gloss(&mut m, 13.5);
    assert_eq!(m.edit.as_ref().unwrap().draft_on_disk(), None, "B failed");
    tick(&mut m, 40.5); // a drag to C, its save waits for the release
    let mut h = view(m);
    h.run_steps(4);
    h.get_by_label_contains(&format!("Draft saved in {}", draft(&p).display()));
    assert!(!h
        .get_by_label("Show in Explorer: the draft")
        .accesskit_node()
        .is_disabled());
    read_only(&draft(&p), false);
}

/// Ruling E6-Y10: A saved, the replacement denied, undo to no ops (that save fails), then a drag back to A: between
/// that drag's tick and its release the line names the draft.
#[test]
fn a_drag_back_to_the_written_ops_names_the_draft_before_the_release() {
    let (_d, mut m, p) = session_in_project("p", None);
    m.open_edit();
    tick(&mut m, 40.5);
    m.edit.as_mut().unwrap().flush();
    assert_eq!(op_values(&m).len(), 1);
    read_only(&draft(&p), true);
    assert!(m.edit.as_mut().unwrap().undo());
    assert_eq!(
        m.edit.as_ref().unwrap().draft_on_disk(),
        None,
        "no ops differ from A"
    );
    tick(&mut m, 40.5); // the same op again, not released yet
    let mut h = view(m);
    h.run_steps(4);
    h.get_by_label_contains(&format!("Draft saved in {}", draft(&p).display()));
    read_only(&draft(&p), false);
}

/// Ruling E6-Y1: an edit whose save failed is not on disk, so the line does not name the older draft.
#[test]
fn an_edit_whose_save_fails_does_not_name_the_older_draft() {
    let (d, mut m) = export_session();
    let a = project_folder(&d, "a");
    m.set_project_root(Some(&a));
    m.open_edit();
    gloss(&mut m, 12.5);
    assert!(draft(&a).is_file());
    std::fs::remove_dir_all(&a).unwrap();
    gloss(&mut m, 13.5);
    let mut h = view(m);
    h.run_steps(4);
    h.get_by_label_contains("Draft not saved yet");
    assert!(h.query_by_label_contains("Draft saved in").is_none());
    assert!(h
        .get_by_label("Show in Explorer: the draft")
        .accesskit_node()
        .is_disabled());
}

/// Ruling E6-Y1: a session opened from a winning kept autosave names that file, and Show in Explorer reveals it.
#[test]
fn a_session_opened_from_a_kept_autosave_names_it() {
    let (d, mut m, p) = session_in_project("p", None);
    m.open_edit();
    gloss(&mut m, 12.5);
    make_old(&draft(&p));
    tick(&mut m, 40.5); // a drag, not flushed
    let id = id_of(&p);
    let p2 = d.path().join("p2");
    std::fs::rename(&p, &p2).unwrap();
    m.set_project_root(Some(&p2));
    assert!(kept(&d, &id).exists(), "the drag was kept at the switch");
    assert_eq!(
        op_values(&m).iter().map(|v| v.1).collect::<Vec<_>>(),
        [12.5, 40.5],
        "the kept autosave won"
    );
    let mut h = view(m);
    h.run_steps(4);
    h.get_by_label_contains(&format!("Draft saved in {}", kept(&d, &id).display()));
    h.get_by_label("Show in Explorer: the draft").click();
    h.run_steps(4);
    assert_eq!(h.state().reveal_request, Some(kept(&d, &id)));
}

/// Ruling E6-Y1: a carry that failed leaves the edit in the plain autosave, which the line names.
#[test]
fn a_failed_carry_names_the_autosave_that_holds_the_edit() {
    let (d, mut m) = export_session();
    m.open_edit();
    gloss(&mut m, 12.5);
    assert!(autosave(&d).is_file());
    let p = project_folder(&d, "p");
    std::fs::write(p.join("_sdc"), b"a file where the folder goes").unwrap();
    m.set_project_root(Some(&p));
    let e = m.edit.as_ref().unwrap();
    assert!(
        e.export_block().is_some(),
        "the switch waits: {:?}",
        e.draft_path
    );
    assert!(autosave(&d).is_file(), "the autosave stays");
    let mut h = view(m);
    h.run_steps(4);
    h.get_by_label_contains(&format!("Draft saved in {}", autosave(&d).display()));
    assert!(!h
        .get_by_label("Show in Explorer: the draft")
        .accesskit_node()
        .is_disabled());
}

#[test]
fn before_the_first_edit_the_draft_button_is_off() {
    let (_d, mut m) = export_session();
    m.open_edit();
    let mut h = view(m);
    h.run_steps(4);
    h.get_by_label_contains("Draft not saved yet");
    assert!(h
        .get_by_label("Show in Explorer: the draft")
        .accesskit_node()
        .is_disabled());
    h.get_by_label("Show in Explorer: the draft").click();
    h.run_steps(4);
    assert_eq!(h.state().reveal_request, None);
}

/// A switch that waits on a failed save: the footer does not name the project the edit has not reached, nor a draft
/// that may be gone with its folder.
#[test]
fn a_failed_switch_does_not_name_a_draft() {
    let (d, mut m) = export_session();
    let (a, b) = (project_folder(&d, "a"), project_folder(&d, "b"));
    project_with_a_draft(&mut m, &b);
    m.set_project_root(Some(&a));
    m.open_edit();
    gloss(&mut m, 12.5);
    assert!(draft(&a).is_file());
    std::fs::write(
        d.path().join("nif-edits"),
        b"a file where the autosave folder goes",
    )
    .unwrap();
    std::fs::remove_dir_all(&a).unwrap();
    m.set_project_root(Some(&b));
    assert!(
        m.edit.as_ref().unwrap().export_block().is_some(),
        "the switch waits"
    );
    let mut h = view(m);
    h.run_steps(4);
    h.get_by_label_contains("Draft not saved yet");
    assert!(h.query_by_label_contains("Draft saved in").is_none());
    assert!(h
        .get_by_label("Show in Explorer: the draft")
        .accesskit_node()
        .is_disabled());
}
