//! `--set` overrides (spec §8): `field=value`, `subtype:<id>.field=value`, `file:<glob>.field=value`, and
//! `set:<id>.shader=<shader>` (M6, DECISIONS Note S-shader-1: a set's shader, the only field a set scope takes).
//! `Scope::Path` is never parsed from `--set`; it comes only from `sdc.toml`'s `[files]` table.
use crate::codes::Code;
use crate::config::Subtype;
use crate::discover::InputFile;
use crate::shader::Shader;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    All,
    Subtype(Subtype),
    File(String),
    /// A `[files]` key (DECISIONS N2): '/'-separated, relative to the project root, matched by `path_key_matches`.
    Path(String),
    /// `set:<id>`: a set id as `sets::set_id` prints it, or a bare base (M6, DECISIONS Note S-shader-1). Sets only
    /// `shader`; it never reaches a file's profile.
    Set(String),
}

/// Whether a `[files]` key names the file at project-relative path `rel` (DECISIONS N2): case-insensitively,
/// `rel == key`, or `rel` without the last extension of its file name `== key` (a packed output's key).
pub fn path_key_matches(key: &str, rel: &str) -> bool {
    KeyForms::of(rel).matches(key)
}

/// What a `[files]` key is compared with for one project-relative path (`path_key_matches`): the path lower-cased,
/// and lower-cased without the last extension of its file name when that leaves a non-empty stem. Worked out once
/// per file, so matching many keys costs no path work (DECISIONS AV3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyForms {
    pub(crate) full: String,
    pub(crate) stem: Option<String>,
}

impl KeyForms {
    pub fn of(rel: &str) -> KeyForms {
        let full = rel.to_lowercase();
        let (dir, name) = full.rsplit_once('/').unwrap_or(("", full.as_str()));
        let stem = match name.rsplit_once('.') {
            Some((stem, _)) if !stem.is_empty() => Some(if dir.is_empty() {
                stem.to_string()
            } else {
                format!("{dir}/{stem}")
            }),
            _ => None,
        };
        KeyForms { full, stem }
    }
    /// Whether `key` names this path, case-insensitively.
    pub fn matches(&self, key: &str) -> bool {
        let k = key.to_lowercase();
        self.full == k || self.stem.as_deref() == Some(k.as_str())
    }
}

/// The files `[files]`-style keys name, by match form: for many keys over many files (DECISIONS AV3).
pub(crate) struct KeyIndex(std::collections::HashMap<String, Vec<usize>>);

impl KeyIndex {
    pub(crate) fn new(root: &Path, files: &[InputFile]) -> KeyIndex {
        let mut m: std::collections::HashMap<String, Vec<usize>> = Default::default();
        for (i, f) in files.iter().enumerate() {
            if let Some(r) = project_rel(root, f) {
                let KeyForms { full, stem } = KeyForms::of(&r);
                for form in std::iter::once(full).chain(stem) {
                    m.entry(form).or_default().push(i);
                }
            }
        }
        KeyIndex(m)
    }
    /// The indices of the files `key` names, in file order (a file matches a key by one form only).
    pub(crate) fn hits(&self, key: &str) -> &[usize] {
        self.0.get(&key.to_lowercase()).map_or(&[], Vec::as_slice)
    }
}

/// `p` as an absolute path (the current directory for an empty or relative one); `p` itself when that fails.
pub(crate) fn absolute_or_same(p: &Path) -> PathBuf {
    let p = if p.as_os_str().is_empty() {
        Path::new(".")
    } else {
        p
    };
    std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf())
}

