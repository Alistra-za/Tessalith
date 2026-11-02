//! Fast re-plan (R2 §5): an edit to a `[files]` key shows a partial plan of the sets it reaches at once; the full
//! plan lands behind it and always wins. `hold_plan_results` parks the plan messages so each test delivers them in
//! the order it wants.
mod support;
use engine::config::{AlphaPolicy, Subtype};
use sdc_gui::model::bottom::{problems, problems_builds, At, Level};
use sdc_gui::model::sets::{view_builds, views, SetView, SlotFill, SlotKind};
use sdc_gui::model::{Answer, AppModel};
use sdc_gui::settings::GuiSettings;
use sdc_gui::worker::WorkerMsg;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use support::*;

const LEAF: &str = "landscape/leaf.png";
const ROCK: &str = "landscape/rock01.png";
const DDS: &str = "landscape/plate.dds";
const WAIT: &str = "Wait for the plan to finish";

/// The support fixture plus `extra`, opened with its export folder chosen, then holding plan results.
fn open(dir: &Path, extra: impl FnOnce(&Path)) -> AppModel {
    let root = mod_folder(dir);
    extra(&root);
    let mut m = fresh(dir, root);
    m.hold_plan_results = true;
    m
}

fn fresh(dir: &Path, root: PathBuf) -> AppModel {
    let settings = GuiSettings {
        export_folder: Some(dir.join("export")),
        ..GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, session());
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(root);
    assert!(m.wait_idle(Duration::from_secs(60)));
    choose_export(&mut m, dir.join("export"));
    m
}

fn is_partial(x: &WorkerMsg) -> bool {
    matches!(x, WorkerMsg::Partial { .. })
}

fn seq_of(x: &WorkerMsg) -> u64 {
    match x {
        WorkerMsg::Partial { seq, .. } => *seq,
        _ => u64::MAX,
    }
}

