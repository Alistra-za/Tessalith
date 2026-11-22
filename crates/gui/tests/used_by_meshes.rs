//! Item 9 (Task 12): the settings panel's "Used by meshes" list, read off the UI thread from the scan database.
mod support;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use sdc_gui::model::used_by::{read_users, UsedBy, USED_BY_CAP};
use sdc_gui::model::AppModel;
use std::path::Path;
use std::time::{Duration, Instant};
use support::*;

/// One NIF copy that uses the texture in slot 0 of a lighting shape: (NIF, mod, origin kind, priority, alpha flags,
/// threshold).
type User<'a> = (&'a str, &'a str, i32, i32, Option<i64>, Option<i64>);

fn db_with(db: &Path, tex: &str, users: &[User]) {
    let s = modscan::store::Store::open(db).unwrap();
    let c = s.conn();
    c.execute("INSERT INTO path(id, p) VALUES (1, ?1)", [tex])
        .unwrap();
    c.execute_batch(
        "INSERT INTO texset(id, n, t0) VALUES (1, 1, 1);
         INSERT INTO tex_use(tex_path_id, texset_id, slot) VALUES (1, 1, 0);",
    )
    .unwrap();
    for (i, (nif, name, kind, prio, flags, thr)) in users.iter().enumerate() {
        let id = i as i64 + 2;
        c.execute(
            "INSERT INTO path(id, p) VALUES (?1, ?2)",
            rusqlite::params![id, nif],
        )
        .unwrap();
        c.execute(
            "INSERT INTO origin(id, kind, name, root, priority, enabled) VALUES (?1, ?2, ?3, ?3, ?4, 1)",
            rusqlite::params![id, kind, name, prio],
        )
        .unwrap();
        c.execute(
            "INSERT INTO nif_copy(id, path_id, origin_id, parse_status, is_derived) VALUES (?1, ?1, ?1, 0, 0)",
            rusqlite::params![id],
        )
        .unwrap();
        c.execute(
            "INSERT INTO shape(copy_id, block, kind, shader_kind, shader_type, slsf2, texset_id, alpha_flags, alpha_threshold) VALUES (?1, 0, 'BSTriShape', 1, 0, 0, 1, ?2, ?3)",
            rusqlite::params![id, flags, thr],
        )
        .unwrap();
    }
}

fn read(users: &[User], alpha: bool) -> UsedBy {
    let d = tempfile::tempdir().unwrap();
    let db = d.path().join("scan.db");
    db_with(&db, "textures/a/wall.dds", users);
    read_users(&db, "a/wall.png", "textures/a/wall.dds", alpha)
}

fn order(u: &UsedBy) -> Vec<(&str, &str, i32)> {
    u.rows
        .iter()
        .map(|r| (r.nif.as_str(), r.mod_name.as_str(), r.priority))
        .collect()
}

#[test]
fn rows_are_sorted_by_mo2_priority_winner_first_and_loose_beats_bsa() {
    let u = read(
        &[
            ("meshes/a.nif", "Low", 1, 3, None, None),
            ("meshes/c.nif", "Skyrim - Meshes0.bsa", 4, 50, None, None),
            ("meshes/b.nif", "High", 1, 10, None, None),
            ("meshes/d.nif", "Overwrite", 2, i32::MAX, None, None),
        ],
        false,
    );
    assert_eq!(u.error, None);
    assert_eq!(
        order(&u),
        vec![
            ("meshes/d.nif", "Overwrite", i32::MAX),
            ("meshes/b.nif", "High", 10),
            ("meshes/a.nif", "Low", 3),
            ("meshes/c.nif", "Skyrim - Meshes0.bsa", 50),
        ]
    );
    assert!(u.rows[3].bsa && !u.rows[0].bsa);
    assert_eq!(u.more, 0);
}

