//! Community Shaders Advanced Skin material files (design 2026-11-05): one `<diffuse>.csskin.json` beside the diffuse
//! of each cs Advanced skin set, written after a clean export with the other JSON kinds' guard. No egui types.
use super::history::{orphans, records_at};
use super::sets::{views, Missing, SlotFill, SlotKind};
use super::skin::{skin_field, Kind};
use super::xpmf_json::data_rel;
use super::AppModel;
use engine::config::{Subtype, Target};
use engine::policy::ResolvedJob;
use serde_json::{json, Map, Value as Json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// CS `Editor/Files.cpp` `Fingerprint`: FNV-1a 64 over the file's bytes.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// One material file's content. Texture values are game paths (`textures/...dds`, lower case, `/`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SkinMaterial {
    pub parameters: Map<String, Json>,
    pub controls: Option<String>,
    pub wet: Option<String>,
    pub detail: Option<String>,
    /// (size, FNV-1a 64) of the diffuse DDS.
    pub authored_for: Option<(u64, u64)>,
}

impl SkinMaterial {
    fn texture_mut(&mut self, slot: &str) -> &mut Option<String> {
        match slot {
            "controls" => &mut self.controls,
            "wet" => &mut self.wet,
            _ => &mut self.detail,
        }
    }
}

/// The JSON members in the order the file lists them (serde_json's `Map` sorts its keys).
#[derive(serde::Serialize)]
struct Doc {
    format: &'static str,
    version: u32,
    #[serde(skip_serializing_if = "Map::is_empty")]
    parameters: Map<String, Json>,
    #[serde(skip_serializing_if = "Map::is_empty")]
    textures: Map<String, Json>,
    #[serde(rename = "authoredFor", skip_serializing_if = "Option::is_none")]
    authored_for: Option<Authored>,
}

#[derive(serde::Serialize)]
struct Authored {
    size: u64,
    hash: String,
}

/// The file's text: 2-space pretty JSON and a final newline; empty `parameters`/`textures` left out, numbers as floats.
pub fn material_text(m: &SkinMaterial) -> String {
    let textures = [
        ("controls", &m.controls),
        ("wet", &m.wet),
        ("detail", &m.detail),
    ]
    .into_iter()
    .filter_map(|(k, v)| Some((k.to_string(), json!(v.as_ref()?))))
    .collect();
    let doc = Doc {
        format: "CommunityShaders.SkinMaterial",
        version: 1,
        parameters: m
            .parameters
            .iter()
            .map(|(k, v)| (k.clone(), super::cs_pbr::floats(v)))
            .collect(),
        textures,
        authored_for: m.authored_for.map(|(size, h)| Authored {
            size,
            hash: format!("{h:016x}"),
        }),
    };
    let mut s = serde_json::to_string_pretty(&doc).expect("JSON values serialise");
    s.push('\n');
    s
}

/// Why an Advanced skin set writes no `.csskin.json`, or (`HostMissing`) leaves a texture out of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkinSkip {
    NoDiffuse,
    /// Decided 8: the host set's material file is the one CS reads for that diffuse.
    BorrowedDiffuse {
        host: String,
    },
    /// The diffuse is borrowed from a set that is not an Advanced skin set: no file for that diffuse.
    HostNotSkin {
        host: String,
    },
    /// A borrowed `controls`/`wet`/`detail` whose host plans no output for it: left out of `textures`.
    HostMissing {
        slot: &'static str,
        host: String,
    },
}

/// The JSON's texture slots and the set slots that fill them.
const SLOTS: [(&str, SlotKind); 3] = [
    ("controls", SlotKind::Rmaos),
    ("wet", SlotKind::SkinWet),
    ("detail", SlotKind::SkinDetail),
];

