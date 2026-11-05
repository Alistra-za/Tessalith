//! Community Shaders Advanced Skin material files (design 2026-11-05): one `<diffuse>.csskin.json` beside the diffuse
//! of each cs Advanced skin set, written after a clean export with the other JSON kinds' guard. No egui types.
use super::history::{orphans, record_key, recorded, records_at, remove_own_case_twin, sha256_of};
use super::sets::{engine_inputs, game_path_in, views, SlotFill, SlotKind};
use super::skin::{skin_field, Kind};
use super::xpmf_json::data_rel;
use super::AppModel;
use engine::config::{Subtype, Target};
use serde_json::{json, Map, Value as Json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
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

fn build_plan(m: &AppModel) -> Plan {
    let mut p = Plan::default();
    if m.target != Target::Cs {
        return p;
    }
    let vs = views(m);
    let mut paths = None;
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
        let paths = paths.get_or_insert_with(|| engine_inputs(m).1);
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
            let Some(j) = job(item) else {
                // R4: a host that plans no output for the slot. An own member without a job (a grey `_wet`) has its
                // own row from the engine.
                if let Some(SlotFill::Borrowed { from, .. }) = fill {
                    skip(SkinSkip::HostMissing {
                        slot,
                        host: from.clone(),
                    });
                }
                continue;
            };
            // The PGPatcher JSON's rule for slot paths (P1, P6, P11), folded as game paths are (Note BF).
            match game_path_in(m, &vs, paths, item) {
                Ok(g) => {
                    *row.material.texture_mut(slot) =
                        Some(g.replace('\\', "/").to_ascii_lowercase());
                    row.outputs.push((slot, j.output.clone()));
                }
                Err(why) => row.notes.push(format!(
                    "{}: {slot} left out of the CS skin JSON: {why}",
                    v.id
                )),
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
/// `None` while the diffuse cannot be read.
fn text_now(f: &CsSkinFile) -> Option<(String, Vec<&'static str>)> {
    let bytes = std::fs::read(&f.diffuse).ok()?;
    let mut mat = f.material.clone();
    let mut dropped = vec![];
    for (slot, out) in &f.outputs {
        if !out.is_file() {
            *mat.texture_mut(slot) = None;
            dropped.push(*slot);
        }
    }
    mat.authored_for = Some((bytes.len() as u64, fnv1a64(&bytes)));
    Some((material_text(&mat), dropped))
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
        let Some((text, dropped)) = text_now(f) else {
            notes.push(format!(
                "{}: {} was not exported; CS skin JSON not written",
                path.display(),
                f.diffuse.file_name().unwrap_or_default().to_string_lossy()
            ));
            continue;
        };
        for slot in dropped {
            notes.push(format!(
                "{}: the {slot} texture is not in the export folder; left out",
                path.display()
            ));
        }
        let key = record_key(&path);
        let last = out.get(&key).cloned();
        let last = last.as_deref();
        remove_own_case_twin(&path, last);
        if let Err(k) = engine::dds::may_replace(&path, text.as_bytes(), recorded(last)) {
            notes.push(k.note(&path));
            continue;
        }
        match engine::dds::write_atomic(&path, text.as_bytes()) {
            Ok(()) => {
                written += 1;
                if let Some(s) = sha256_of(&path) {
                    out.insert(key, s);
                }
            }
            Err(e) => notes.push(format!("{} not written: {e}", path.display())),
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
            .filter_map(|f| Some((f.rel.clone(), text_now(f)?.0)))
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
