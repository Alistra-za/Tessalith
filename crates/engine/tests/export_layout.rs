//! Note HQ: the export always has the `textures` folder. A request whose input root is at or inside a `textures`
//! folder plans under `<export>\<that textures folder down to the root>`; `rel` and set ids stay as they were.
//! Every test plans and reads where the jobs write.
use engine::api::*;
use engine::config::Target;
use engine::events::*;
use engine::sets::OutputKey;
use image::{ImageBuffer, Rgb};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn png(p: &Path) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    ImageBuffer::from_fn(16, 16, |x, y| Rgb([x as u8 * 8, y as u8 * 8, 90]))
        .save(p)
        .unwrap();
}

fn request(root: &Path, export: &Path) -> BatchRequest {
    let mut req = BatchRequest::new(vec![root.to_path_buf()], export.to_path_buf(), Target::Cs);
    req.recursive = true;
    req.overrides = vec![engine::overrides::parse_set("file:**/*.png.subtype=diffuse").unwrap()];
    req
}

fn plan(req: BatchRequest) -> Plan {
    let rep = Reporter::new(Arc::new(VecSink::default()), new_batch_id());
    Engine::new()
        .unwrap()
        .plan(req, &rep, &CancelToken::default())
        .unwrap()
}

/// Where the planned jobs write, sorted.
fn outputs(req: BatchRequest) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = plan(req).jobs.into_iter().map(|j| j.output).collect();
    v.sort();
    v
}

/// HQ1: a root that is a `textures` folder, or lies inside one, writes under the export's own `textures\...`, and
/// discovery skips the whole export folder; a mod folder root keeps the export folder, as its `rel` has `textures`.
#[test]
fn a_textures_root_plans_under_the_exports_textures_folder() {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("out");
    png(&d.path().join("Mod/textures/armor/x.png"));
    let want = vec![out.join("textures/armor/x.dds")];
    let tex = d.path().join("Mod/textures");
    assert_eq!(outputs(request(&tex, &out)), want);
    let armor = tex.join("armor");
    let r = request(&armor, &out);
    assert_eq!(r.skip_root.as_deref(), Some(out.as_path()));
    assert_eq!(outputs(r), want);
    assert_eq!(outputs(request(&d.path().join("Mod"), &out)), want);
}

/// HQ1: the folder names keep their case from disk, whatever case the root was given in.
#[test]
fn the_folder_case_comes_from_disk() {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("out");
    png(&d.path().join("Mod/Textures/Armor/x.png"));
    let r = request(&d.path().join("Mod/textures/armor"), &out);
    assert_eq!(r.output_root, out.join("Textures").join("Armor"));
    let got = outputs(r);
    let comps: Vec<String> = got[0]
        .strip_prefix(&out)
        .unwrap()
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    assert_eq!(comps, ["Textures", "Armor", "x.dds"]);
}

/// HQ2: no `textures` folder anywhere: nothing is added, and nothing extra is skipped.
#[test]
fn a_root_without_textures_is_unchanged() {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("out");
    png(&d.path().join("src/x.png"));
    let r = request(&d.path().join("src"), &out);
    assert_eq!((&r.output_root, &r.skip_root), (&out, &None));
    assert_eq!(outputs(r), vec![out.join("x.dds")]);
}

/// HQ3 and HQ5: in place, or with the export folder an ancestor of the root (inside its `textures` folders or above
/// them), a DDS lands beside its source, and discovery skips nothing (the export folder holds every input).
#[test]
fn in_place_writes_beside_the_source() {
    let d = tempfile::tempdir().unwrap();
    png(&d.path().join("Mod/textures/armor/x.png"));
    let armor = d.path().join("Mod/textures/armor");
    let beside = vec![armor.join("x.dds")];
    for export in [
        armor.clone(),
        d.path().join("Mod/textures"),
        d.path().join("Mod"),
    ] {
        let r = request(&armor, &export);
        assert_eq!(r.skip_root, None, "{export:?}");
        assert_eq!(outputs(r), beside, "{export:?}");
    }
    let tex = d.path().join("Mod/textures");
    assert_eq!(outputs(request(&tex, &tex)), vec![tex.join("armor/x.dds")]);
}

/// HQ1, HQ7: a planned texture lands at its game path under the export folder, and its set id is the same as before
/// (relative to the output root, without `textures`).
#[test]
fn outputs_move_and_set_ids_stay() {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("out");
    let tex = d.path().join("Mod/textures");
    png(&tex.join("armor/iron.png"));
    let p = plan(request(&tex, &out));
    let outputs: Vec<&PathBuf> = p.jobs.iter().map(|j| &j.output).collect();
    assert_eq!(outputs, vec![&out.join("textures/armor/iron.dds")]);
    let ids: Vec<String> = p
        .shaders
        .keys()
        .map(|k| engine::sets::set_id(&p.request.output_root, k))
        .collect();
    assert_eq!(ids, vec!["armor/iron".to_string()]);
}

/// HQ6 (fix round 1): explicit output paths are under the export folder, so one may leave the moved output root
/// (a PGPatcher `textures\pbr\...` prefix for a root inside `textures`).
#[test]
fn explicit_paths_are_under_the_export_folder() {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("out");
    let armor = d.path().join("Mod/textures/armor");
    png(&armor.join("iron.png"));
    let mut r = request(&armor, &out);
    assert_eq!(r.export_root.as_deref(), Some(out.as_path()));
    r.output_paths.insert(
        OutputKey::File("iron.png".into()),
        PathBuf::from("textures/pbr/armor/iron.dds"),
    );
    assert_eq!(outputs(r), vec![out.join("textures/pbr/armor/iron.dds")]);
}

/// HQ1: an `[inputs]` alias target under a `textures` root lands under the export's `textures` folder too.
#[test]
fn an_alias_target_under_a_textures_root_moves_with_it() {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("out");
    let tex = d.path().join("Mod/textures");
    png(&tex.join("armor/ironnormal.png"));
    std::fs::write(
        tex.join("sdc.toml"),
        "[inputs.\"armor/ironnormal.png\"]\nas = \"armor/iron_n.png\"\n",
    )
    .unwrap();
    let mut r = request(&tex, &out);
    r.overrides.clear();
    let got = outputs(r);
    assert_eq!(got, vec![out.join("textures/armor/iron_n.dds")]);
}

/// Note HS4: an export folder inside the input root is not source layout. After one run its own `textures` folder
/// exists; `root_prefix` must not read it as HQ8's nested `textures` and flip the output root back to the export.
#[test]
fn an_export_folder_inside_the_root_does_not_flip_the_output_root() {
    let d = tempfile::tempdir().unwrap();
    png(&d.path().join("Mod/textures/armor/x.png"));
    let tex = d.path().join("Mod/textures");
    let out = tex.join("out");
    let want = vec![out.join("textures/armor/x.dds")];
    assert_eq!(outputs(request(&tex, &out)), want);
    std::fs::create_dir_all(out.join("textures/armor")).unwrap();
    let r = request(&tex, &out);
    assert_eq!(r.output_root, out.join("textures"));
    assert_eq!(r.skip_root.as_deref(), Some(out.as_path()));
    assert_eq!(outputs(r), want);
    // The first export folder is source layout to the second one.
    std::fs::remove_dir_all(&out).unwrap();
    let out2 = tex.join("armor2/textures");
    std::fs::create_dir_all(&out2).unwrap();
    assert_eq!(request(&tex, &out2).output_root, out2.join("textures"));
}