#[test]
fn the_meshes_that_decided_the_alpha_are_marked() {
    let decided = |u: &UsedBy| -> Vec<String> {
        u.rows
            .iter()
            .filter(|r| r.decided_alpha)
            .map(|r| r.nif.clone())
            .collect()
    };
    // Blend anywhere decides blend: the blending mesh is marked, the plain one is not.
    let users = [
        ("meshes/a.nif", "A", 1, 1, Some(1), Some(0)),
        ("meshes/b.nif", "B", 1, 2, None, None),
    ];
    assert_eq!(decided(&read(&users, true)), vec!["meshes/a.nif"]);
    // The cutout threshold is the largest: the mesh testing at 200 decided it, the one at 100 did not.
    let users = [
        ("meshes/a.nif", "A", 1, 1, Some(1 << 9), Some(100)),
        ("meshes/b.nif", "B", 1, 2, Some(1 << 9), Some(200)),
    ];
    assert_eq!(decided(&read(&users, true)), vec!["meshes/b.nif"]);
    // The meshes did not decide this texture's alpha (another source did, or it is no colour member): no mark.
    assert!(decided(&read(&users, false)).is_empty());
    // No shape has alpha: "remove" was decided by all of them alike, so none is singled out.
    let users = [("meshes/a.nif", "A", 1, 1, None, None)];
    assert!(decided(&read(&users, true)).is_empty());
}

#[test]
fn the_list_stops_at_200_and_counts_the_rest() {
    let names: Vec<String> = (0..USED_BY_CAP + 5)
        .map(|i| format!("meshes/m{i:04}.nif"))
        .collect();
    let users: Vec<User> = names
        .iter()
        .map(|n| (n.as_str(), n.as_str(), 1, 1, None, None))
        .collect();
    let u = read(&users, false);
    assert_eq!((u.rows.len(), u.more), (USED_BY_CAP, 5));
    assert_eq!(USED_BY_CAP, 200);
}

