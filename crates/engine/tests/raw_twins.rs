//! Note HE7: two raw maps of one name (`x_ao.png` + `x_ao.tga`) write nothing of their own, so they are not
//! `IN-DUPLICATE`: the first by path feeds the channel (Note M13) and the other is `PACK-UNUSED-INPUT`.
use engine::api::*;
use engine::codes::Code;
use engine::config::{RawMap, Target};
use engine::events::*;
use engine::overrides::parse_set;
use engine::pack::ChannelSource;
use engine::policy::JobStatus;
use engine::sets::SetEdit;
use image::{ImageBuffer, Rgb};
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;

fn put(d: &TempDir, name: &str, seed: u8) {
    let p = d.path().join("in").join(name);
    ImageBuffer::from_fn(16, 16, |x, y| {
        Rgb([(x * 5) as u8 ^ seed, (y * 3) as u8, seed])
    })
    .save(p)
    .unwrap();
}

fn project(names: &[&str], toml: &str) -> TempDir {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("in")).unwrap();
    for (n, name) in names.iter().enumerate() {
        put(&d, name, 10 + n as u8 * 20);
    }
    if !toml.is_empty() {
        std::fs::write(d.path().join("in/sdc.toml"), toml).unwrap();
    }
    d
}

fn plan(d: &TempDir, sets: &[&str], edits: Vec<SetEdit>) -> Plan {
    let mut r = BatchRequest::new(vec![d.path().join("in")], d.path().join("out"), Target::Cs);
    r.recursive = true;
    r.overrides = sets.iter().map(|s| parse_set(s).unwrap()).collect();
    r.set_edits = edits;
    let rep = Reporter::new(Arc::new(VecSink::default()), "t");
    Engine::new()
        .unwrap()
        .plan(r, &rep, &CancelToken::default())
        .unwrap()
}

fn name(p: &Path) -> String {
    p.file_name().unwrap().to_string_lossy().into_owned()
}

/// The files `IN-DUPLICATE` skipped, sorted.
fn dups(p: &Plan) -> Vec<String> {
    let mut v: Vec<String> = p
        .skipped
        .iter()
        .filter(|s| s.code == Some(Code::InDuplicate))
        .map(|s| name(&s.path))
        .collect();
    v.sort();
    v
}

/// The file feeding `role` in the plan's packed job.
fn source_of(p: &Plan, role: RawMap) -> Option<String> {
    p.jobs
        .iter()
        .filter_map(|j| j.pack.as_ref())
        .flat_map(|pk| pk.channels.iter())
        .find_map(|c| match c {
            ChannelSource::Source { path, role: r, .. } if *r == role => Some(name(path)),
            _ => None,
        })
}

/// The `PACK-UNUSED-INPUT` warnings: (file, message).
fn unused(p: &Plan) -> Vec<(String, String)> {
    p.jobs
        .iter()
        .filter(|j| matches!(j.status, JobStatus::PackingInput { .. }))
        .flat_map(|j| {
            j.warnings
                .iter()
                .filter(|w| w.code == Code::PackUnusedInput)
                .map(|w| (name(&j.input), w.message.clone()))
        })
        .collect()
}

fn ready_at(p: &Plan, out: &str) -> Vec<String> {
    p.jobs
        .iter()
        .filter(|j| j.status == JobStatus::Ready && name(&j.output) == out)
        .map(|j| name(&j.input))
        .collect()
}

const SET: [&str; 3] = ["x.png", "x_rough.png", "x_ao.png"];

fn twins() -> TempDir {
    project(&[SET[0], SET[1], SET[2], "x_ao.tga"], "")
}

#[test]
fn two_same_stem_raw_maps_feed_the_channel() {
    let d = twins();
    let p = plan(&d, &[], vec![]);
    assert_eq!(dups(&p), Vec::<String>::new());
    assert_eq!(source_of(&p, RawMap::Ao).as_deref(), Some("x_ao.png"));
    let u = unused(&p);
    assert_eq!(u.len(), 1, "{u:?}");
    assert_eq!(u[0].0, "x_ao.tga");
    assert!(u[0].1.contains("x_ao.png"), "{}", u[0].1);
}

#[test]
fn removing_one_twin_leaves_the_other_feeding() {
    let d = twins();
    let p = plan(
        &d,
        &[],
        vec![SetEdit::Remove {
            file: "x_ao.png".into(),
        }],
    );
    // The removed file is a set of its own that packs the same `x_rmaos`; the BV2 writer dedupe flags that one
    // (not the twin pair), and it is the only IN-DUPLICATE.
    assert_eq!(dups(&p), ["x_ao.png"]);
    assert!(p.skipped[0]
        .reason
        .starts_with("two outputs map to one path"));
    assert_eq!(source_of(&p, RawMap::Ao).as_deref(), Some("x_ao.tga"));
}

#[test]
fn a_twin_set_to_none_writes_and_the_raw_one_still_feeds() {
    let d = twins();
    let p = plan(
        &d,
        &[
            "file:**/x_ao.png.helper.role=none",
            "file:**/x_ao.png.subtype=diffuse",
        ],
        vec![],
    );
    assert_eq!(dups(&p), Vec::<String>::new());
    assert_eq!(ready_at(&p, "x_ao.dds"), ["x_ao.png"]);
    assert_eq!(source_of(&p, RawMap::Ao).as_deref(), Some("x_ao.tga"));
}

#[test]
fn two_writing_twins_stay_duplicates() {
    let d = twins();
    let p = plan(
        &d,
        &[
            "file:**/x_ao.png.helper.role=none",
            "file:**/x_ao.tga.helper.role=none",
        ],
        vec![],
    );
    assert_eq!(dups(&p), ["x_ao.png", "x_ao.tga"]);
    // No raw name: both write, so both are duplicates.
    let d = project(&["wall.png", "wall.tga"], "");
    assert_eq!(dups(&plan(&d, &[], vec![])), ["wall.png", "wall.tga"]);
}

/// A rule that names a sub-type is invisible to `prepare`; the BV2 writer dedupe is the backstop.
#[test]
fn a_rule_typed_twin_pair_never_both_write() {
    let d = project(
        &[SET[0], SET[1], SET[2], "x_ao.tga"],
        "[[rules]]\nmatch = { glob = \"**/x_ao.*\" }\nsubtype = \"diffuse\"\n",
    );
    let p = plan(&d, &[], vec![]);
    assert!(
        ready_at(&p, "x_ao.dds").len() <= 1,
        "{:?}",
        ready_at(&p, "x_ao.dds")
    );
    assert_eq!(dups(&p).len(), 1, "{:?}", dups(&p));
}

#[test]
fn two_skin_f_twins_feed_fuzz() {
    let skin = "[files.\"y_rmaos\".packing]\nrecipe = \"rfaos\"\n";
    let d = project(
        &[
            "y.png",
            "y_sk.png",
            "y_roughness.png",
            "y_ao.png",
            "y_f.png",
            "y_f.tga",
        ],
        skin,
    );
    let p = plan(&d, &[], vec![]);
    assert_eq!(dups(&p), Vec::<String>::new());
    assert_eq!(source_of(&p, RawMap::Fuzz).as_deref(), Some("y_f.png"));
    // Outside a skin set a PBR `_f` writes.
    let d = project(&["z.png", "z_f.png", "z_f.tga"], "");
    assert_eq!(dups(&plan(&d, &[], vec![])), ["z_f.png", "z_f.tga"]);
}
