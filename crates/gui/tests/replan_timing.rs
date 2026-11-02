//! Fast re-plan, Task 9: how fast one edit settles on a real project. Owner data, read-only: `#[ignore]`, and it
//! skips unless `SDC_REPLAN_PROJECT` (the project folder) and `SDC_REPLAN_SCRATCH` (an empty or missing folder on
//! the same volume) are set; `SDC_REPLAN_SCAN_DB` (a copy of scan.db, opened read-only) is optional. The sources are
//! hard-linked into the scratch folder and the `.toml` files copied, so nothing is written to the project. Run with
//! `cargo test -p sdc-gui --release --test replan_timing -- --ignored --nocapture`.
use engine::config::AlphaPolicy;
use sdc_gui::model::bottom::problems;
use sdc_gui::model::pbr_export::{export_asks, export_blocks};
use sdc_gui::model::sets::{self, views};
use sdc_gui::model::AppModel;
use sdc_gui::settings::GuiSettings;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Removes the scratch folder (links and copies only) however the test ends.
struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// Best and median of `n` runs of `f`, in ms.
fn time(n: usize, mut f: impl FnMut()) -> (f64, f64) {
    let mut t: Vec<f64> = (0..n)
        .map(|_| {
            let s = Instant::now();
            f();
            ms(s.elapsed())
        })
        .collect();
    t.sort_by(f64::total_cmp);
    (t[0], t[n / 2])
}

fn count_files(dir: &Path) -> usize {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .count()
}