/// Poll until at least `partials` partials and `planned` full plans are parked; returns them, partials by `seq`.
fn collect(m: &mut AppModel, partials: usize, planned: usize) -> (Vec<WorkerMsg>, Vec<WorkerMsg>) {
    let end = Instant::now() + Duration::from_secs(60);
    let mut got = vec![];
    loop {
        m.poll();
        got.extend(m.take_held());
        let p = got.iter().filter(|x| is_partial(x)).count();
        if p >= partials && got.len() - p >= planned {
            let (mut p, f): (Vec<_>, Vec<_>) = got.into_iter().partition(is_partial);
            p.sort_by_key(seq_of);
            return (p, f);
        }
        assert!(Instant::now() < end, "plan messages did not arrive");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Handle `msgs` in this order, then poll with results still held (a plan message arriving meanwhile stays parked).
fn deliver(m: &mut AppModel, msgs: Vec<WorkerMsg>) {
    for x in msgs {
        m.handle_held(x);
    }
    m.poll();
}

/// Deliver the rest and let every plan run through.
fn finish(m: &mut AppModel, msgs: Vec<WorkerMsg>) {
    m.hold_plan_results = false;
    for x in msgs {
        m.sender().send(x).unwrap();
    }
    assert!(m.wait_idle(Duration::from_secs(60)));
}

fn policy(m: &AppModel, key: &str) -> Option<AlphaPolicy> {
    m.item(key)?.job.as_ref().map(|j| j.profile.alpha.policy)
}

/// What a fresh open must show the same: key, set, status, warnings and the whole job.
fn shape(m: &AppModel) -> Vec<String> {
    m.items
        .iter()
        .map(|i| {
            format!(
                "{} {:?} {:?} {:?} {}",
                i.key,
                i.set_key,
                i.status,
                i.warnings,
                serde_json::to_value(&i.job).unwrap()
            )
        })
        .collect()
}

/// Save, open the folder again in a new window, and compare its items with `m`'s.
fn equals_a_fresh_open(dir: &Path, m: &mut AppModel) {
    m.save().unwrap();
    let again = fresh(dir, m.root.clone().unwrap());
    assert_eq!(shape(m), shape(&again));
}

fn cutout(m: &mut AppModel, key: &str) {
    m.set_field(key, "alpha.policy", json!(AlphaPolicy::Cutout));
}

#[test]
fn partial_lands_before_full() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open(d.path(), |_| {});
    assert_ne!(policy(&m, LEAF), Some(AlphaPolicy::Cutout));
    let rock = policy(&m, ROCK);
    let sdc = m.project.as_ref().unwrap().path().to_path_buf();
    let on_disk = std::fs::read_to_string(&sdc).ok();
    let marks = m.plan_marks();
    cutout(&mut m, LEAF);
    let text = m.project.as_ref().unwrap().text();
    let marks_edit = m.plan_marks();
    let rev = m.items_rev();
    let (partials, full) = collect(&mut m, 1, 1);
    assert_eq!(partials.len(), 1);
    deliver(&mut m, partials);
    assert_eq!(
        policy(&m, LEAF),
        Some(AlphaPolicy::Cutout),
        "the partial shows the edit"
    );
    assert_eq!(policy(&m, ROCK), rock);
    assert!(m.items_rev() > rev, "the views see the splice");
    assert!(!m.partial_pending);
    // Note F?: the panel takes the next edit at once; the status line still says the full plan runs.
    assert_eq!(m.fix_off(), None, "the panel waits only for the partial");
    // Problems lists rows (the fixture's undecided textures), and its fix buttons are enabled by the same `fix_off`
    // (`ui/problems.rs`), so they are live behind the partial too.
    assert!(!sdc_gui::model::bottom::problems(&m).is_empty());
    assert!(!m.plan_wait(), "Problems' fix buttons wait for nothing");
    assert!(m.status_line.ends_with(" · planning…"), "{}", m.status_line);
    let generation = m.generation();
    m.sender()
        .send(WorkerMsg::PlanProgress {
            generation,
            done: 1,
            total: 6,
        })
        .unwrap();
    m.poll();
    assert_eq!(m.plan_progress, Some((1, 6)));
    assert!(
        m.status_line.ends_with(" · planning…"),
        "a progress tick keeps the counts: {}",
        m.status_line
    );
    // Nothing but the edit's own `replan` moved: no `last_inputs` write, no checks run, no project write.
    assert_eq!(m.plan_marks(), marks_edit);
    assert_eq!(marks_edit.1, marks.1);
    assert_eq!(m.project.as_ref().unwrap().text(), text);
    assert_eq!(std::fs::read_to_string(&sdc).ok(), on_disk);
    assert!(m.busy(), "the full plan is still due");
    finish(&mut m, full);
    assert_eq!(policy(&m, LEAF), Some(AlphaPolicy::Cutout));
    assert!(
        m.dirty_keys.is_empty(),
        "the full plan clears the dirty keys"
    );
    equals_a_fresh_open(d.path(), &mut m);
}

#[test]
fn two_partials_in_a_row_keep_both() {
    for reversed in [false, true] {
        let d = tempfile::tempdir().unwrap();
        let mut m = open(d.path(), |_| {});
        cutout(&mut m, LEAF);
        cutout(&mut m, ROCK);
        let (mut partials, full) = collect(&mut m, 2, 2);
        let newest = seq_of(&partials[1]);
        if reversed {
            partials.reverse();
        }
        deliver(&mut m, partials);
        assert_eq!(
            policy(&m, LEAF),
            Some(AlphaPolicy::Cutout),
            "reversed: {reversed}"
        );
        assert_eq!(
            policy(&m, ROCK),
            Some(AlphaPolicy::Cutout),
            "reversed: {reversed}"
        );
        assert_eq!(
            m.applied_seq, newest,
            "an older partial never applies after a newer one"
        );
        finish(&mut m, full);
        equals_a_fresh_open(d.path(), &mut m);
    }
}

#[test]
fn a_partial_after_its_full_plan_is_dropped() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open(d.path(), |_| {});
    cutout(&mut m, LEAF);
    let (partials, full) = collect(&mut m, 1, 1);
    finish(&mut m, full);
    let (rev, seq) = (m.items_rev(), m.applied_seq);
    finish(&mut m, partials);
    assert_eq!(m.items_rev(), rev, "the full plan wins");
    assert_eq!(m.applied_seq, seq);
    equals_a_fresh_open(d.path(), &mut m);
}

