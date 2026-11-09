//! The two read-only queries `sdc uv` uses (Note IB): the texture paths in use, and the winning shapes of one texture.
use modscan::store::Store;

fn path_id(c: &rusqlite::Connection, p: &str) -> i64 {
    c.execute("INSERT OR IGNORE INTO path(p) VALUES (?1)", [p])
        .unwrap();
    c.query_row("SELECT id FROM path WHERE p = ?1", [p], |r| r.get(0))
        .unwrap()
}

fn origin(c: &rusqlite::Connection, id: i64, prio: i32, derived: bool) {
    c.execute(
        "INSERT INTO origin(id, kind, name, root, priority, enabled, derived_by) VALUES (?1, 1, ?2, ?2, ?3, 1, ?4)",
        rusqlite::params![id, format!("o{id}"), prio, derived.then_some(1)],
    )
    .unwrap();
}

/// A NIF copy with one shape per (block, shader_kind, texset).
fn copy(
    c: &rusqlite::Connection,
    id: i64,
    nif: &str,
    origin: i64,
    derived: bool,
    shapes: &[(u32, i32, i64)],
) {
    let p = path_id(c, nif);
    c.execute(
        "INSERT INTO nif_copy(id, path_id, origin_id, parse_status, le_mesh, is_derived) VALUES (?1, ?2, ?3, 0, 0, ?4)",
        rusqlite::params![id, p, origin, derived],
    )
    .unwrap();
    for (b, kind, set) in shapes {
        c.execute(
            "INSERT INTO shape(copy_id, block, kind, shader_kind, shader_type, slsf2, texset_id) VALUES (?1, ?2, 'BSTriShape', ?3, 0, 0, ?4)",
            rusqlite::params![id, b, kind, set],
        )
        .unwrap();
    }
}

fn winner(c: &rusqlite::Connection, nif: &str, copy: i64, src: Option<i64>) {
    let p = path_id(c, nif);
    c.execute(
        "INSERT INTO nif_winner(path_id, copy_id, n_copies) VALUES (?1, ?2, 1)",
        rusqlite::params![p, copy],
    )
    .unwrap();
    if let Some(s) = src {
        c.execute(
            "INSERT INTO nif_winner_src(path_id, copy_id) VALUES (?1, ?2)",
            rusqlite::params![p, s],
        )
        .unwrap();
    }
}

#[test]
fn winning_shapes_cover_any_lighting_slot_and_the_effect_source_only() {
    let d = tempfile::tempdir().unwrap();
    let s = Store::open(&d.path().join("scan.db")).unwrap();
    let c = s.conn();
    let (a, b) = (path_id(c, "textures/a.dds"), path_id(c, "textures/b.dds"));
    // Set 1: lighting, a in slot 0 and b in slot 2. Set 2: used by an effect shape, a is the source, b the greyscale.
    c.execute_batch(&format!(
        "INSERT INTO texset(id, n, t0, t1, t2) VALUES (1, 3, {a}, 0, {b});
         INSERT INTO texset(id, n, t0, t1) VALUES (2, 2, {a}, {b});
         INSERT INTO tex_use(tex_path_id, texset_id, slot) VALUES ({a}, 1, 0), ({b}, 1, 2), ({a}, 2, 0), ({b}, 2, 1);"
    ))
    .unwrap();
    origin(c, 1, 1, false);
    origin(c, 2, 10, false);
    origin(c, 3, 5, false);
    origin(c, 4, 20, true);
    // x.nif: a loser copy (id 10) and the winner (id 11): block 0 lighting, block 1 effect.
    // Set 3 holds `lose`, used only by the loser copy; set 4 holds `grey`, used only as an effect shape's greyscale.
    let (lose, grey) = (
        path_id(c, "textures/lose.dds"),
        path_id(c, "textures/grey.dds"),
    );
    c.execute_batch(&format!(
        "INSERT INTO texset(id, n, t0) VALUES (3, 1, {lose});
         INSERT INTO texset(id, n, t1) VALUES (4, 2, {grey});
         INSERT INTO tex_use(tex_path_id, texset_id, slot) VALUES ({lose}, 3, 0), ({grey}, 4, 1);"
    ))
    .unwrap();
    copy(c, 10, "meshes/x.nif", 1, false, &[(0, 1, 1), (5, 1, 3)]);
    copy(
        c,
        11,
        "meshes/x.nif",
        2,
        false,
        &[(0, 1, 1), (1, 2, 2), (2, 2, 4)],
    );
    winner(c, "meshes/x.nif", 11, None);
    // y.nif: a derived winner (id 21) whose source copy (id 20) holds the shape.
    copy(c, 20, "meshes/y.nif", 3, false, &[(4, 1, 1)]);
    copy(c, 21, "meshes/y.nif", 4, true, &[]);
    winner(c, "meshes/y.nif", 21, Some(20));

    // `lose` (a losing copy only) and `grey` (an effect greyscale slot only) are not candidates.
    assert_eq!(
        s.used_texture_paths().unwrap(),
        ["textures/a.dds", "textures/b.dds"]
    );
    // a: slot 0 of the lighting shape and the effect source; the loser copy is not listed.
    assert_eq!(
        s.winning_shapes_of_texture("textures/a.dds").unwrap(),
        [
            (11, "meshes/x.nif".to_string(), 0),
            (11, "meshes/x.nif".to_string(), 1),
            (20, "meshes/y.nif".to_string(), 4),
        ]
    );
    // b: slot 2 of the lighting shape counts; slot 1 of the effect shape (greyscale) does not.
    assert_eq!(
        s.winning_shapes_of_texture("textures/b.dds").unwrap(),
        [
            (11, "meshes/x.nif".to_string(), 0),
            (20, "meshes/y.nif".to_string(), 4),
        ]
    );
    assert!(s
        .winning_shapes_of_texture("textures/none.dds")
        .unwrap()
        .is_empty());
}