/// `input`'s path relative to `root`, '/'-separated: the folder of `input.path` relative to `root`, then the file
/// name of `input.rel` (for a packed output, its virtual name; for an `[inputs]` alias, the real name). `None` when `input` is not under `root`.
pub fn project_rel(root: &Path, input: &InputFile) -> Option<String> {
    let lower = |p: &Path| p.to_string_lossy().replace('\\', "/").to_lowercase();
    // One path form: a relative root or input (e.g. `--config sdc.toml`) is resolved against the current directory.
    let (root, dir) = (
        absolute_or_same(root),
        absolute_or_same(input.path.parent()?),
    );
    let (root, dir) = (root.as_path(), dir.as_path());
    let (root_s, dir_s) = (lower(root), lower(dir));
    let root_s = root_s.trim_end_matches('/');
    let rest = if dir_s == root_s {
        ""
    } else {
        dir_s.strip_prefix(&format!("{root_s}/"))?
    };
    let dir_orig = dir.to_string_lossy().replace('\\', "/");
    // Keep the original case of the folder part, by taking the same number of bytes from the original string.
    // Lower-casing can change the byte length (non-ASCII): then fall back to the lower-cased part.
    let rest_orig = if dir_orig.len() == dir_s.len() {
        dir_orig.get(dir_orig.len() - rest.len()..).unwrap_or(rest)
    } else {
        rest
    };
    // F3b: an aliased file keeps its real name here, so its keys never move with the alias.
    let name = input
        .real_rel
        .as_deref()
        .unwrap_or(&input.rel)
        .file_name()?
        .to_string_lossy();
    Some(if rest_orig.is_empty() {
        name.into_owned()
    } else {
        format!("{rest_orig}/{name}")
    })
}

/// The `[files]` layer (DECISIONS N2) and the per-file acknowledgements, resolved against one project root.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FileScopes {
    /// The folder `[files]` keys are relative to; `None` disables the layer.
    pub root: Option<PathBuf>,
    /// `[files]` field values, every one `Scope::Path`, in key order then field order.
    pub overrides: Vec<SetOverride>,
    /// Key → codes acknowledged for that file only.
    pub allow: BTreeMap<String, BTreeSet<Code>>,
    /// The planned files' match forms by path (`index`), with the file name they were worked out for.
    pub(crate) forms: BTreeMap<PathBuf, (Option<std::ffi::OsString>, Option<KeyForms>)>,
    /// Note DW: indices (into the planned files, as `TextureSet::members`) of the GUI's pinned files named as a
    /// set's diffuse member (no suffix, or `_d`): their set gets no Advanced Skin reading from its chosen shader.
    pub(crate) skin_pinned: BTreeSet<usize>,
    /// `overrides` and `allow` by lower-cased key (`index`); `None` until built. Valid only while `overrides` and
    /// `allow` change through `index` or `set_entry`: staleness is detected by their lengths only, so any other
    /// in-place key change needs `index` again.
    pub(crate) keys: Option<ScopeIndex>,
}

/// The `[files]` layer by lower-cased key, so a file's lookup costs two map hits, not a scan of every entry.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ScopeIndex {
    /// Lower-cased key → indices into `FileScopes::overrides`, ascending.
    overrides: BTreeMap<String, Vec<usize>>,
    /// Lower-cased key → the `FileScopes::allow` keys that fold to it.
    allow: BTreeMap<String, Vec<String>>,
    /// `overrides.len()` and `allow.len()` when built: a different length means the index is stale.
    lens: (usize, usize),
}

impl FileScopes {
    pub const NONE: FileScopes = FileScopes {
        root: None,
        overrides: Vec::new(),
        allow: BTreeMap::new(),
        forms: BTreeMap::new(),
        skin_pinned: BTreeSet::new(),
        keys: None,
    };

    /// The key index, when built and not stale.
    fn fresh_keys(&self) -> Option<&ScopeIndex> {
        self.keys
            .as_ref()
            .filter(|ix| ix.lens == (self.overrides.len(), self.allow.len()))
    }