#[test]
fn edit_during_full_plan() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open(d.path(), |_| {});
    cutout(&mut m, LEAF);
    let (p1, f1) = collect(&mut m, 1, 1);
    deliver(&mut m, p1);
    // A second edit while the first full plan is out: that plan is stale now.
    cutout(&mut m, ROCK);
    let (p2, f2) = collect(&mut m, 1, 1);
    deliver(&mut m, f1);
    assert!(m.busy(), "the stale full plan was dropped");
    assert_eq!(policy(&m, LEAF), Some(AlphaPolicy::Cutout));
    deliver(&mut m, p2);
    assert_eq!(policy(&m, LEAF), Some(AlphaPolicy::Cutout));
    assert_eq!(policy(&m, ROCK), Some(AlphaPolicy::Cutout));
    finish(&mut m, f2);
    equals_a_fresh_open(d.path(), &mut m);
}

#[test]
fn undo_after_partial() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open(d.path(), |_| {});
    let before = policy(&m, LEAF);
    cutout(&mut m, LEAF);
    let (p1, f1) = collect(&mut m, 1, 1);
    deliver(&mut m, p1);
    assert_eq!(policy(&m, LEAF), Some(AlphaPolicy::Cutout));
    m.undo();
    let (p2, f2) = collect(&mut m, 1, 1);
    assert!(m.dirty_keys.iter().any(|k| k == LEAF));
    deliver(&mut m, p2);
    assert_eq!(
        policy(&m, LEAF),
        before,
        "the undo's partial takes the edit back"
    );
    let mut rest = f1;
    rest.extend(f2);
    finish(&mut m, rest);
    assert_eq!(policy(&m, LEAF), before);
    equals_a_fresh_open(d.path(), &mut m);
}

/// Final fix (parked from Task 6): a partial removes only the RNM keys no item outside its scope still reads.
#[test]
fn a_partial_keeps_the_rnm_keys_other_sets_read() {
    let d = tempfile::tempdir().unwrap();
    let r = d.path().join("m");
    std::fs::create_dir_all(&r).unwrap();
    for f in ["iron.png", "copper.png"] {
        image::ImageBuffer::from_fn(64, 64, |x, y| {
            image::Rgb([(x * 4) as u8, (y * 4) as u8, 90u8])
        })
        .save(r.join(f))
        .unwrap();
    }
    for f in [
        "iron_n.png",
        "iron_bump.png",
        "iron_detailnorm.png",
        "iron_detailnormmask.png",
    ] {
        image::ImageBuffer::from_fn(64, 64, |x, _| image::Luma([(x * 3) as u8]))
            .save(r.join(f))
            .unwrap();
    }
    let settings = GuiSettings {
        export_folder: Some(d.path().join("export")),
        ..GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, session());
    m.accept = engine::policy::AcceptSuggestions::High;
    m.open_folder(d.path().to_path_buf());
    assert!(m.wait_idle(Duration::from_secs(60)));
    choose_export(&mut m, d.path().join("export"));
    let rnm = m
        .item("m/iron_n.png")
        .unwrap()
        .job
        .as_ref()
        .unwrap()
        .rnm
        .clone();
    let rnm = rnm.expect("iron_n has layers");
    let layer = rnm.layers[0].file.path.clone();
    assert!(m.rnm_keys.contains_key(&layer));
    // Copper, a set of its own, reads the same layer file.
    let copper = m
        .items
        .iter()
        .position(|i| i.key == "m/copper.png")
        .unwrap();
    m.items[copper].job.as_mut().expect("copper's job").rnm = Some(rnm);
    m.invalidate_views();
    m.hold_plan_results = true;
    m.set_field("m/copper.png", "alpha.policy", json!(AlphaPolicy::Cutout));
    let (partials, full) = collect(&mut m, 1, 1);
    assert!(
        matches!(&partials[0], WorkerMsg::Partial { result: Ok((_, scope)), .. } if scope.len() == 1),
        "a partial of copper's set alone"
    );
    deliver(&mut m, partials);
    assert!(m.rnm_keys.contains_key(&layer), "iron_n still reads it");
    finish(&mut m, full);
}

#[test]
fn needs_full_waits_for_full() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open(d.path(), |root| bc1_dds(&root.join(DDS)));
    assert!(
        m.item(DDS).is_some_and(|i| i.job.is_none()),
        "a skipped DDS"
    );
    m.set_reencode(&[DDS.to_string()], true);
    assert!(m.partial_pending, "a [files] edit sends a partial");
    let rev = m.items_rev();
    let (partials, full) = collect(&mut m, 1, 1);
    let WorkerMsg::Partial {
        result: Err(why), ..
    } = &partials[0]
    else {
        panic!("reencode needs a full plan")
    };
    assert!(why.contains("reencode"), "{why}");
    deliver(&mut m, partials);
    assert!(!m.partial_pending);
    assert_eq!(m.items_rev(), rev, "nothing changed");
    assert!(m.item(DDS).is_some_and(|i| i.job.is_none()));
    // Note F?: no partial for this edit, so the panel waits for the full plan (Note FK).
    assert_eq!(m.fix_off().as_deref(), Some(WAIT));
    finish(&mut m, full);
    assert_eq!(m.fix_off(), None);
    assert!(
        m.item(DDS).is_some_and(|i| i.job.is_some()),
        "the full plan re-encodes it"
    );
}

