use sdc_gui::settings::*;

#[test]
fn defaults_follow_the_spec() {
    let s = GuiSettings::default();
    assert!(!s.live_encode, "preview encoding is off by default");
}

#[test]
fn settings_round_trip_and_bad_files_fall_back_to_defaults() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("sdc/gui.toml");
    let (s, err) = GuiSettings::load(&p);
    assert_eq!((s, err), (GuiSettings::default(), None));
    let mut s = GuiSettings {
        live_encode: true,
        ..GuiSettings::default()
    };
    s.push_recent("C:/a".into());
    s.push_recent("C:/b".into());
    s.push_recent("C:/a".into());
    assert_eq!(
        s.recent,
        vec![std::path::PathBuf::from("C:/a"), "C:/b".into()]
    );
    s.save(&p).unwrap();
    assert_eq!(GuiSettings::load(&p).0, s);
    std::fs::write(&p, "live_encode = \"big\"").unwrap();
    let (s, err) = GuiSettings::load(&p);
    assert_eq!(s, GuiSettings::default());
    assert!(err.is_some());
}

#[test]
fn recent_keeps_ten() {
    let mut s = GuiSettings::default();
    for i in 0..12 {
        s.push_recent(format!("C:/f{i}").into());
    }
    assert_eq!(s.recent.len(), 10);
    assert_eq!(s.recent[0], std::path::PathBuf::from("C:/f11"));
}

/// 2.0 dropped these settings (Note FM25). A 1.x gui.toml that still has their keys loads, the settings that remain
/// keep their values, and the next save leaves the keys out.
#[test]
fn a_1x_gui_toml_still_loads() {
    let removed = [
        "jobs = 6",
        "mem_budget_mb = 2048",
        "bc1_decoder = \"nvidia\"",
        "nvtt_path = \"C:/nvtt/nvtt_export.exe\"",
        "reduce_motion = true",
        "allow_16k_preview = true",
        "preview_max = 16384",
    ];
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("gui.toml");
    std::fs::write(
        &p,
        format!(
            // Final review (GUI M9): the removed `[layout]` keys too.
            "{}\nrecent = [\"C:/mods/a\"]\nlive_encode = true\n[layout]\nmeshes_view = true\nfilter_encoder = \"nvtt\"\npreview_layout = \"split\"\n",
            removed.join("\n")
        ),
    )
    .unwrap();
    let (s, err) = GuiSettings::load(&p);
    assert_eq!(err, None);
    assert_eq!(s.recent, [std::path::PathBuf::from("C:/mods/a")]);
    assert!(s.live_encode);
    assert!(s.layout.meshes_view, "the rest of [layout] still loads");
    s.save(&p).unwrap();
    let saved = std::fs::read_to_string(&p).unwrap();
    for line in removed
        .into_iter()
        .chain(["filter_encoder =", "preview_layout ="])
    {
        let key = line.split(' ').next().unwrap();
        let kept = saved.lines().any(|l| l.starts_with(key));
        assert!(!kept, "{key} is dropped on save: {saved}");
    }
    assert_eq!(GuiSettings::load(&p), (s, None));
}

#[test]
fn saving_is_atomic_and_keeps_an_unreadable_file_as_gui_toml_bad() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("sdc/gui.toml");
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, "live_encode = \"big\"\n# my hand-written notes\n").unwrap();
    let (s, err) = GuiSettings::load(&p);
    assert!(err.is_some());
    s.save(&p).unwrap();
    let bad = d.path().join("sdc/gui.toml.bad");
    assert_eq!(
        std::fs::read_to_string(&bad).unwrap(),
        "live_encode = \"big\"\n# my hand-written notes\n"
    );
    assert_eq!(GuiSettings::load(&p), (s.clone(), None));
    // Once: the next save finds a readable file and leaves the kept copy alone.
    let s2 = GuiSettings {
        live_encode: true,
        ..s
    };
    s2.save(&p).unwrap();
    assert_eq!(GuiSettings::load(&p).0, s2);
    assert!(std::fs::read_to_string(&bad)
        .unwrap()
        .contains("hand-written"));
    let leftovers: Vec<_> = std::fs::read_dir(p.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(leftovers.len(), 2, "no temporary file stays: {leftovers:?}");
}

mod support;

/// Audit 2026-10-15 item 15: a field edited by hand while the app runs survives the app's next save; a field the
/// app changed since its last write wins.
#[test]
fn a_hand_edit_made_while_running_survives_the_next_save() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("sdc/gui.toml");
    GuiSettings::default().save(&p).unwrap();
    let (s, _) = GuiSettings::load(&p);
    let mut m = sdc_gui::model::AppModel::new(s, Some(p.clone()), support::session());
    let mut hand = GuiSettings::load(&p).0;
    hand.export_folder = Some(d.path().join("hand"));
    hand.set_view_3d = true;
    hand.save(&p).unwrap();
    m.settings.live_encode = true;
    m.settings.export_folder = Some(d.path().join("app"));
    m.save_settings();
    let back = GuiSettings::load(&p).0;
    assert!(back.set_view_3d, "the hand edit is kept");
    assert!(back.live_encode, "the app's change is written");
    assert_eq!(
        back.export_folder,
        Some(d.path().join("app")),
        "both changed: the app wins"
    );
    assert_eq!(m.settings, back, "the app now holds what it wrote");
}

