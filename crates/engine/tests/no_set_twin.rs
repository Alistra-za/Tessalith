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
        // The writer dedupe (BV2) is Task 4's to end: a no-set file that resolves still meets its twin there.
        .filter(|s| {
            s.code == Some(Code::InDuplicate)
                && !s.reason.starts_with("two outputs map to one path ")
        })
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