/// Note F?: a partial frees the texture panel only. Export clicked after it queues, Review says "Planning…",
/// and the export runs on the full plan once it lands.
#[test]
fn export_waits_for_full_after_partial() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open(d.path(), |_| {});
    m.hold_plan_results = false;
    decide_all(&mut m);
    m.hold_plan_results = true;
    m.set_field(LEAF, "alpha.policy", json!(AlphaPolicy::Blend));
    let (partials, full) = collect(&mut m, 1, 1);
    deliver(&mut m, partials);
    assert_eq!(policy(&m, LEAF), Some(AlphaPolicy::Blend));
    assert_eq!(m.fix_off(), None);
    assert!(m.full_plan_pending(), "Review says Planning…");
    m.export_clicked();
    assert!(m.export_queued(), "the click waits for the full plan");
    assert!(m.export.running.is_none() && m.review.is_none());
    finish(&mut m, full);
    // The queued click ran its checks on the full plan and, with everything decided, exported without Review.
    assert!(!m.export_queued());
    assert!(
        m.review.is_none(),
        "this fixture exports straight from the queue"
    );
    assert!(
        m.export.summary.is_some() && m.export.error.is_none(),
        "the export ran: {:?}",
        m.export.error
    );
    assert_eq!(policy(&m, LEAF), Some(AlphaPolicy::Blend));
}

/// A set of two plain files beside the fixture's: `wall.png` (a diffuse) and `wall_n.png` (a normal).
fn wall(root: &Path) {
    let r = root.join("landscape");
    image::ImageBuffer::from_fn(64, 64, |x, y| image::Rgb([x as u8 * 4, y as u8 * 4, 90u8]))
        .save(r.join("wall.png"))
        .unwrap();
    image::ImageBuffer::from_fn(64, 64, |_, _| image::Rgb([128u8, 128, 255]))
        .save(r.join("wall_n.png"))
        .unwrap();
}

const WALL: &str = "landscape/wall.png";
const WALL_N: &str = "landscape/wall_n.png";

/// Decide `key`'s sub-type and let every plan land, then hold plan results again.
fn decided(m: &mut AppModel, key: &str, st: Subtype) {
    m.hold_plan_results = false;
    m.decide(key, Answer::Subtype(st));
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.hold_plan_results = true;
}

fn view_of(m: &AppModel, id: &str) -> SetView {
    views(m).iter().find(|v| v.id == id).expect(id).clone()
}

/// Note F?7: after a partial and its full plan, Problems is what a fresh full build gives. The edit adds an export
/// block (`_g` beside `_sk` in slot 3): the partial shows the set's new slots at once, the block (a project-wide
/// check) waits for the full plan, and the full plan must not keep the blocks of the plan before it.
#[test]
fn problems_after_partial_then_full_equal_fresh() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open(d.path(), wall);
    decided(&mut m, WALL_N, Subtype::SkinSk);
    let clash = |m: &AppModel| {
        problems(m)
            .iter()
            .any(|p| p.level == Level::Blocks && p.text.contains("_g and _sk both use slot 3"))
    };
    assert!(!clash(&m));
    m.decide(WALL, Answer::Subtype(Subtype::Glow));
    let (partials, full) = collect(&mut m, 1, 1);
    deliver(&mut m, partials);
    let v = view_of(&m, "landscape/wall");
    assert!(
        v.slots.contains_key(&SlotKind::Emissive) && v.slots.contains_key(&SlotKind::SkinSk),
        "the partial shows the set's new slots: {:?}",
        v.slots
    );
    assert!(
        !clash(&m),
        "the export blocks stay as of the last full plan until it lands"
    );
    finish(&mut m, full);
    assert!(clash(&m), "the full plan's blocks");
    let after = problems(&m);
    m.save().unwrap();
    let again = fresh(d.path(), m.root.clone().unwrap());
    assert_eq!(*after, *problems(&again));
}