#[test]
fn same_name_copies_count_toward_the_cap() {
    let d = tempfile::tempdir().unwrap();
    let db = d.path().join("scan.db");
    let tex = "textures/a/wall.dds";
    let names: Vec<_> = (0..USED_BY_CAP)
        .map(|i| format!("meshes/m{i:04}.nif"))
        .collect();
    let users: Vec<User> = names
        .iter()
        .map(|n| (n.as_str(), n.as_str(), 1, 1, None, None))
        .collect();
    db_with(&db, tex, &users);
    let s = modscan::store::Store::open(&db).unwrap();
    s.conn()
        .execute_batch(
            "INSERT INTO origin(id, kind, name, root, priority, enabled)
             SELECT 202, kind, name, 'second-root', priority, enabled FROM origin WHERE id = 2;
         INSERT INTO nif_copy(id, path_id, origin_id, parse_status, is_derived) VALUES (202, 2, 202, 0, 0);
         INSERT INTO shape(copy_id, block, kind, shader_kind, shader_type, slsf2, texset_id)
             VALUES (202, 0, 'BSTriShape', 1, 0, 0, 1);",
        )
        .unwrap();
    let copies: i64 = s
        .conn()
        .query_row(
            "SELECT count(DISTINCT s.copy_id) FROM shape s JOIN tex_use t ON t.texset_id = s.texset_id
         WHERE t.tex_path_id = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(copies, 201);
    drop(s);
    let u = read_users(&db, "a/wall.png", tex, false);
    assert_eq!(u.error, None);
    assert_eq!(u.more, 1);
}

#[test]
fn a_row_knows_its_shapes() {
    let d = tempfile::tempdir().unwrap();
    let db = d.path().join("scan.db");
    db_with(
        &db,
        "textures/a/wall.dds",
        &[("meshes/tree.nif", "Trees", 1, 7, None, None)],
    ); // copy id 2, block 0
    {
        let s = modscan::store::Store::open(&db).unwrap();
        s.conn().execute_batch(
            "INSERT INTO shape(copy_id, block, name, kind, shader_kind, shader_type, slsf2, texset_id) VALUES (2, 5, 'Body', 'BSTriShape', 1, 0, 0, 1);
             INSERT INTO shape(copy_id, block, name, kind, shader_kind, shader_type, slsf2, texset_id) VALUES (2, 9, NULL, 'BSTriShape', 1, 0, 0, 1);
             INSERT INTO origin(id, kind, name, root, priority, enabled) VALUES (3, 1, 'Trees', 'other-root', 6, 1);
             INSERT INTO nif_copy(id, path_id, origin_id, parse_status, is_derived) VALUES (3, 2, 3, 0, 0);
             INSERT INTO shape(copy_id, block, name, kind, shader_kind, shader_type, slsf2, texset_id) VALUES (3, 5, 'Body', 'BSTriShape', 1, 0, 0, 1);
             INSERT INTO tex_use(tex_path_id, texset_id, slot) VALUES (1, 1, 1);",
        )
        .unwrap();
    }
    let u = read_users(&db, "a/wall.png", "textures/a/wall.dds", false);
    assert_eq!(
        (u.total_copies, u.rows.len(), u.rows[0].copies.len()),
        (2, 1, 2)
    );
    let copies = &u.rows[0].copies;
    assert_eq!((copies[0].copy_id, copies[1].copy_id), (2, 3));
    assert_eq!(
        copies[0].shapes,
        vec![(0, None), (5, Some("Body".into())), (9, None)]
    );
    assert_eq!(copies[1].shapes, vec![(5, Some("Body".into()))]);
}

#[test]
fn a_losing_copy_names_the_actual_winner_even_when_it_uses_another_texture() {
    let d = tempfile::tempdir().unwrap();
    let db = d.path().join("scan.db");
    // Copies 2 (Low, priority 1) and 3 (Mid, 2) use the texture; copy 9 in "Boss" wins the path but uses none.
    db_with(
        &db,
        "textures/a/wall.dds",
        &[("meshes/tree.nif", "Low", 1, 1, None, None)],
    );
    {
        let s = modscan::store::Store::open(&db).unwrap();
        s.conn()
            .execute_batch(
                "INSERT INTO origin(id, kind, name, root, priority, enabled) VALUES (3, 1, 'Mid', 'mid', 2, 1);
                 INSERT INTO nif_copy(id, path_id, origin_id, parse_status, is_derived) VALUES (3, 2, 3, 0, 0);
                 INSERT INTO shape(copy_id, block, kind, shader_kind, shader_type, slsf2, texset_id) VALUES (3, 0, 'BSTriShape', 1, 0, 0, 1);
                 INSERT INTO origin(id, kind, name, root, priority, enabled) VALUES (9, 1, 'Boss', 'boss', 9, 1);
                 INSERT INTO nif_copy(id, path_id, origin_id, parse_status, is_derived) VALUES (9, 2, 9, 0, 0);
                 INSERT INTO nif_winner(path_id, copy_id, n_copies) VALUES (2, 9, 3);",
            )
            .unwrap();
    }
    let u = read_users(&db, "a/wall.png", "textures/a/wall.dds", false);
    assert_eq!(u.error, None);
    let by: Vec<_> = u
        .rows
        .iter()
        .flat_map(|r| r.copies.iter())
        .map(|c| (c.copy_id, c.overridden_by.as_deref()))
        .collect();
    assert_eq!(by, vec![(3, Some("Boss")), (2, Some("Boss"))]);
    // The winner itself reads no annotation.
    {
        let s = modscan::store::Store::open(&db).unwrap();
        s.conn()
            .execute("UPDATE nif_winner SET copy_id = 3", [])
            .unwrap();
    }
    let u = read_users(&db, "a/wall.png", "textures/a/wall.dds", false);
    let by: Vec<_> = u
        .rows
        .iter()
        .flat_map(|r| r.copies.iter())
        .map(|c| (c.copy_id, c.overridden_by.as_deref()))
        .collect();
    assert_eq!(by, vec![(3, None), (2, Some("Mid"))]);
}

#[test]
fn alpha_evidence_past_the_cap_still_decides() {
    // 200 copies at priority 5 fill the list, the first of them cutting out at 100. The one copy cutting out at 200
    // has priority 1 and sorts past the cap, but its evidence decides the threshold: the listed copy is not marked.
    let names: Vec<String> = (0..USED_BY_CAP)
        .map(|i| format!("meshes/m{i:04}.nif"))
        .collect();
    let mut users: Vec<User> = names
        .iter()
        .map(|n| (n.as_str(), n.as_str(), 1, 5, None, None))
        .collect();
    users[0].4 = Some(1 << 9);
    users[0].5 = Some(100);
    users.push(("meshes/z.nif", "Z", 1, 1, Some(1 << 9), Some(200)));
    let u = read(&users, true);
    assert_eq!(
        (u.rows.len(), u.more, u.total_copies),
        (USED_BY_CAP, 1, USED_BY_CAP + 1)
    );
    assert!(u.rows.iter().all(|r| r.nif != "meshes/z.nif"));
    assert!(
        u.rows.iter().all(|r| !r.decided_alpha),
        "the threshold came from the copy past the cap"
    );
}

fn wait_used_by(m: &mut AppModel, key: &str) -> std::sync::Arc<UsedBy> {
    let t = Instant::now();
    loop {
        m.poll();
        if let Some(Some(u)) = m.used_by_for(key) {
            return u;
        }
        assert!(
            t.elapsed() < Duration::from_secs(30),
            "the read never landed"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The scan key of an item, as the plan names its output (the key the read asks for).
fn tex_of(m: &AppModel, key: &str) -> String {
    let j = m.item(key).unwrap().job.clone().unwrap();
    let stem = j.output.file_stem().unwrap().to_string_lossy().into_owned();
    format!("{}.dds", engine::shader::mesh_key(&j.game_path, &stem))
}

#[test]
fn no_scan_database_hides_the_list_and_a_database_reads_it_off_the_ui_thread() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let key = "landscape/leaf.png";
    m.select_item(key);
    assert!(m.used_by_for(key).is_none(), "no database: hidden");
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(eframe::egui::vec2(1400.0, 900.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    h.run();
    assert!(h.query_by_label_contains("Used by meshes").is_none());
    let tex = tex_of(h.state(), key);
    let dbdir = tempfile::tempdir().unwrap();
    let db = dbdir.path().join("scan.db");
    db_with(
        &db,
        &tex,
        &[("meshes/tree.nif", "Trees", 1, 7, Some(1), Some(0))],
    );
    h.state_mut().scan_db = Some(db);
    let u = wait_used_by(h.state_mut(), key);
    assert_eq!(order(&u), vec![("meshes/tree.nif", "Trees", 7)]);
    h.run();
    // The card's likely-type buttons (Task 11) take a row, so the header sits under the clip: scroll to it as a user
    // would before the click.
    h.get_by_label_contains("Used by meshes (1)").scroll_to_me();
    h.run();
    h.get_by_label_contains("Used by meshes (1)").click();
    h.run();
    h.get_by_label_contains("meshes/tree.nif");
    h.get_by_label_contains("Trees · priority 7");
    // Note GC test 6: the link asks the app for the Meshes view, cut to this texture's users.
    // The link's node carries a label and an empty value, which `get_by_label` skips on a Label role.
    // With the system fonts (since the egui 0.37 upgrade; before it this harness drew no glyphs, so the panel was
    // 110+ points shorter) the link sits at y 880-897, under the panel's clip at 860: scroll to it as a user would.
    h.get_by(|n| n.label().as_deref() == Some("Show in Meshes"))
        .scroll_to_me();
    h.run();
    h.get_by(|n| n.label().as_deref() == Some("Show in Meshes"))
        .click();
    h.run();
    assert_eq!(h.state().show_in_meshes.as_deref(), Some(tex.as_str()));
}

/// Note GC test 6, the app's half: the request switches to the Meshes view and sets the texture chip, on a clean
/// list ("Any mod", no search, All), and is taken once.
#[test]
fn show_in_meshes_switches_the_view_and_sets_the_texture_chip() {
    use sdc_gui::model::meshes::NifFilter;
    let d = tempfile::tempdir().unwrap();
    let mut app = sdc_gui::app::SdcApp::new(opened(d.path()));
    let ctx = eframe::egui::Context::default();
    app.meshes.search = "rock".into();
    app.meshes.filter = NifFilter::Problems;
    app.meshes.selected_nif = Some(7);
    app.route_3d(&ctx);
    assert!(!sdc_gui::ui::header::meshes_view_active(&ctx));
    app.model.show_in_meshes = Some("textures/a/wall.dds".into());
    app.route_3d(&ctx);
    assert!(sdc_gui::ui::header::meshes_view_active(&ctx));
    let m = &app.meshes;
    assert_eq!(
        m.tex_filter.as_ref().map(|(t, _)| t.as_str()),
        Some("textures/a/wall.dds")
    );
    assert_eq!(
        (m.selected_mod, m.search.as_str(), m.filter),
        (None, "", NifFilter::All)
    );
    assert_eq!(m.selected_nif, Some(7), "never deselects");
    assert!(app.model.show_in_meshes.is_none());
}

#[test]
fn a_stale_read_is_dropped() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let dbdir = tempfile::tempdir().unwrap();
    let db = dbdir.path().join("scan.db");
    db_with(&db, "textures/none.dds", &[]);
    m.scan_db = Some(db);
    assert_eq!(m.used_by_for("landscape/leaf.png"), Some(None), "reading");
    let first = m.used_by.generation;
    // Another texture is asked for before the first read lands: the first answer is stale.
    assert_eq!(m.used_by_for("landscape/rock01.png"), Some(None));
    let stale = UsedBy {
        item: "landscape/leaf.png".into(),
        ..UsedBy::default()
    };
    m.used_by_landed(first, stale);
    assert!(m.used_by.current.is_none(), "the stale answer is dropped");
    let u = wait_used_by(&mut m, "landscape/rock01.png");
    assert_eq!(u.item, "landscape/rock01.png");
    // A finished scan reads again (the old list stays until the new one lands).
    m.refresh_mesh_votes(false);
    assert!(m.used_by.current.is_some());
    assert_eq!(
        m.used_by_for("landscape/rock01.png").map(|u| u.is_some()),
        Some(true)
    );
    assert!(m.used_by.pending.is_some(), "a new read is under way");
}

#[test]
fn a_panicked_read_shows_its_error_instead_of_reading_for_good() {
    use sdc_gui::worker::WorkerMsg;
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let key = "landscape/rock01.png";
    // The read in flight, as `used_by_for` leaves it (no real thread, so nothing races the panic).
    m.used_by.pending = Some(key.to_string());
    m.sender()
        .send(WorkerMsg::Panicked {
            what: "used-by",
            message: "boom".into(),
        })
        .unwrap();
    m.poll();
    assert!(m.used_by.pending.is_none(), "no longer reading");
    let u = m.used_by.current.clone().expect("the error is shown");
    assert_eq!(u.item, key);
    assert!(
        u.error.as_deref().is_some_and(|e| e.contains("boom")),
        "{u:?}"
    );
}

#[test]
fn stepping_through_textures_starts_one_read_after_the_selection_settles() {
    use sdc_gui::model::used_by::USED_BY_SETTLE;
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let dbdir = tempfile::tempdir().unwrap();
    let db = dbdir.path().join("scan.db");
    db_with(&db, "textures/none.dds", &[]);
    m.scan_db = Some(db);
    // Held Up/Down: a new texture every frame, no read starts.
    for key in [
        "landscape/leaf.png",
        "landscape/rock01.png",
        "landscape/leaf.png",
    ] {
        assert_eq!(m.used_by_for(key), Some(None));
        assert!(m.used_by.pending.is_none(), "{key}: no read yet");
        assert!(m.used_by_pending(), "frames keep coming");
    }
    std::thread::sleep(USED_BY_SETTLE);
    m.used_by_for("landscape/leaf.png");
    assert_eq!(m.used_by.pending.as_deref(), Some("landscape/leaf.png"));
    let u = wait_used_by(&mut m, "landscape/leaf.png");
    assert_eq!(u.item, "landscape/leaf.png");
    assert!(!m.used_by_pending());
}

#[test]
fn the_scan_database_and_show_in_explorer_are_not_statted_every_frame() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let dbdir = tempfile::tempdir().unwrap();
    let db = dbdir.path().join("scan.db");
    db_with(&db, "textures/none.dds", &[]);
    m.scan_db = Some(db);
    let before = m.stats.borrow().stats;
    for _ in 0..5 {
        m.used_by_for("landscape/leaf.png");
        assert_eq!(m.reveal_problem("landscape/rock01.png"), None);
    }
    assert_eq!(m.stats.borrow().stats, before + 2, "one stat per file");
}

#[test]
fn an_idle_window_after_a_folder_change_asks_for_no_frames() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let dbdir = tempfile::tempdir().unwrap();
    let db = dbdir.path().join("scan.db");
    db_with(&db, "textures/none.dds", &[]);
    m.scan_db = Some(db);
    m.used_by_for("landscape/leaf.png");
    std::thread::sleep(sdc_gui::model::used_by::USED_BY_SETTLE);
    wait_used_by(&mut m, "landscape/leaf.png");
    assert!(!m.used_by_pending());
    // Nothing asks again after a scan or another folder: no repaint loop.
    m.refresh_mesh_votes(false);
    assert!(!m.used_by_pending(), "after a scan");
    m.open_folder(mod_folder(&d.path().join("other")));
    assert!(!m.used_by_pending(), "after a folder change");
}

#[test]
fn a_wanted_read_keeps_frames_coming_only_while_it_is_asked_for() {
    use sdc_gui::model::used_by::USED_BY_SETTLE;
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let dbdir = tempfile::tempdir().unwrap();
    let db = dbdir.path().join("scan.db");
    db_with(&db, "textures/none.dds", &[]);
    m.scan_db = Some(db);
    m.used_by_for("landscape/leaf.png");
    // Past the settle, still asked for by each draw: pending until the read starts.
    std::thread::sleep(USED_BY_SETTLE * 2);
    assert!(m.used_by_pending(), "the settle is not missed");
    // Nobody draws it any more (a set got selected): the frames stop.
    std::thread::sleep(std::time::Duration::from_millis(600));
    assert!(!m.used_by_pending());
}

/// Trace-debug wave 2 #6: while a scan runs, or after one was cancelled or failed, the store has every origin
/// disabled and reads empty. A plan then keeps the mesh evidence it has, and "Used by meshes" starts no read.
#[test]
fn a_plan_while_the_scan_store_is_unsettled_keeps_the_mesh_evidence() {
    let settle = |m: &mut AppModel| {
        let t = Instant::now();
        while !m.wait_idle(Duration::from_secs(60)) || m.votes_pending() {
            assert!(t.elapsed() < Duration::from_secs(60), "never idle");
        }
    };
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let dbdir = tempfile::tempdir().unwrap();
    let db = dbdir.path().join("scan.db");
    let tex = tex_of(&m, "landscape/leaf.png");
    db_with(&db, &tex, &[("meshes/a.nif", "A", 1, 1, Some(1), Some(0))]);
    m.scan_db = Some(db.clone());
    m.refresh_mesh_votes(false);
    settle(&mut m);
    let alpha = m.mesh_alpha.clone();
    assert!(!alpha.is_empty(), "the evidence was read");
    let votes = m.mesh_votes.clone();
    // What `begin_scan` does to the store, and what the app records when a scan starts.
    modscan::store::Store::open(&db)
        .unwrap()
        .conn()
        .execute("UPDATE origin SET enabled = 0", [])
        .unwrap();
    m.scan_store_unsettled = true;
    m.replan();
    settle(&mut m);
    assert_eq!(m.mesh_alpha, alpha);
    assert_eq!(m.mesh_votes, votes);
    let key = "landscape/rock01.png";
    let _ = m.used_by_for(key);
    std::thread::sleep(sdc_gui::model::used_by::USED_BY_SETTLE * 2);
    let _ = m.used_by_for(key);
    assert!(
        m.used_by.pending.is_none(),
        "no read of the unsettled store"
    );
}