/// Audit 2026-10-15 item 9: a failed gui.toml write is a warning toast, once per distinct error.
#[test]
fn a_failed_settings_save_warns_once() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("file"), "x").unwrap();
    let mut m = sdc_gui::model::AppModel::placeholder(GuiSettings::default());
    m.settings_path = Some(d.path().join("file/gui.toml"));
    m.save_settings();
    m.save_settings();
    let n = m
        .messages
        .entries()
        .filter(|e| e.text.starts_with("gui.toml"))
        .count();
    assert_eq!(n, 1);
}

fn running(p: &std::path::Path, s: GuiSettings) -> sdc_gui::model::AppModel {
    s.save(p).unwrap();
    let (s, _) = GuiSettings::load(p);
    sdc_gui::model::AppModel::new(s, Some(p.to_path_buf()), support::session())
}

/// Item 15: a key removed by hand while the app runs stays removed (an optional field becomes unset).
#[test]
fn a_key_removed_by_hand_while_running_stays_removed() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("sdc/gui.toml");
    let mut m = running(
        &p,
        GuiSettings {
            export_folder: Some(d.path().join("out")),
            ..GuiSettings::default()
        },
    );
    let text = std::fs::read_to_string(&p).unwrap();
    let edited: String = text
        .lines()
        .filter(|l| !l.starts_with("export_folder"))
        .map(|l| format!("{l}\n"))
        .collect();
    assert_ne!(edited, text);
    std::fs::write(&p, edited).unwrap();
    m.settings.live_encode = true;
    m.save_settings();
    let back = GuiSettings::load(&p).0;
    assert_eq!(back.export_folder, None);
    assert!(back.live_encode);
    assert_eq!(m.settings.export_folder, None);
}

/// Item 15 and review minor 6: a gui.toml broken by hand while the app runs is kept as `.bad` (with a warning) and
/// the app's settings are written.
#[test]
fn a_file_broken_while_running_is_kept_and_warned_about() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("sdc/gui.toml");
    let mut m = running(&p, GuiSettings::default());
    std::fs::write(&p, "live_encode = [broken").unwrap();
    m.settings.advanced_open = true;
    m.save_settings();
    assert_eq!(GuiSettings::load(&p), (m.settings.clone(), None));
    assert!(GuiSettings::load(&p).0.advanced_open);
    assert_eq!(
        std::fs::read_to_string(p.with_extension("toml.bad")).unwrap(),
        "live_encode = [broken"
    );
    assert!(m
        .messages
        .entries()
        .any(|e| e.text.starts_with("gui.toml: it could not be read")));
}

/// Review minor 2: Recent merges per folder: the hand edit's removal and addition, and the app's addition.
#[test]
fn recent_folders_merge_per_folder() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("sdc/gui.toml");
    let f = |n: &str| std::path::PathBuf::from(format!("C:/m/{n}"));
    let mut m = running(
        &p,
        GuiSettings {
            recent: vec![f("a"), f("b")],
            ..GuiSettings::default()
        },
    );
    let mut hand = GuiSettings::load(&p).0;
    hand.recent = vec![f("a"), f("c")];
    hand.save(&p).unwrap();
    m.settings.push_recent(f("d"));
    m.save_settings();
    assert_eq!(GuiSettings::load(&p).0.recent, [f("d"), f("a"), f("c")]);
}

/// Trace-debug 2026-10-16 #45: a gui.toml that `load` could not read (not UTF-8) is kept as `.bad` by the first
/// save, as one that does not parse is.
#[test]
fn saving_keeps_a_non_utf8_gui_toml_as_bad() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("sdc/gui.toml");
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    let bytes = b"live_encode = true\n# caf\xe9\n".to_vec();
    std::fs::write(&p, &bytes).unwrap();
    let (s, err) = GuiSettings::load(&p);
    assert!(err.is_some());
    let kept = s.save(&p).unwrap();
    assert_eq!(kept, Some(p.with_extension("toml.bad")));
    assert_eq!(std::fs::read(p.with_extension("toml.bad")).unwrap(), bytes);
}