/// Note F?7 (final fixes I3): a block row carried from the last full plan keeps its fix off ("being rechecked")
/// while the full plan of an edit that resolved it is out; other rows' fixes work after the partial.
#[test]
fn a_carried_block_row_waits_for_the_full_plan() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open(d.path(), wall);
    decided(&mut m, WALL_N, Subtype::SkinSk);
    decided(&mut m, WALL, Subtype::Glow);
    let clash = |m: &AppModel| {
        problems(m)
            .iter()
            .find(|p| p.level == Level::Blocks && p.text.contains("_g and _sk both use slot 3"))
            .cloned()
    };
    let row = clash(&m).expect("the clash blocks");
    assert!(row.carried);
    assert_eq!(m.fix_off_for(&row), None, "no plan out: the fix works");
    m.decide(WALL, Answer::Subtype(Subtype::Diffuse));
    let (partials, full) = collect(&mut m, 1, 1);
    deliver(&mut m, partials);
    assert_eq!(m.fix_off(), None, "the partial landed: plain fixes work");
    let row = clash(&m).expect("the block is carried until the full plan");
    let off = m.fix_off_for(&row).expect("a carried row's fix is off");
    assert!(off.contains("rechecked"), "{off}");
    finish(&mut m, full);
    assert!(clash(&m).is_none(), "the full plan dropped the block");
}

/// Note F?7: the views a partial splices equal a fresh build on the same model, for an edited set and for a set that
/// borrows from it, and Problems' set rows read them at once.
#[test]
fn views_after_a_partial_equal_a_fresh_build() {
    // The leaf becomes a normal: its set's slot and its shader warning change.
    let d = tempfile::tempdir().unwrap();
    let mut m = open(d.path(), |_| {});
    decided(&mut m, LEAF, Subtype::Diffuse);
    m.decide(LEAF, Answer::Subtype(Subtype::Normal));
    let (partials, _full) = collect(&mut m, 1, 1);
    deliver(&mut m, partials);
    let spliced = views(&m);
    assert!(view_of(&m, "landscape/leaf")
        .slots
        .contains_key(&SlotKind::Normal));
    assert!(
        problems(&m).iter().any(
            |p| matches!(&p.at, At::Set { id, .. } if id == "landscape/leaf")
                && p.text.contains("needs diffuse")
        ),
        "the set's shader row follows the partial"
    );
    m.invalidate_views();
    assert_eq!(*spliced, *views(&m));

    // The leaf borrows the wall's normal; the wall's normal becomes `_sk`, so the borrow finds nothing.
    let d = tempfile::tempdir().unwrap();
    let mut m = open(d.path(), wall);
    m.hold_plan_results = false;
    m.edit_docs("borrow", None, true, |_, s| {
        s.set_borrow("landscape/leaf", 2, Some("landscape/wall"))
    });
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.hold_plan_results = true;
    let leaf = view_of(&m, "landscape/leaf");
    assert!(
        matches!(
            leaf.slots.get(&SlotKind::Normal),
            Some(SlotFill::Borrowed { .. })
        ),
        "{:?} {:?}",
        leaf.slots,
        leaf.missing
    );
    m.decide(WALL_N, Answer::Subtype(Subtype::SkinSk));
    let (partials, _full) = collect(&mut m, 1, 1);
    assert!(
        matches!(&partials[0], WorkerMsg::Partial { result: Ok(_), .. }),
        "a partial"
    );
    deliver(&mut m, partials);
    let spliced = views(&m);
    let leaf = view_of(&m, "landscape/leaf");
    assert!(
        !leaf.slots.contains_key(&SlotKind::Normal) && !leaf.missing.is_empty(),
        "the borrower follows its source: {:?} {:?}",
        leaf.slots,
        leaf.missing
    );
    m.invalidate_views();
    assert_eq!(*spliced, *views(&m));
}