#[test]
#[ignore = "owner data: set SDC_REPLAN_PROJECT and SDC_REPLAN_SCRATCH"]
fn one_edit_settles() {
    let (Some(project), Some(scratch)) = (
        std::env::var_os("SDC_REPLAN_PROJECT").map(PathBuf::from),
        std::env::var_os("SDC_REPLAN_SCRATCH").map(PathBuf::from),
    ) else {
        eprintln!("SDC_REPLAN_PROJECT / SDC_REPLAN_SCRATCH unset: skipped");
        return;
    };
    assert!(project.is_dir(), "{} is not a folder", project.display());
    assert!(
        !scratch.starts_with(&project) && !project.starts_with(&scratch),
        "the scratch folder must be outside the project"
    );
    assert!(
        !scratch.exists() || std::fs::read_dir(&scratch).unwrap().next().is_none(),
        "{} must be empty",
        scratch.display()
    );
    let before = count_files(&project);
    // Nothing reaches the real %APPDATA%\sdc or %LOCALAPPDATA%\sdc.
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("APPDATA", tmp.path().join("roaming"));
    std::env::set_var("LOCALAPPDATA", tmp.path().join("local"));

    // 1. Hard-link the sources, copy the `.toml` files.
    let guard = Scratch(scratch.clone());
    let t = Instant::now();
    for e in walkdir::WalkDir::new(&project) {
        let e = e.unwrap();
        let to = scratch.join(e.path().strip_prefix(&project).unwrap());
        if e.file_type().is_dir() {
            std::fs::create_dir_all(&to).unwrap();
        } else if e.path().extension().is_some_and(|x| x == "toml") {
            std::fs::copy(e.path(), &to).unwrap();
        } else if let Err(err) = std::fs::hard_link(e.path(), &to) {
            panic!("hard link {} failed: {err}", e.path().display());
        }
    }
    println!(
        "linked {} files in {:.0} ms",
        count_files(&scratch),
        ms(t.elapsed())
    );

    // 2. Open the copy (the analysis disk cache in scratch: a cold open), then wait.
    let session = engine::session::Session::new(encoders_sys::Bc1Model::Ideal)
        .unwrap()
        .with_disk_cache(tmp.path().join("analysis-cache"));
    let export = tmp.path().join("export");
    let settings = GuiSettings {
        export_folder: Some(export.clone()),
        ..GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, Arc::new(session));
    // One edit is one plan: no suggestion is recorded after it (`finish_plan`).
    m.accept = engine::policy::AcceptSuggestions::None;
    m.scan_db = std::env::var_os("SDC_REPLAN_SCAN_DB").map(PathBuf::from);
    let t = Instant::now();
    m.open_folder(scratch.clone());
    assert!(
        m.wait_idle(Duration::from_secs(900)),
        "the open did not settle"
    );
    println!("open settled in {:.1} s", t.elapsed().as_secs_f64());
    m.set_export_folder(export);
    assert!(m.wait_idle(Duration::from_secs(300)));
    let t = Instant::now();
    m.replan();
    assert!(m.wait_idle(Duration::from_secs(300)));
    println!(
        "warm full re-plan (no edit) settled in {:.2} s",
        t.elapsed().as_secs_f64()
    );
    let key = m
        .items
        .iter()
        .find(|i| {
            i.job.as_ref().is_some_and(|j| {
                j.profile.subtype == engine::config::Subtype::Diffuse
                    && j.profile.alpha.policy != AlphaPolicy::Cutout
            })
        })
        .map(|i| i.key.clone())
        .expect("a diffuse with a job");
    println!(
        "{} items, {} sets; edited key {key}",
        m.items.len(),
        m.sets.len()
    );

    // The frame after a full plan (as the app draws one before the next edit), for comparison with (ii).
    let frame = |m: &AppModel| {
        let s = Instant::now();
        let _ = views(m);
        let _ = problems(m);
        ms(s.elapsed())
    };
    m.invalidate_views();
    println!(
        "frame after a full plan (views + problems): {:.1} ms",
        frame(&m)
    );

    // 3. Time (i), (ii), (iii), alternating the value; best of 3.
    let (mut t1, mut t2, mut t3) = (vec![], vec![], vec![]);
    for (n, policy) in [AlphaPolicy::Cutout, AlphaPolicy::Blend, AlphaPolicy::Cutout]
        .into_iter()
        .enumerate()
    {
        let plans = m.plan_marks().1;
        let s = Instant::now();
        m.set_field(&key, "alpha.policy", json!(policy));
        let edit = s.elapsed();
        assert!(m.partial_pending, "the edit sent a partial plan");
        let (mut slowest, mut landing) = (Duration::ZERO, Duration::ZERO);
        while m.applied_seq < m.edit_seq {
            let p = Instant::now();
            m.poll();
            let d = p.elapsed();
            slowest = slowest.max(d);
            if m.applied_seq == m.edit_seq {
                landing = d;
            }
            assert!(s.elapsed() < Duration::from_secs(120), "no partial landed");
            std::thread::sleep(Duration::from_millis(1));
        }
        let i = s.elapsed();
        assert_eq!(
            m.plan_marks().1,
            plans,
            "the partial landed first, not the full plan"
        );
        let f = Instant::now();
        let _ = views(&m);
        let rows = problems(&m).len();
        let first = f.elapsed();
        let ii = s.elapsed();
        assert!(m.wait_idle(Duration::from_secs(300)));
        let iii = s.elapsed();
        assert_eq!(m.plan_marks().1, plans + 1, "one edit is one full plan");
        println!(
            "edit {n} ({policy:?}): set_field {:.1} ms; (i) partial applied {:.0} ms (the poll that applied it \
             {:.1} ms, slowest poll {:.1} ms); (ii) +views+problems {:.1} ms ({rows} rows) = {:.0} ms; (iii) settled \
             {:.2} s",
            ms(edit),
            ms(i),
            ms(landing),
            ms(slowest),
            ms(first),
            ms(ii),
            iii.as_secs_f64()
        );
        t1.push(ms(i));
        t2.push(ms(ii));
        t3.push(iii.as_secs_f64());
        // The app draws after the full plan lands; the next partial splices that frame's views.
        println!("  frame after its full plan: {:.1} ms", frame(&m));
    }
    let best = |v: &[f64]| v.iter().copied().fold(f64::INFINITY, f64::min);
    println!(
        "BEST: (i) {:.0} ms, (ii) {:.0} ms, (iii) {:.2} s",
        best(&t1),
        best(&t2),
        best(&t3)
    );

    // The GUI thread's part of one edit, on the settled model.
    let (b, md) = time(5, || {
        let _ = m.request();
    });
    println!("request(): best {b:.1} ms, median {md:.1} ms");
    let doc = m.project.as_ref().unwrap();
    let (b, md) = time(5, || {
        let _ = doc.config();
    });
    println!("  project.config() (sdc.toml to text, parsed): best {b:.1} ms, median {md:.1} ms");
    for (name, f) in [
        (
            "engine_inputs",
            &(|m: &AppModel| sets::engine_inputs(m).0.len()) as &dyn Fn(&AppModel) -> usize,
        ),
        ("  of which output_candidates", &|m: &AppModel| {
            sets::output_candidates(m, &views(m)).len()
        }),
        ("  of which output_name_paths", &|m: &AppModel| {
            sets::output_name_paths(m, &views(m)).len()
        }),
        ("shared_roughness", &|m: &AppModel| {
            sets::shared_roughness(m).len()
        }),
        ("borrowed_normal", &|m: &AppModel| {
            sets::borrowed_normal(m).len()
        }),
        ("set_shaders", &|m: &AppModel| sets::set_shaders(m).len()),
        ("pinned_files", &|m: &AppModel| sets::pinned_files(m).len()),
        ("cs_txst_sets", &|m: &AppModel| sets::cs_txst_sets(m).len()),
    ] {
        let (b, md) = time(5, || {
            let _ = f(&m);
        });
        println!("  {name}: best {b:.1} ms, median {md:.1} ms");
    }
    let req = m.request().unwrap();
    let (b, md) = time(5, || {
        let _ = req.sig_without_files();
    });
    println!("sig_without_files: best {b:.1} ms, median {md:.1} ms");
    let old = req.project_config_inline.as_ref().unwrap().files.clone();
    let (b, md) = time(5, || {
        // `send_partial`'s clone and diff of `[files]`.
        let files = req.project_config_inline.as_ref().unwrap().files.clone();
        let n = old
            .keys()
            .chain(files.keys())
            .filter(|k| old.get(*k) != files.get(*k))
            .count();
        assert_eq!(n, 0);
    });
    println!(
        "[files] clone + diff ({} keys): best {b:.1} ms, median {md:.1} ms",
        old.len()
    );

    // `review_gate`, as the header calls it every frame: once on a fresh views key (the first frame after a plan;
    // views and Problems are warm, as that frame draws them), then steady state, and its parts.
    m.invalidate_views();
    let _ = frame(&m);
    let s = Instant::now();
    let gate = m.review_gate();
    println!(
        "review_gate, first call after a plan: {:.2} ms",
        ms(s.elapsed())
    );
    let (b, md) = time(20, || {
        let _ = m.review_gate();
    });
    println!("review_gate ({gate:?}): best {b:.2} ms, median {md:.2} ms");
    for (name, f) in [
        (
            "export_blocks",
            &(|m: &AppModel| export_blocks(m).len()) as &dyn Fn(&AppModel) -> usize,
        ),
        ("export_asks", &|m: &AppModel| export_asks(m).len()),
        ("unanswered_half_size", &|m: &AppModel| {
            m.unanswered_half_size()
        }),
        ("unanswered_overwrites", &|m: &AppModel| {
            m.unanswered_overwrites().len()
        }),
        ("export_files", &|m: &AppModel| m.export_files()),
        ("undecided", &|m: &AppModel| m.undecided()),
    ] {
        let (b, md) = time(20, || {
            let _ = f(&m);
        });
        println!("  {name}: best {b:.2} ms, median {md:.2} ms");
    }

    // 4. Remove the links and copies; the project is as it was.
    drop(m);
    drop(guard);
    assert!(!scratch.exists(), "the scratch folder was removed");
    assert_eq!(count_files(&project), before, "the project is unchanged");
}