/// #45: a gui.toml locked at launch (load failed, defaults used) is merged by the first save, not replaced.
#[cfg(windows)]
#[test]
fn a_gui_toml_locked_at_launch_is_merged_by_the_first_save() {
    use std::os::windows::fs::OpenOptionsExt;
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("sdc/gui.toml");
    GuiSettings {
        advanced_open: true,
        ..GuiSettings::default()
    }
    .save(&p)
    .unwrap();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&p)
        .unwrap();
    let (s, err) = GuiSettings::load(&p);
    assert!(err.is_some(), "the lock makes load fail");
    let mut m = sdc_gui::model::AppModel::new(s, Some(p.clone()), support::session());
    drop(lock);
    m.settings.live_encode = true;
    m.save_settings();
    let back = GuiSettings::load(&p).0;
    assert!(back.advanced_open, "the file's value is kept");
    assert!(back.live_encode, "the app's change is written");
}

/// Review M3 (#45, Note BR): when the broken gui.toml cannot be moved aside (held open by another program), the
/// earlier `gui.toml.bad` is not deleted; the save fails and both files stay.
#[cfg(windows)]
#[test]
fn a_locked_broken_gui_toml_keeps_the_earlier_bad_copy() {
    use std::os::windows::fs::OpenOptionsExt;
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("gui.toml");
    let bad = p.with_extension("toml.bad");
    std::fs::write(&p, "live_encode = [").unwrap();
    std::fs::write(&bad, "earlier hand edit").unwrap();
    // FILE_SHARE_READ only: readable, but it cannot be renamed while this handle is open.
    let hold = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&p)
        .unwrap();
    assert!(GuiSettings::default().save(&p).is_err());
    drop(hold);
    assert_eq!(std::fs::read_to_string(&bad).unwrap(), "earlier hand edit");
    assert_eq!(std::fs::read_to_string(&p).unwrap(), "live_encode = [");
    // Unlocked, the next save replaces the earlier copy.
    assert_eq!(GuiSettings::default().save(&p).unwrap(), Some(bad.clone()));
    assert_eq!(std::fs::read_to_string(&bad).unwrap(), "live_encode = [");
}

/// Trace #31: a folder name that is not valid Unicode (a lone surrogate) cannot be written to TOML. It is left out of
/// the file; the rest of the settings still save.
#[cfg(windows)]
#[test]
fn a_non_unicode_path_does_not_stop_the_settings_saving() {
    use std::os::windows::ffi::OsStringExt;
    let bad = std::path::PathBuf::from(std::ffi::OsString::from_wide(&[0x43, 0x3a, 0x5c, 0xD800]));
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("sdc/gui.toml");
    let mut s = GuiSettings {
        live_encode: true,
        ..Default::default()
    };
    s.meshes.folder = Some(bad.clone());
    s.push_recent("C:/a".into());
    s.push_recent(bad);
    s.save(&p).unwrap();
    let (back, err) = GuiSettings::load(&p);
    assert_eq!(err, None);
    assert!(back.live_encode);
    assert_eq!(back.recent, [std::path::PathBuf::from("C:/a")]);
    assert!(back.meshes.folder.is_none());
}

/// Note GT4: a gui.toml written before the preview used the bundled shaders still has `[render3d] shaders`; it loads
/// without an error, the other settings survive, and the next save drops the key.
#[test]
fn an_old_shaders_key_is_ignored() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("sdc/gui.toml");
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(
        &p,
        "live_encode = true\n[render3d.shaders]\nroot = 'C:\\CS\\Shaders'\nowner_mod = 'CS'\nhash = 'ab12'\n",
    )
    .unwrap();
    let (s, err) = GuiSettings::load(&p);
    assert_eq!(err, None);
    assert!(s.live_encode);
    s.save(&p).unwrap();
    let text = std::fs::read_to_string(&p).unwrap();
    assert!(
        !text.contains("render3d") && !text.contains("shaders"),
        "{text}"
    );
}

/// Note GT4: the app's own save (`save_merged`) with the file changed since `base` runs the merge branch; the old
/// key is gone from the text and the hand-edited field survives.
#[test]
fn an_old_shaders_key_is_dropped_by_the_merging_save() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("sdc/gui.toml");
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(
        &p,
        "live_encode = true\n[render3d.shaders]\nroot = 'C:\\CS'\n",
    )
    .unwrap();
    let (mut s, err) = GuiSettings::load(&p);
    assert_eq!(err, None);
    let mut base = Some((s.clone(), engine::policy::SourceStamp::of(&p)));
    // A hand edit while the app runs: still carries the old key.
    std::fs::write(
        &p,
        "live_encode = true\nadvanced_open = true\n[render3d.shaders]\nroot = 'C:\\CS\\Shaders'\nhash = 'ab12'\n",
    )
    .unwrap();
    s.live_encode = false;
    s.save_merged(&p, &mut base).unwrap();
    let text = std::fs::read_to_string(&p).unwrap();
    assert!(
        !text.contains("render3d") && !text.contains("shaders"),
        "{text}"
    );
    let back = GuiSettings::load(&p).0;
    assert!(back.advanced_open, "the hand-edited field survives: {text}");
    assert!(!back.live_encode, "the app's change is written: {text}");
}
