use std::process::Command;

fn sdc() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_sdc"));
    // Keep the developer's real user config out of the tests.
    c.env(
        "APPDATA",
        std::env::temp_dir().join("sdc-test-appdata-none"),
    );
    c
}

#[test]
fn presets_list_and_show() {
    let (code, out, _) = stdout_of(&["presets", "list"]);
    assert_eq!(code, 0);
    assert!(
        out.contains("cs-quality") && out.contains("vanilla-quality"),
        "{out}"
    );
    let (code, out, _) = stdout_of(&["presets", "show", "cs-quality", "--subtype", "pbr_albedo"]);
    assert_eq!(code, 0);
    assert!(
        out.contains("\"tag\": \"srgb\"") && out.contains("builtin"),
        "{out}"
    );
    assert_eq!(stdout_of(&["presets", "show", "nope"]).0, 2);
}

fn stdout_of(args: &[&str]) -> (i32, String, String) {
    let o = sdc().args(args).output().unwrap();
    (
        o.status.code().unwrap(),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

#[test]
fn presets_list_json_has_one_object_per_preset_with_aliases() {
    let (code, out, _) = stdout_of(&["presets", "list", "--json"]);
    assert_eq!(code, 0);
    let rows: Vec<serde_json::Value> = out
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let ids: Vec<&str> = rows.iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert!(
        ids.contains(&"cs-quality") && ids.contains(&"vanilla-quality"),
        "{ids:?}"
    );
    assert!(!ids.contains(&"sdc-base"), "hidden presets are not listed");
    assert!(rows.iter().all(|r| r["suffix_aliases"].is_object()));
}

#[test]
fn presets_show_reports_provenance_and_rejects_bad_arguments() {
    let (code, out, _) = stdout_of(&["presets", "show", "cs-quality", "--subtype", "normal"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["subtype"], "normal");
    assert!(
        v["provenance"].as_object().is_some_and(|m| !m.is_empty()),
        "{out}"
    );
    assert_eq!(
        stdout_of(&["presets", "show", "cs-quality", "--subtype", "nope"]).0,
        2
    );
    // Without --subtype every sub-type is shown, one object each.
    let (code, out, _) = stdout_of(&["presets", "show", "vanilla-quality", "--target", "vanilla"]);
    assert_eq!(code, 0);
    assert!(out.matches("\"provenance\"").count() >= 20, "{out}");
}

#[test]
fn presets_schema_prints_one_schema_or_writes_all() {
    let (code, out, _) = stdout_of(&["presets", "schema", "event"]);
    assert_eq!(code, 0);
    let committed = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schema/event.schema.json"
    ))
    .unwrap();
    assert_eq!(out, committed.replace("\r\n", "\n"));
    // `all` without a folder is a usage error; with one it writes every committed file.
    assert_eq!(stdout_of(&["presets", "schema", "all"]).0, 2);
    assert_eq!(stdout_of(&["presets", "schema", "bogus"]).0, 2);
    let d = tempfile::tempdir().unwrap();
    let dir = d.path().join("out");
    let (code, _, _) = stdout_of(&["presets", "schema", "all", "--out", dir.to_str().unwrap()]);
    assert_eq!(code, 0);
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 7);
    let (code, out, _) = stdout_of(&[
        "presets",
        "schema",
        "dds-validation",
        "--out",
        dir.to_str().unwrap(),
    ]);
    assert_eq!(
        (code, out.contains("dds-validation.schema.json")),
        (0, true)
    );
}

fn sdc_in(appdata: &std::path::Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_sdc"))
        .env("APPDATA", appdata)
        .args(args)
        .output()
        .unwrap();
    (
        o.status.code().unwrap(),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

const MINE: &str = "schema_version = 1\nid = \"mine\"\nversion = \"1.0.0\"\nbase = \"cs-quality\"\n[subtypes.diffuse]\nformat = \"bc1\"\n";

fn appdata_with_mine() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("sdc/presets")).unwrap();
    std::fs::write(d.path().join("sdc/presets/mine.toml"), MINE).unwrap();
    std::fs::write(d.path().join("sdc/presets/broken.toml"), "nope").unwrap();
    d
}