    /// The `[files]` overrides whose key names the file with match forms `forms`, in `overrides` order.
    pub(crate) fn path_overrides<'s>(&'s self, forms: &KeyForms) -> Vec<&'s SetOverride> {
        let Some(ix) = self.fresh_keys() else {
            return self
                .overrides
                .iter()
                .filter(|o| matches!(&o.scope, Scope::Path(k) if forms.matches(k)))
                .collect();
        };
        let mut hits: Vec<usize> = forms_iter(forms)
            .filter_map(|f| ix.overrides.get(f))
            .flatten()
            .copied()
            .collect();
        hits.sort_unstable();
        hits.dedup();
        hits.into_iter().map(|i| &self.overrides[i]).collect()
    }

    /// Whether the file with match forms `forms` has `code` acknowledged by its own `allow`.
    pub(crate) fn allows(&self, forms: &KeyForms, code: Code) -> bool {
        let Some(ix) = self.fresh_keys() else {
            return self
                .allow
                .iter()
                .any(|(k, codes)| codes.contains(&code) && forms.matches(k));
        };
        forms_iter(forms)
            .filter_map(|f| ix.allow.get(f))
            .flatten()
            .any(|k| self.allow.get(k).is_some_and(|c| c.contains(&code)))
    }

    /// Work out each of `files`' match forms once (audit 2026-10-15 item 5), so `forms_of` costs a lookup, not two
    /// absolute-path resolutions per key. Nothing to do when the layer is off or empty.
    pub fn index(&mut self, files: &[InputFile]) {
        let Some(root) = self.root.clone() else {
            return;
        };
        if self.overrides.is_empty() && self.allow.is_empty() {
            return;
        }
        self.rebuild_keys();
        self.forms = files
            .iter()
            .map(|f| {
                let forms = project_rel(&root, f).map(|r| KeyForms::of(&r));
                (f.path.clone(), (key_name(f).map(Into::into), forms))
            })
            .collect();
    }

    /// Replace every `[files]` override and the `allow` entry of `key` (this exact spelling, as the document holds
    /// it) with `entry` parsed as the config loader does; `None` removes them. Overrides stay in key order, and the
    /// key index is rebuilt. A bad entry is an error and changes nothing.
    pub(crate) fn set_entry(
        &mut self,
        key: &str,
        entry: Option<&toml::Table>,
    ) -> Result<(), crate::presets::PresetError> {
        let parsed = match entry {
            Some(t) => {
                let v = serde_json::to_value(t)
                    .map_err(|e| crate::presets::PresetError::Parse(e.to_string()))?;
                Some(crate::presets::file_entry(key, &v)?)
            }
            None => None,
        };
        let is_key = |o: &SetOverride| matches!(&o.scope, Scope::Path(k) if k == key);
        self.overrides.retain(|o| !is_key(o));
        self.allow.remove(key);
        if let Some((ovs, codes)) = parsed {
            // `[files]` is a sorted map: this key's fields go before the first later key's.
            let at = self
                .overrides
                .iter()
                .position(|o| matches!(&o.scope, Scope::Path(k) if k.as_str() > key))
                .unwrap_or(self.overrides.len());
            self.overrides.splice(at..at, ovs);
            if let Some(codes) = codes {
                self.allow.insert(key.to_string(), codes);
            }
        }
        self.rebuild_keys();
        Ok(())
    }

    /// The key index of `overrides` and `allow` as they stand now.
    pub(crate) fn rebuild_keys(&mut self) {
        let mut ix = ScopeIndex {
            overrides: BTreeMap::new(),
            allow: BTreeMap::new(),
            lens: (self.overrides.len(), self.allow.len()),
        };
        for (i, o) in self.overrides.iter().enumerate() {
            if let Scope::Path(k) = &o.scope {
                ix.overrides.entry(k.to_lowercase()).or_default().push(i);
            }
        }
        for k in self.allow.keys() {
            ix.allow
                .entry(k.to_lowercase())
                .or_default()
                .push(k.clone());
        }
        self.keys = Some(ix);
    }

    /// The match forms of `input`'s project-relative path when the layer is active: from `index` when it saw this
    /// path under this name, else worked out now (a packed output's virtual name, a file planned later).
    pub fn forms_of(&self, input: &InputFile) -> Option<std::borrow::Cow<'_, KeyForms>> {
        let root = self.root.as_deref()?;
        match self.forms.get(&input.path) {
            Some((name, forms)) if name.as_deref() == key_name(input) => {
                forms.as_ref().map(std::borrow::Cow::Borrowed)
            }
            _ => project_rel(root, input).map(|r| std::borrow::Cow::Owned(KeyForms::of(&r))),
        }
    }
}