/// One set's file as planned.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PlanRow {
    id: String,
    key: String,
    json: PathBuf,
    diffuse: PathBuf,
    material: SkinMaterial,
    /// (slot, output path) of each texture named in `material`, for the on-disk check at finish.
    outputs: Vec<(&'static str, PathBuf)>,
    /// Textures left out for want of a game path.
    notes: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Plan {
    rows: Vec<PlanRow>,
    /// (set id, set key, why).
    skips: Vec<(String, String, SkinSkip)>,
}

/// A member of only an Advanced skin set (the diffuse decides `SetView::skin`, so a set without one needs this).
fn skin_only(st: Subtype) -> bool {
    matches!(
        st,
        Subtype::SkinRfaos | Subtype::SkinWet | Subtype::SkinDetail
    )
}

/// The game path the game loads a job's output from: its path under the export root `out_root`, re-based on its
/// source's game path (an output mirrors its source's place under the opened folder, which can sit above or inside
/// `textures`, so a renamed output or a project opened at `textures` still gets `textures/...`). `Err` unless it is
/// `textures/...dds` in lower-case ASCII with `/`: CS ignores a whole file for one bad path.
fn output_game_path(m: &AppModel, out_root: &Path, job: &ResolvedJob) -> Result<String, String> {
    let comps = |p: &Path| -> Vec<String> {
        p.components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect()
    };
    let out = job
        .output
        .strip_prefix(out_root)
        .map(comps)
        .map_err(|_| format!("{} is outside the export folder", job.output.display()))?;
    let src = m
        .root
        .as_deref()
        .and_then(|r| job.input.strip_prefix(r).ok())
        .map_or(out.len(), |r| r.components().count());
    let game: Vec<&str> = job.game_path.split('/').collect();
    let parts: Vec<String> = match game.len().checked_sub(src) {
        Some(n) => game[..n].iter().map(|s| s.to_string()).chain(out).collect(),
        None => out.into_iter().skip(src - game.len()).collect(),
    };
    let g = parts.join("/").to_ascii_lowercase();
    let shape = g.starts_with("textures/")
        && g.ends_with(".dds")
        && g.is_ascii()
        && g.split('/').all(|c| !c.is_empty() && c != "." && c != "..");
    if shape {
        Ok(g)
    } else {
        Err(format!("{g} is not a textures/...dds game path"))
    }
}

fn build_plan(m: &AppModel) -> Plan {
    let mut p = Plan::default();
    if m.target != Target::Cs {
        return p;
    }
    let vs = views(m);
    let out_root = m.output_root();
    for v in vs.iter() {
        let job = |k: &str| m.item(k).and_then(|i| i.job.as_ref());
        let has_skin_member = v
            .members
            .iter()
            .any(|k| job(k).is_some_and(|j| skin_only(j.profile.subtype)));
        if !v.skin && !has_skin_member {
            continue;
        }
        let mut skip = |s| p.skips.push((v.id.clone(), v.key.clone(), s));
        let diffuse = match v.slots.get(&SlotKind::Diffuse) {
            Some(SlotFill::Own { item }) if v.skin => item,
            // An own diffuse that is not `skin_albedo`: not an Advanced skin set (P4).
            Some(SlotFill::Own { .. }) => continue,
            Some(SlotFill::Borrowed { from, .. }) if v.skin => {
                skip(SkinSkip::BorrowedDiffuse { host: from.clone() });
                continue;
            }
            Some(SlotFill::Borrowed { from, .. }) => {
                skip(SkinSkip::HostNotSkin { host: from.clone() });
                continue;
            }
            Some(SlotFill::Empty) | None => {
                skip(SkinSkip::NoDiffuse);
                continue;
            }
        };
        // An undecided diffuse plans nothing yet.
        let Some(dj) = job(diffuse) else {
            continue;
        };
        let mut row = PlanRow {
            id: v.id.clone(),
            key: v.key.clone(),
            json: dj.output.with_extension("csskin.json"),
            diffuse: dj.output.clone(),
            material: SkinMaterial {
                parameters: m.skin_values(&v.id).into_iter().collect(),
                ..Default::default()
            },
            outputs: vec![],
            notes: vec![],
        };
        for (slot, kind) in SLOTS {
            let fill = v.slots.get(&kind);
            let Some(item) = fill.and_then(SlotFill::item) else {
                continue;
            };
            // A slot's item always has a job (`slot_of`); a grey `_wet` has none, so it fills no slot.
            let Some(j) = job(item) else {
                continue;
            };
            match output_game_path(m, &out_root, j) {
                Ok(g) => {
                    *row.material.texture_mut(slot) = Some(g);
                    row.outputs.push((slot, j.output.clone()));
                }
                Err(why) => row.notes.push(format!(
                    "{}: {slot} left out of the CS skin JSON: {why}",
                    v.id
                )),
            }
        }
        // R4: a borrow whose host has nothing in that slot (its grey `_wet` is not exported) or is gone.
        for miss in &v.missing {
            if let Missing::Borrow { slot: n, from, .. } = miss {
                if let Some(&(slot, _)) = SLOTS.iter().find(|(_, k)| k.slot_number() == Some(*n)) {
                    skip(SkinSkip::HostMissing {
                        slot,
                        host: from.clone(),
                    });
                }
            }
        }
        p.rows.push(row);
    }
    p
}

fn plan(m: &AppModel) -> Arc<Plan> {
    super::cached(
        m,
        |c| c.cs_skin.clone(),
        |c, v| c.cs_skin = Some(v),
        || Arc::new(build_plan(m)),
    )
}

/// (set id, set key, the JSON's path, its material without `authored_for`).
pub type PlannedFile = (String, String, PathBuf, SkinMaterial);
/// (set id, set key, why).
pub type SkippedSet = (String, String, SkinSkip);

/// The `.csskin.json` files the next export writes, one per cs Advanced skin set with its own diffuse: (set id, set
/// key, the JSON's path, its material without `authored_for`), and the sets that write none or leave a texture out:
/// (set id, set key, why). Cached with the set views.
pub fn skin_json_plan(m: &AppModel) -> (Vec<PlannedFile>, Vec<SkippedSet>) {
    let p = plan(m);
    let rows = p
        .rows
        .iter()
        .map(|r| {
            (
                r.id.clone(),
                r.key.clone(),
                r.json.clone(),
                r.material.clone(),
            )
        })
        .collect();
    (rows, p.skips.clone())
}

/// A value the card's range or type refuses (a hand-edited `sdc-project.toml`): CS would ignore the whole file.
fn refused_value(k: &str, v: &Json) -> Option<String> {
    let ok = match skin_field(k)?.kind {
        Kind::Num { lo, hi, .. } => v.as_f64().is_some_and(|x| (lo..=hi).contains(&x)),
        Kind::Flag { .. } => v.is_boolean(),
    };
    (!ok).then(|| format!("{k} = {v} is outside what CS accepts"))
}

/// One file an export writes once it has ended.
#[derive(Debug, Clone, PartialEq)]
pub struct CsSkinFile {
    /// The JSON's path under the root, `/`-separated (the hash record's key).
    pub rel: String,
    /// The diffuse DDS `authoredFor` hashes.
    pub diffuse: PathBuf,
    pub material: SkinMaterial,
    /// (slot, output path) of each texture in `material`: one not on disk at the end is left out.
    pub outputs: Vec<(&'static str, PathBuf)>,
}

/// What an export about to start writes once it has ended (computed when it starts, like the CS PBR files).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PendingCsSkin {
    pub root: PathBuf,
    pub files: Vec<CsSkinFile>,
    pub notes: Vec<String>,
    /// Paths (as in `files`) refused this run: not orphans.
    pub refused: Vec<String>,
}

/// One file per planned set (P4). `None` when no set plans one and nothing is recorded; with a record it is `Some`, so
/// the stale files are reported (P3).
pub(super) fn prepare(m: &AppModel) -> Option<PendingCsSkin> {
    let doc = m.sets_doc.as_ref()?;
    let plan = plan(m);
    let mut p = PendingCsSkin {
        root: m.output_root(),
        ..Default::default()
    };
    if plan.rows.is_empty() {
        return (!doc.cs_skin_shas().is_empty()).then_some(p);
    }
    let mut seen = BTreeSet::new();
    for r in &plan.rows {
        p.notes.extend(r.notes.iter().cloned());
        let Some(rel) = data_rel(&p.root, &r.json) else {
            p.notes.push(format!(
                "{}: CS skin JSON not written: {} is outside the export folder {}",
                r.id,
                r.json.display(),
                p.root.display()
            ));
            continue;
        };
        if let Some(why) = r
            .material
            .parameters
            .iter()
            .find_map(|(k, v)| refused_value(k, v))
        {
            p.notes
                .push(format!("{}: CS skin JSON not written: {why}", r.id));
            p.refused.push(rel);
            continue;
        }
        // Windows names ignore case; the borrowed-diffuse rule should make this impossible.
        if !seen.insert(rel.to_lowercase()) {
            p.notes.push(format!(
                "{}: {rel} is also another set's CS skin JSON; written once",
                r.id
            ));
            continue;
        }
        p.files.push(CsSkinFile {
            rel,
            diffuse: r.diffuse.clone(),
            material: r.material.clone(),
            outputs: r.outputs.clone(),
        });
    }
    Some(p)
}

/// The file's text as it is now: `authoredFor` from the diffuse on disk, textures not on disk left out (returned).
/// `Err` while the diffuse cannot be read.
fn text_now(f: &CsSkinFile) -> std::io::Result<(String, Vec<&'static str>)> {
    let bytes = std::fs::read(&f.diffuse)?;
    let mut mat = f.material.clone();
    let mut dropped = vec![];
    for (slot, out) in &f.outputs {
        if !out.is_file() {
            *mat.texture_mut(slot) = None;
            dropped.push(*slot);
        }
    }
    mat.authored_for = Some((bytes.len() as u64, fnv1a64(&bytes)));
    Ok((material_text(&mat), dropped))
}

/// After a clean export: write each file whose diffuse is on disk, unless it was edited outside Tessalith, then report
/// the recorded files this run did not write (never deleted). Returns the new record.
pub(super) fn finish(
    p: &PendingCsSkin,
    shas: &BTreeMap<String, String>,
    notes: &mut Vec<String>,
) -> BTreeMap<String, String> {
    notes.extend(p.notes.iter().cloned());
    let mut out = records_at(shas, &p.root);
    let mut written = 0;
    for f in &p.files {
        let path = p.root.join(&f.rel);
        let (text, dropped) = match text_now(f) {
            Ok(t) => t,
            Err(e) => {
                let name = f.diffuse.file_name().unwrap_or_default().to_string_lossy();
                notes.push(match e.kind() {
                    std::io::ErrorKind::NotFound => format!(
                        "{}: {name} was not exported; CS skin JSON not written",
                        path.display()
                    ),
                    _ => format!(
                        "{}: CS skin JSON not written: {name} could not be read ({e})",
                        path.display()
                    ),
                });
                continue;
            }
        };
        for slot in dropped {
            notes.push(format!(
                "{}: the {slot} texture is not in the export folder; left out",
                path.display()
            ));
        }
        if super::json_finish::guarded_write(&path, &text, &mut out, notes) {
            written += 1;
        }
    }
    if written > 0 {
        notes.push(format!(
            "CS skin JSON written: {written} file{} under {}",
            if written == 1 { "" } else { "s" },
            p.root.display()
        ));
    }
    let this_run = p.files.iter().map(|f| &f.rel).chain(&p.refused);
    for path in orphans(&out, &p.root, this_run) {
        notes.push(format!(
            "{}: no Advanced skin set writes this CS skin JSON any more; the file was left in place",
            path.display()
        ));
    }
    out
}

impl super::json_finish::Guarded for PendingCsSkin {
    const NAME: &'static str = "CS skin JSON";
    const GATE: Option<engine::also::Phase> = None;
    fn has_files(&self) -> bool {
        !self.files.is_empty()
    }
    fn recorded(d: &crate::sets_doc::SetsDoc) -> BTreeMap<String, String> {
        d.cs_skin_shas()
    }
    fn write(
        &self,
        recorded: &BTreeMap<String, String>,
        notes: &mut Vec<String>,
    ) -> BTreeMap<String, String> {
        finish(self, recorded, notes)
    }
    fn current(&self) -> Vec<PathBuf> {
        let files: Vec<(String, String)> = self
            .files
            .iter()
            .filter_map(|f| Some((f.rel.clone(), text_now(f).ok()?.0)))
            .collect();
        super::also::current_files(&self.root, &files)
    }
    fn record(
        d: &mut crate::sets_doc::SetsDoc,
        shas: &BTreeMap<String, String>,
    ) -> std::io::Result<()> {
        d.set_cs_skin_shas(shas)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn pending(
        root: &Path,
        diffuse: PathBuf,
        outputs: Vec<(&'static str, PathBuf)>,
    ) -> PendingCsSkin {
        PendingCsSkin {
            root: root.into(),
            files: vec![CsSkinFile {
                rel: "textures/h.csskin.json".into(),
                diffuse,
                material: SkinMaterial {
                    controls: Some("textures/h_rfaos.dds".into()),
                    ..Default::default()
                },
                outputs,
            }],
            ..Default::default()
        }
    }

    /// The finish-time drop: a texture the export did not leave on disk is left out, with a note; the file is written.
    #[test]
    fn a_texture_missing_at_the_end_is_left_out_with_a_note() {
        let d = tempfile::tempdir().unwrap();
        let t = d.path().join("textures");
        std::fs::create_dir_all(&t).unwrap();
        std::fs::write(t.join("h.dds"), b"DDS x").unwrap();
        let p = pending(
            d.path(),
            t.join("h.dds"),
            vec![("controls", t.join("h_rfaos.dds"))],
        );
        let mut notes = vec![];
        let shas = finish(&p, &BTreeMap::new(), &mut notes);
        let text = std::fs::read_to_string(t.join("h.csskin.json")).unwrap();
        assert!(!text.contains("textures/h_rfaos.dds"), "{text}");
        assert!(text.contains("authoredFor"), "{text}");
        assert!(
            notes
                .iter()
                .any(|n| n.contains("the controls texture is not in the export folder")),
            "{notes:?}"
        );
        assert_eq!(shas.len(), 1);
    }

    /// Fix round 1 #2: a missing diffuse and an unreadable one (here a folder) say which, with the error.
    #[test]
    fn a_missing_diffuse_and_an_unreadable_one_get_their_own_notes() {
        let d = tempfile::tempdir().unwrap();
        let t = d.path().join("textures");
        std::fs::create_dir_all(t.join("h.dds")).unwrap();
        for (diffuse, want, not) in [
            (t.join("gone.dds"), "was not exported", "could not be read"),
            (t.join("h.dds"), "could not be read", "was not exported"),
        ] {
            let mut notes = vec![];
            finish(
                &pending(d.path(), diffuse, vec![]),
                &BTreeMap::new(),
                &mut notes,
            );
            let all = notes.join("\n");
            assert!(all.contains(want) && !all.contains(not), "{all}");
            assert!(!t.join("h.csskin.json").exists());
        }
    }
}