/// Spec §4: `list` shows a folder preset with its source; `--json` has `source` on every row.
#[test]
fn list_shows_user_presets_with_their_source() {
    let d = appdata_with_mine();
    let (code, out, err) = sdc_in(d.path(), &["presets", "list"]);
    assert_eq!(code, 0, "{err}");
    let line = out.lines().find(|l| l.starts_with("mine")).unwrap();
    assert!(line.contains("mine.toml"), "{line}");
    assert!(
        out.contains("cs-compact") && out.contains("fast-preview"),
        "{out}"
    );
    assert!(
        err.contains("CFG-PRESET-SKIPPED") && err.contains("broken.toml"),
        "{err}"
    );
    let (_, out, _) = sdc_in(d.path(), &["presets", "list", "--json"]);
    let rows: Vec<serde_json::Value> = out
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let src = |id: &str| rows.iter().find(|r| r["id"] == id).unwrap()["source"].clone();
    assert_eq!(src("cs-quality"), "builtin");
    assert!(src("mine").as_str().unwrap().ends_with("mine.toml"));
}

/// Spec §3: `--preset-file` works on every verb, and a duplicate id is an error (exit 2).
#[test]
fn preset_file_on_the_verbs_and_duplicates() {
    let d = appdata_with_mine();
    let extra = d.path().join("extra.toml");
    std::fs::write(&extra, MINE.replace("\"mine\"", "\"extra\"")).unwrap();
    let (code, out, _) = sdc_in(
        d.path(),
        &["presets", "list", "--preset-file", extra.to_str().unwrap()],
    );
    assert_eq!(code, 0);
    assert!(out.lines().any(|l| l.starts_with("extra")), "{out}");
    let dup = d.path().join("dup.toml");
    std::fs::write(&dup, MINE).unwrap();
    let (code, _, err) = sdc_in(
        d.path(),
        &[
            "presets",
            "show",
            "mine",
            "--preset-file",
            dup.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("already loaded"), "{err}");
}

/// M6 Track D spec §8: the CLI help names the user presets folder.
#[test]
fn help_names_the_user_presets_folder() {
    for args in [&["presets", "--help"][..], &["convert", "--help"]] {
        let (code, out, _) = stdout_of(args);
        assert_eq!(code, 0);
        assert!(out.contains(r"%APPDATA%\sdc\presets"), "{args:?}: {out}");
    }
}

/// Note FM43: `presets show` gives the effort a BC6H sub-type encodes at (`slowest`) and says why; the stored preset
/// value (`veryslow`, from the base) is what the other sub-types show, and a preset that sets the effort keeps it.
#[test]
fn presets_show_gives_the_bc6h_default_effort_and_its_reason() {
    let show = |args: &[&str]| -> serde_json::Value {
        let mut a = vec!["presets", "show"];
        a.extend_from_slice(args);
        let (code, out, err) = stdout_of(&a);
        assert_eq!(code, 0, "{err}");
        serde_json::from_str(&out).unwrap()
    };
    for st in ["pbr_emissive", "hdr_sky"] {
        let v = show(&["cs-quality", "--subtype", st]);
        assert_eq!(v["profile"]["encoder"]["effort"], "slowest", "{st}");
        assert_eq!(
            v["provenance"]["encoder.effort"], "builtin (BC6H default)",
            "{st}"
        );
        // Every field the effort row shares with the stored value is untouched.
        assert_eq!(v["profile"]["format"], "bc6h_uf16", "{st}");
    }
    let v = show(&["cs-quality", "--subtype", "diffuse"]);
    assert_eq!(v["profile"]["encoder"]["effort"], "veryslow");
    assert_eq!(v["provenance"]["encoder.effort"], "builtin");
    // fast-preview sets the effort itself: it is not the BC6H default.
    let v = show(&["fast-preview", "--subtype", "pbr_emissive"]);
    assert_eq!(v["profile"]["encoder"]["effort"], "fast");
    assert_ne!(v["provenance"]["encoder.effort"], "builtin (BC6H default)");
}