/// The lower-cased forms a key is compared with (`KeyForms::matches`).
fn forms_iter(forms: &KeyForms) -> impl Iterator<Item = &str> {
    std::iter::once(forms.full.as_str()).chain(forms.stem.as_deref())
}

/// The file name `project_rel` takes for `input`.
fn key_name(input: &InputFile) -> Option<&std::ffi::OsStr> {
    input.real_rel.as_deref().unwrap_or(&input.rel).file_name()
}

/// For `ResolveCtx` literals that have no `[files]` layer (tests, `inspect` on a single file).
pub static NO_FILE_SCOPES: FileScopes = FileScopes::NONE;

#[derive(Debug, Clone, PartialEq)]
pub struct SetOverride {
    pub scope: Scope,
    pub field: String,
    pub value: Value,
}

/// Every settable dotted path (Profile fields minus identity fields target/category), plus `reencode`, a
/// plan-time selector (DECISIONS S6).
pub const FIELD_PATHS: &[&str] = &[
    "subtype",
    "reencode",
    "color.content",
    "color.tag",
    "alpha.policy",
    "alpha.threshold",
    "alpha.compare",
    "alpha.exact_opaque",
    "alpha.parallax",
    "alpha.exr_straight",
    "channels.swizzle",
    "channels.flip_green",
    "channels.green_convention",
    "channels.reconstruct_z",
    "channels.grayscale_source",
    "mips.levels",
    "mips.filter",
    "mips.edge",
    "mips.normal",
    "mips.normal_base",
    "mips.bake_clamp",
    "mips.coverage",
    "format",
    "header",
    "resize",
    "drop_top_mip",
    "encoder.backend",
    "encoder.effort",
    "encoder.rgbcx_level",
    "encoder.perceptual",
    "encoder.bc7_weights",
    "encoder.bc1_transparent_black",
    "encoder.bc45_search_radius",
    "validation.min_mip_psnr_db",
    "validation.coverage_tolerance_pts",
    "validation.complex_m_warn_alpha",
    "validation.bc6h_min_peak_ratio",
    "validation.pgpatcher_non_white_share",
    "validation.mips",
    "packing.enabled",
    "packing.roughness_default",
    "packing.prefer",
    "packing.recipe",
    "packing.fill.roughness",
    "packing.fill.metallic",
    "packing.fill.ao",
    "packing.fill.specular",
    "packing.fill.glossiness",
    "packing.fill.height",
    "packing.fill.opacity",
    "packing.fill.fuzz",
    "rnm.enabled",
    "rnm.order",
    "rnm.bump.strength",
    "rnm.bump.flip_green",
    "rnm.bump.mask",
    "rnm.bump2.strength",
    "rnm.bump2.flip_green",
    "rnm.bump2.mask",
    "rnm.detail.strength",
    "rnm.detail.flip_green",
    "rnm.detail.mask",
    "rnm.detail2.strength",
    "rnm.detail2.flip_green",
    "rnm.detail2.mask",
    "rnm.bent.strength",
    "rnm.bent.flip_green",
    "rnm.bent.mask",
    "rnm.bent.mode",
    "rnm.bent.ao_power",
    "rnm.bent.ao",
    "rnm.target",
    "helper.role",
    "helper.fills",
    "helper.read",
    "helper.invert",
    "cubemap.style",
    "cubemap.layout",
    "cubemap.face_size",
    "cubemap.yaw",
    "cubemap.mirror",
    "cubemap.face_rotate",
    "cubemap.face_flip",
    "cubemap.vcross_negz_rotate",
    "cubemap.blur",
    "cubemap.exposure",
    "cubemap.brightness",
    "cubemap.contrast",
    "cubemap.midtones",
    "cubemap.saturation",
    "cubemap.tint",
    "cubemap.highlight_clamp",
    "cubemap.ground_fill",
    "cubemap.horizon",
];

