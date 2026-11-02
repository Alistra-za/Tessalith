//! Planning time grows linearly with the number of `[files]` entries: each file's lookup goes through the key index,
//! not a scan of every entry.
use engine::api::*;
use engine::config::Target;
use engine::events::{Reporter, VecSink};
use engine::presets::ConfigFile;
use image::{ImageBuffer, Rgba};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// `n` 4x4 PNGs in `<dir>/p<n>/textures`, each with one `subtype = "diffuse"` `[files]` entry.
fn project(dir: &Path, n: usize) -> BatchRequest {
    let t = dir.join(format!("p{n}")).join("textures");
    std::fs::create_dir_all(&t).unwrap();
    let mut cfg = ConfigFile::default();
    for i in 0..n {
        let name = format!("f{i:05}.png");
        ImageBuffer::from_fn(4, 4, |x, y| {
            Rgba([x as u8 * 60, y as u8 * 60, i as u8, 255])
        })
        .save(t.join(&name))
        .unwrap();
        cfg.files
            .insert(name, serde_json::json!({ "subtype": "diffuse" }));
    }
    let mut req = BatchRequest::new(vec![t], dir.join(format!("out{n}")), Target::Cs);
    req.project_config_inline = Some(cfg);
    req
}

fn best_plan(e: &Engine, req: &BatchRequest, n: usize, runs: usize) -> Duration {
    (0..runs)
        .map(|_| {
            let rep = Reporter::new(Arc::new(VecSink::default()), "scale");
            let t = Instant::now();
            let plan = e.plan(req.clone(), &rep, &CancelToken::default()).unwrap();
            let took = t.elapsed();
            assert_eq!(plan.jobs.len(), n);
            took
        })
        .min()
        .unwrap()
}

#[test]
fn plan_scales_linearly_with_files_entries() {
    let d = tempfile::tempdir().unwrap();
    // Four times the files (and entries) must cost well under 16 times (a scan of every entry per file), with room
    // for noise: a linear plan gives about 4.
    const N: usize = 500;
    let (small, large) = (project(d.path(), N), project(d.path(), 4 * N));
    let e = Engine::new().unwrap();
    let (ts, tl) = (best_plan(&e, &small, N, 3), best_plan(&e, &large, 4 * N, 3));
    eprintln!("{N} files {ts:?}, {} files {tl:?}", 4 * N);
    assert!(
        tl < ts * 10,
        "planning {} files took {tl:?}, more than 10 times the {ts:?} of {N} files",
        4 * N
    );
}