/// Final fix I2: a full plan builds again only the views of the sets it changed, with no whole build, and the result
/// equals a fresh build. An edit of the sets document, or a plan that changes the engine inputs, builds them whole.
#[test]
fn views_after_a_full_plan_equal_a_fresh_build() {
    // The leaf becomes a normal; only its full plan lands (its partial stays parked).
    let d = tempfile::tempdir().unwrap();
    let mut m = open(d.path(), wall);
    decided(&mut m, LEAF, Subtype::Diffuse);
    let _ = views(&m);
    let builds = view_builds(&m);
    m.decide(LEAF, Answer::Subtype(Subtype::Normal));
    let (_partials, full) = collect(&mut m, 1, 1);
    finish(&mut m, full);
    let spliced = views(&m);
    assert!(view_of(&m, "landscape/leaf")
        .slots
        .contains_key(&SlotKind::Normal));
    assert_eq!(
        view_builds(&m),
        builds,
        "no whole build after the full plan"
    );
    m.invalidate_views();
    assert_eq!(*spliced, *views(&m));

    // The leaf borrows the wall's normal (a sets-document edit: built whole), then the wall's normal becomes `_sk`:
    // the borrower outside the edited set follows.
    m.hold_plan_results = false;
    let builds = view_builds(&m);
    m.edit_docs("borrow", None, true, |_, s| {
        s.set_borrow("landscape/leaf", 2, Some("landscape/wall"))
    });
    assert!(m.wait_idle(Duration::from_secs(60)));
    let _ = views(&m);
    assert!(
        view_builds(&m) > builds,
        "a sets-document edit builds whole"
    );
    assert!(matches!(
        view_of(&m, "landscape/leaf").slots.get(&SlotKind::Normal),
        Some(SlotFill::Borrowed { .. })
    ));
    m.hold_plan_results = true;
    let builds = view_builds(&m);
    m.decide(WALL_N, Answer::Subtype(Subtype::SkinSk));
    let (_partials, full) = collect(&mut m, 1, 1);
    let inputs = m.plan_marks().0;
    finish(&mut m, full);
    let spliced = views(&m);
    let leaf = view_of(&m, "landscape/leaf");
    // The leaf (a normal itself since the first part) loses the borrowed one and lists the empty borrow.
    assert!(
        !matches!(
            leaf.slots.get(&SlotKind::Normal),
            Some(SlotFill::Borrowed { .. })
        ) && !leaf.missing.is_empty(),
        "the borrower follows its source: {:?} {:?}",
        leaf.slots,
        leaf.missing
    );
    // `_sk` changes the engine inputs (output paths), which re-plans once more and moves the views key outside the
    // items: one whole build, as ruled.
    assert!(m.plan_marks().0 > inputs, "the engine inputs changed");
    assert_eq!(view_builds(&m), builds + 1, "built whole once");
    m.invalidate_views();
    assert_eq!(*spliced, *views(&m));
}

/// Final fix round 2 (B): a full plan that changes a helper field (the roughness read as glossiness, packed into the
/// rock's `_rmaos`) and no sub-type: the carried views equal a fresh build.
#[test]
fn views_after_a_helper_change_equal_a_fresh_build() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open(d.path(), |_| {});
    let rough = "landscape/rock01_roughness.png";
    let before = shape(&m);
    let _ = views(&m);
    m.hold_plan_results = false;
    m.set_field(rough, "helper.role", json!("glossiness"));
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert_ne!(shape(&m), before, "the plan changed an item");
    let carried = views(&m);
    m.invalidate_views();
    assert_eq!(*carried, *views(&m));
}

/// Final fix (Minor 7): a partial whose sets are not all sets of the landed plan (planned on the skeleton of a full plan
/// that has not landed here yet) is dropped; the panel waits for the full plan.
#[test]
fn a_partial_with_an_unknown_set_is_dropped() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open(d.path(), |_| {});
    cutout(&mut m, LEAF);
    let (partials, full) = collect(&mut m, 1, 1);
    let leaf_set = m.item(LEAF).unwrap().set_key.clone().unwrap();
    m.sets.retain(|s| s.key != leaf_set);
    m.invalidate_views();
    let seq = m.applied_seq;
    deliver(&mut m, partials);
    assert_eq!(m.applied_seq, seq, "dropped");
    assert_ne!(policy(&m, LEAF), Some(AlphaPolicy::Cutout));
    assert_eq!(m.fix_off().as_deref(), Some(WAIT));
    finish(&mut m, full);
    assert_eq!(policy(&m, LEAF), Some(AlphaPolicy::Cutout));
}