pub const REENCODE_FIELD: &str = "reencode";

/// M6 Track F2 (DECISIONS Note W3): which base normal a layer file feeds; a per-file selector, not a profile field.
pub const RNM_TARGET_FIELD: &str = "rnm.target";

/// A plan-time per-file selector (`reencode`, `rnm.target`, `helper.*`): never merged into a profile.
pub fn is_selector(field: &str) -> bool {
    field == REENCODE_FIELD || field == RNM_TARGET_FIELD || crate::helper::FIELDS.contains(&field)
}

/// `rnm.target`'s value, when it is one of `n`, `cnr`, `both`.
pub fn rnm_target_value(v: &Value) -> Option<crate::config::RnmTarget> {
    serde_json::from_value(v.clone()).ok()
}

/// A TOML literal (`100`, `true`, `"x"`) keeps its type; anything else (`bc1`, `none`) is a string.
fn parse_value(raw: &str) -> Value {
    match toml::from_str::<toml::Table>(&format!("v = {raw}")) {
        Ok(t) => serde_json::to_value(&t["v"]).unwrap_or(Value::String(raw.to_string())),
        Err(_) => Value::String(raw.to_string()),
    }
}

/// Whether `v` holds a null: TOML's `inf`, `-inf` and `nan` convert to JSON null, which the merge ignores, so a
/// setting typed that way would silently do nothing (audit #8, Note BC).
pub(crate) fn non_finite(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Array(a) => a.iter().any(non_finite),
        Value::Object(o) => o.values().any(non_finite),
        _ => false,
    }
}

