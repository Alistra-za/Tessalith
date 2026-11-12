//! Note HF1: a no-set file writes nothing, so it blocks no same-named file ("stuck ... even when one is removed
//! from the set"). A plain `Remove` still exports, so the twins stay `IN-DUPLICATE`.
use engine::api::*;
use engine::codes::Code;
use engine::config::Target;
use engine::events::*;
use engine::overrides::parse_set;
use engine::policy::JobStatus;
use engine::sets::SetEdit;
use image::{ImageBuffer, Rgb};
use std::sync::Arc;
use tempfile::TempDir;

fn fixture() -> TempDir {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("in")).unwrap();
    for (n, name) in ["rock.png", "rock.tga"].iter().enumerate() {
        ImageBuffer::from_fn(16, 16, |x, y| {
            Rgb([(x * 5) as u8 + n as u8 * 40, (y * 3) as u8, 90])
        })
        .save(d.path().join("in").join(name))
        .unwrap();
    }
    d
}

fn plan(d: &TempDir, edits: Vec<SetEdit>) -> Plan {
    let mut r = BatchRequest::new(vec![d.path().join("in")], d.path().join("out"), Target::Cs);
    r.recursive = true;
    r.overrides = [
        "file:**/rock.png.subtype=diffuse",
        "file:**/rock.tga.subtype=diffuse",
    ]
    .iter()
    .map(|s| parse_set(s).unwrap())
    .collect();
    r.set_edits = edits;
    let rep = Reporter::new(Arc::new(VecSink::default()), "t");
    Engine::new()
        .unwrap()
        .plan(r, &rep, &CancelToken::default())
        .unwrap()
}

/// The files `prepare` found `IN-DUPLICATE`, sorted.
fn dups(p: &Plan) -> Vec<String> {
    let mut v: Vec<String> = p
        .skipped
        .iter()
        // The writer dedupe (BV2) included: a no-set job never claims or loses an output path (Note HF9).
        .filter(|s| s.code == Some(Code::InDuplicate))
        .map(|s| s.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn a_no_set_twin_stops_blocking_its_twin() {
    let d = fixture();
    // Both write rock.dds.
    assert_eq!(dups(&plan(&d, vec![])), ["rock.png", "rock.tga"]);
    let p = plan(
        &d,
        vec![SetEdit::NoSet {
            file: "rock.png".into(),
        }],
    );
    assert!(dups(&p).is_empty(), "no IN-DUPLICATE: {:?}", dups(&p));
    let tga = p
        .jobs
        .iter()
        .find(|j| j.input.ends_with("rock.tga"))
        .expect("rock.tga has a job");
    assert!(matches!(tga.status, JobStatus::Ready), "{:?}", tga.status);
    assert_eq!(tga.output.file_name().unwrap(), "rock.dds");
    // `Remove` keeps the file exporting, so the clash stays.
    let p = plan(
        &d,
        vec![SetEdit::Remove {
            file: "rock.png".into(),
        }],
    );
    assert_eq!(dups(&p), ["rock.png", "rock.tga"]);
}

/// Note HF1 (final review I1): the engine never writes a no-set file, whatever `write_only` says (here `None`): the
/// twin alone writes `rock.dds`.
#[test]
fn a_no_set_file_is_never_written() {
    let d = fixture();
    let p = plan(
        &d,
        vec![SetEdit::NoSet {
            file: "rock.png".into(),
        }],
    );
    assert!(p.request.write_only.is_none());
    let sink = Arc::new(VecSink::default());
    let rep = Reporter::new(sink.clone(), "t");
    let s = Engine::new()
        .unwrap()
        .run(p, &rep, &CancelToken::default())
        .unwrap();
    assert_eq!(s.converted, 1, "{s:?}");
    let done: Vec<String> = sink
        .take()
        .into_iter()
        .filter(|e| matches!(e.kind, EventKind::FileDone { .. }))
        .filter_map(|e| e.path)
        .map(|p| p.rsplit(['/', '\\']).next().unwrap().to_string())
        .collect();
    assert_eq!(done, ["rock.tga"]);
}

/// Note HF1 (final review M2): a no-set key names every file the `[files]` rule names: `rock` is both twins.
#[test]
fn a_no_set_key_names_every_file_it_matches() {
    let d = fixture();
    let p = plan(
        &d,
        vec![SetEdit::NoSet {
            file: "rock".into(),
        }],
    );
    let names: Vec<String> = p
        .no_set
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["rock.png", "rock.tga"]);
    assert!(engine::sets::no_set_names("Rock", "rock.tga"));
    assert!(engine::sets::no_set_names("rock.png", "rock.png"));
    assert!(!engine::sets::no_set_names("rock.png", "rock.tga"));
}