/// Note F?: the partial alone updates the edited set's item rows: answering an undecided alpha removes its row.
#[test]
fn problems_rows_of_the_edited_set_change_after_partial() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open(d.path(), |_| {});
    m.hold_plan_results = false;
    m.decide(LEAF, Answer::Subtype(Subtype::Diffuse));
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.hold_plan_results = true;
    let undecided = |m: &AppModel| {
        problems(m)
            .iter()
            .any(|p| p.level == Level::Decide && p.at == At::Texture(LEAF.into()))
    };
    assert!(undecided(&m), "the leaf's alpha is undecided");
    m.decide(LEAF, Answer::Alpha(AlphaPolicy::Cutout));
    let (partials, _full) = collect(&mut m, 1, 1);
    assert!(undecided(&m), "nothing landed yet");
    deliver(&mut m, partials);
    assert!(m.busy(), "the full plan is still out");
    assert!(!undecided(&m), "the partial removed the leaf's Decide row");
}

/// Note F?7: on a big project the first frame after a partial (the set views and Problems) costs a small part of the same frame after a full plan: the partial splices the scope sets' views into the cached
/// ones and carries the export blocks and asks over, so nothing rebuilds all sets. The name is the brief's; the test
/// measures that reuse (a ratio, as `rename_scale.rs`, not milliseconds).
#[test]
fn problems_rebuild_after_partial_is_per_set() {
    const N: usize = 4000;
    let d = tempfile::tempdir().unwrap();
    let mut m = open(d.path(), |_| {});
    // N more single-texture sets, each a clone of the rock's diffuse under its own key, set and output.
    let item = m.item(ROCK).expect("rock").clone();
    let set = m
        .sets
        .iter()
        .find(|s| item.set_key.as_ref() == Some(&s.key))
        .expect("rock's set")
        .clone();
    for i in 0..N {
        let key = format!("big/s{i:05}.png");
        let mut it = item.clone();
        it.key.clone_from(&key);
        it.job_key = format!("big/s{i:05}.dds");
        it.set_key = Some(format!("{}{i:05}", set.key));
        let mut s = set.clone();
        s.key = format!("{}{i:05}", set.key);
        s.members = vec![key];
        m.items.push(it);
        m.sets.push(s);
    }
    m.items.sort_by_cached_key(|i| i.key.to_lowercase());
    m.invalidate_views();
    // What a frame reads that a plan invalidates: the Sets column's views and Problems (which reads the export blocks
    // and asks).
    let frame = |m: &AppModel| {
        let n = problems_builds(m);
        let t = Instant::now();
        let v = views(m);
        let p = problems(m);
        let e = t.elapsed();
        assert_eq!(problems_builds(m), n + 1, "the list was built");
        assert!(!p.is_empty() && v.len() > N);
        e
    };
    let full = (0..3)
        .map(|_| {
            m.invalidate_views();
            frame(&m)
        })
        .min()
        .unwrap();
    let (mut splice, mut partial) = (Duration::MAX, Duration::MAX);
    for p in [AlphaPolicy::Cutout, AlphaPolicy::Blend, AlphaPolicy::Cutout] {
        m.set_field(LEAF, "alpha.policy", json!(p));
        let (partials, _full) = collect(&mut m, 1, 0);
        let t = Instant::now();
        for x in partials {
            m.handle_held(x);
        }
        splice = splice.min(t.elapsed());
        assert_eq!(policy(&m, LEAF), Some(p), "the partial landed");
        partial = partial.min(frame(&m));
        m.poll();
    }
    eprintln!(
        "{N} sets: frame after a full plan {full:?}; after a partial: apply {splice:?}, frame {partial:?}"
    );
    assert!(
        partial * 10 < full,
        "the frame after a partial took {partial:?}, not under a tenth of the {full:?} after a full plan"
    );
}

#[test]
fn a_global_edit_sends_no_partial() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open(d.path(), |_| {});
    // The export folder is outside `[files]`: the request differs from the landed one.
    m.set_export_folder(d.path().join("export2"));
    assert!(!m.partial_pending);
    let (partials, full) = collect(&mut m, 0, 1);
    assert!(partials.is_empty());
    assert_eq!(
        m.fix_off().as_deref(),
        Some(WAIT),
        "a global edit waits for the full plan"
    );
    finish(&mut m, full);
    // A `[files]` edit after it landed sends one again.
    m.hold_plan_results = true;
    cutout(&mut m, LEAF);
    assert!(m.partial_pending);
    let (_, full) = collect(&mut m, 1, 1);
    finish(&mut m, full);
}