pub fn parse_set(s: &str) -> Result<SetOverride, String> {
    let (key, raw) = s
        .split_once('=')
        .ok_or_else(|| format!("--set '{s}': expected <field>=<value>"))?;
    let value = parse_value(raw.trim());
    if non_finite(&value) {
        return Err(format!("--set '{s}': not a finite number"));
    }
    let known = |f: &str| FIELD_PATHS.contains(&f);
    let (scope, field) = if let Some(rest) = key.strip_prefix("subtype:") {
        let (id, field) = rest
            .split_once('.')
            .ok_or_else(|| format!("--set '{s}': expected subtype:<id>.<field>"))?;
        let st: Subtype = serde_json::from_value(Value::String(id.to_string()))
            .map_err(|_| format!("--set '{s}': unknown sub-type '{id}'"))?;
        if field == "subtype" {
            return Err(format!(
                "--set '{s}': a sub-type scope cannot set the sub-type"
            ));
        }
        if field == REENCODE_FIELD {
            return Err(format!(
                "--set '{s}': reencode is decided per file, before the sub-type: use reencode=true or file:<glob>.reencode=true"
            ));
        }
        if field == RNM_TARGET_FIELD {
            return Err(format!(
                "--set '{s}': rnm.target is decided per layer file: use rnm.target=<n|cnr|both> or file:<glob>.rnm.target=<…>"
            ));
        }
        if crate::helper::FIELDS.contains(&field) {
            return Err(format!(
                "--set '{s}': {field} is decided per file, before the sub-type: use file:<glob>.{field}=<…>"
            ));
        }
        if !known(field) {
            return Err(format!("--set '{s}': unknown field '{field}'"));
        }
        (Scope::Subtype(st), field)
    } else if let Some(rest) = key.strip_prefix("set:") {
        // Set ids may contain dots, so only the `.shader` suffix splits the key (DECISIONS Note S-shader-1).
        let id = match rest.strip_suffix(".shader") {
            Some(id) if !id.is_empty() => id,
            _ if rest.contains('.') && !rest.starts_with('.') => {
                return Err(format!("--set '{s}': a set scope sets only `shader`"))
            }
            _ => return Err(format!("--set '{s}': expected set:<id>.shader=<shader>")),
        };
        let raw = raw.trim();
        if serde_json::from_value::<Shader>(Value::String(raw.to_string())).is_err() {
            let ids: Vec<&str> = Shader::ALL.iter().map(|s| s.id()).collect();
            return Err(format!(
                "--set '{s}': unknown shader '{raw}' (one of: {})",
                ids.join(", ")
            ));
        }
        return Ok(SetOverride {
            scope: Scope::Set(id.to_string()),
            field: "shader".to_string(),
            value: Value::String(raw.to_string()),
        });
    } else if let Some(rest) = key.strip_prefix("file:") {
        // The glob may itself contain dots, so take the longest known field the key ends with.
        let field = FIELD_PATHS
            .iter()
            .filter(|f| rest.len() > f.len() + 1 && rest.ends_with(&format!(".{f}")))
            .max_by_key(|f| f.len())
            .ok_or_else(|| format!("--set '{s}': expected file:<glob>.<field>"))?;
        let glob = &rest[..rest.len() - field.len() - 1];
        crate::policy::compile_glob(glob).map_err(|e| format!("--set '{s}': {e}"))?;
        (Scope::File(glob.to_string()), *field)
    } else if known(key) {
        // Note DY: a helper is one file; an unscoped key would make every image one.
        if crate::helper::FIELDS.contains(&key) {
            return Err(format!(
                "--set '{s}': {key} is set per file: use file:<glob>.{key}=<…>"
            ));
        }
        (Scope::All, key)
    } else {
        return Err(format!("--set '{s}': unknown field '{key}'"));
    };
    // A sub-type decides the whole profile, so a misspelt one must not be ignored.
    if field == "subtype" && serde_json::from_value::<Subtype>(value.clone()).is_err() {
        return Err(format!("--set '{s}': unknown sub-type '{raw}'"));
    }
    if field == REENCODE_FIELD && !value.is_boolean() {
        return Err(format!("--set '{s}': reencode takes true or false"));
    }
    if field == RNM_TARGET_FIELD && rnm_target_value(&value).is_none() {
        return Err(format!("--set '{s}': rnm.target takes n, cnr or both"));
    }
    if let Some(e) = crate::helper::value_error(field, &value) {
        return Err(format!("--set '{s}': {e}"));
    }
    Ok(SetOverride {
        scope,
        field: field.to_string(),
        value,
    })
}

/// Dotted path and value to a nested JSON object: `("alpha.policy", "data")` → `{"alpha": {"policy": "data"}}`.
pub fn to_patch(field: &str, value: &Value) -> Value {
    let mut v = value.clone();
    for part in field.rsplit('.') {
        let mut m = Map::new();
        m.insert(part.to_string(), v);
        v = Value::Object(m);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discover::InputKind;

    const ROOT: &str = "C:/proj/textures";

    fn input(path: &str, rel: &str, real_rel: Option<&str>) -> InputFile {
        InputFile {
            path: Path::new(ROOT).join(path),
            rel: PathBuf::from(rel),
            game_path: String::new(),
            output: PathBuf::from("C:/out/x.dds"),
            kind: InputKind::Image,
            real_rel: real_rel.map(PathBuf::from),
        }
    }

    fn path_override(key: &str, field: &str) -> SetOverride {
        SetOverride {
            scope: Scope::Path(key.to_string()),
            field: field.to_string(),
            value: Value::Bool(true),
        }
    }

    fn linear<'s>(fs: &'s FileScopes, forms: &KeyForms) -> Vec<&'s SetOverride> {
        fs.overrides
            .iter()
            .filter(|o| matches!(&o.scope, Scope::Path(k) if forms.matches(k)))
            .collect()
    }

    /// The `[files]` key index returns exactly the overrides (same elements, same order) and acknowledgements a
    /// linear scan does, for mixed case, stems, nested and non-ASCII keys, and inputs `index` never saw.
    #[test]
    fn indexed_matches_linear() {
        let keys = [
            "Rock.png",
            "rock.PNG",
            "ROCK",
            "metal",
            "metal.png",
            "Metal.DDS",
            "a/b/c/deep.png",
            "A/B/C/DEEP",
            "a/b/deep.png",
            "Ätna/ß.png",
            "ätna/ß",
            "ÄTNA/ß.PNG",
            "İ.png",
            "i̇.png",
            "nothing/here.png",
            "metal_rmaos",
            "old.png",
            "new.png",
            "OLD",
            "deep.png",
        ];
        let mut fs = FileScopes {
            root: Some(PathBuf::from(ROOT)),
            ..FileScopes::default()
        };
        for field in ["subtype", "format", "alpha.policy"] {
            fs.overrides
                .extend(keys.iter().map(|k| path_override(k, field)));
        }
        assert_eq!(fs.overrides.len(), 60);
        let (c0, c1) = (Code::ALL[0], Code::ALL[1]);
        for (i, k) in keys.iter().enumerate() {
            let codes = if i % 2 == 0 { [c0].into() } else { [c1].into() };
            fs.allow.insert(k.to_string(), codes);
        }
        let indexed = [
            input("rock.png", "rock.png", None),
            input("metal.png", "metal.png", None),
            input("metal.dds", "metal.dds", None),
            input("a/b/c/deep.png", "a/b/c/deep.png", None),
            input("Ätna/ß.png", "Ätna/ß.png", None),
            input("İ.png", "İ.png", None),
        ];
        fs.index(&indexed);
        // Not indexed: a packed output's virtual name over an indexed path, and an aliased file.
        let packed = input("metal.png", "metal_rmaos.png", None);
        let aliased = input("old.png", "new.png", Some("old.png"));
        let mut hits = 0;
        for f in indexed.iter().chain([&packed, &aliased]) {
            let forms = fs.forms_of(f).expect("under the root");
            let (got, want) = (fs.path_overrides(&forms), linear(&fs, &forms));
            assert_eq!(got.len(), want.len(), "{}", f.rel.display());
            assert!(
                got.iter().zip(&want).all(|(a, b)| std::ptr::eq(*a, *b)),
                "{}",
                f.rel.display()
            );
            hits += got.len();
            for code in [c0, c1, Code::ALL[2]] {
                let want = fs
                    .allow
                    .iter()
                    .any(|(k, codes)| codes.contains(&code) && forms.matches(k));
                assert_eq!(
                    fs.allows(&forms, code),
                    want,
                    "{} {code:?}",
                    f.rel.display()
                );
            }
        }
        assert!(hits > 20, "the fixture matches too little: {hits}");
    }

    /// An override or acknowledgement added after `index` is still found (the stale index falls back to a scan).
    #[test]
    fn stale_index_falls_back() {
        let mut fs = FileScopes {
            root: Some(PathBuf::from(ROOT)),
            overrides: vec![path_override("rock.png", "subtype")],
            ..FileScopes::default()
        };
        let rock = input("rock.png", "rock.png", None);
        fs.index(std::slice::from_ref(&rock));
        fs.overrides.push(path_override("ROCK", "format"));
        fs.allow.insert("rock".into(), [Code::ALL[0]].into());
        let forms = fs.forms_of(&rock).unwrap();
        let got: Vec<&str> = fs
            .path_overrides(&forms)
            .iter()
            .map(|o| o.field.as_str())
            .collect();
        assert_eq!(got, ["subtype", "format"]);
        assert!(fs.allows(&forms, Code::ALL[0]));
    }

    /// An acknowledgement added after `index`, with `overrides` unchanged, is still seen.
    #[test]
    fn allow_only_growth_is_seen() {
        let mut fs = FileScopes {
            root: Some(PathBuf::from(ROOT)),
            overrides: vec![path_override("rock.png", "subtype")],
            ..FileScopes::default()
        };
        let rock = input("rock.png", "rock.png", None);
        fs.index(std::slice::from_ref(&rock));
        fs.allow.insert("ROCK.png".into(), [Code::ALL[1]].into());
        let forms = fs.forms_of(&rock).unwrap();
        assert!(fs.allows(&forms, Code::ALL[1]));
        assert!(!fs.allows(&forms, Code::ALL[0]));
    }

    /// `index` called again after an in-place key change (same lengths) rebuilds the key index.
    #[test]
    fn index_again_after_an_in_place_key_change_rebuilds() {
        let mut fs = FileScopes {
            root: Some(PathBuf::from(ROOT)),
            overrides: vec![path_override("rock.png", "subtype")],
            allow: [("rock.png".to_string(), [Code::ALL[0]].into())].into(),
            ..FileScopes::default()
        };
        let (rock, metal) = (
            input("rock.png", "rock.png", None),
            input("metal.png", "metal.png", None),
        );
        let files = [rock.clone(), metal.clone()];
        fs.index(&files);
        fs.overrides[0].scope = Scope::Path("Metal".into());
        fs.allow = [("metal.png".to_string(), [Code::ALL[0]].into())].into();
        fs.index(&files);
        let (r, m) = (fs.forms_of(&rock).unwrap(), fs.forms_of(&metal).unwrap());
        assert!(fs.path_overrides(&r).is_empty());
        assert_eq!(fs.path_overrides(&m).len(), 1);
        assert!(!fs.allows(&r, Code::ALL[0]));
        assert!(fs.allows(&m, Code::ALL[0]));
    }

    /// A file's `[files]` lookup costs about the same however many entries there are: four times the files and
    /// entries must cost well under 16 times (a scan of every entry per file); a linear total gives about 4.
    #[test]
    fn lookups_scale_linearly_with_files_entries() {
        fn project(n: usize) -> (FileScopes, Vec<InputFile>) {
            let files: Vec<InputFile> = (0..n)
                .map(|i| {
                    let name = format!("d{}/f{i:05}.png", i % 10);
                    input(&name, &name, None)
                })
                .collect();
            let mut fs = FileScopes {
                root: Some(PathBuf::from(ROOT)),
                overrides: (0..n)
                    .map(|i| path_override(&format!("D{}/F{i:05}.png", i % 10), "subtype"))
                    .collect(),
                ..FileScopes::default()
            };
            fs.index(&files);
            (fs, files)
        }
        fn best(fs: &FileScopes, files: &[InputFile]) -> std::time::Duration {
            (0..3)
                .map(|_| {
                    let t = std::time::Instant::now();
                    let hits: usize = files
                        .iter()
                        .map(|f| fs.path_overrides(&fs.forms_of(f).unwrap()).len())
                        .sum();
                    let took = t.elapsed();
                    assert_eq!(hits, files.len());
                    took
                })
                .min()
                .unwrap()
        }
        const N: usize = 1000;
        let (small, large) = (project(N), project(4 * N));
        let (ts, tl) = (best(&small.0, &small.1), best(&large.0, &large.1));
        eprintln!("{N} files {ts:?}, {} files {tl:?}", 4 * N);
        assert!(
            tl < ts * 10,
            "looking up {} files took {tl:?}, more than 10 times the {ts:?} of {N} files",
            4 * N
        );
    }
}
