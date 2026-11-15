// crates/nif/src/csm.rs
//! Community Shaders material files (`.csm`, CS's PBR 2.0 proposal; Note IE): a table of the schema's keys
//! (`zcsm.schema.json`), a per-file parse with the schema's type, range and pattern checks, its allOf rules, a
//! writer, and `resolve` for root chains. A file holds only the keys it sets: an absent key comes from the root
//! material or the NIF. `pbr2::{pbr_values, csm_from_values}` bridge the PBR keys to the shader's values.
use std::collections::BTreeMap;

/// The `Version` this model reads and writes.
pub const VERSION: u32 = 1;
/// The schema link written as `$schema`; empty, so not written, until CS publishes one.
pub const SCHEMA_URL: &str = "";

/// What a key holds and how it is checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Bool,
    Num,
    /// `#rrggbb`.
    Color,
    /// A DDS path relative to `Data\textures`, or empty.
    Texture,
    /// A `.csm` path (`RootMaterialPath`), or empty.
    Path,
}

/// One schema key.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyDef {
    pub name: &'static str,
    pub kind: Kind,
    /// Inclusive bounds of a number; an infinite bound is no bound.
    pub range: Option<(f64, f64)>,
    /// A texture key's slot in the NIF's texture set: the 0-based index, in the order BSShaderTextureSet lists them.
    pub slot: Option<usize>,
    /// One of the generic (non-PBR) shader keys (tiling, UV, alpha, flags, emittance): a set's `.csm` never holds
    /// it, so the NIF's own value stands. False for textures, PBR keys and `RootMaterialPath`.
    pub generic: bool,
}

const INF: f64 = f64::INFINITY;

const fn key(name: &'static str, kind: Kind) -> KeyDef {
    KeyDef {
        name,
        kind,
        range: None,
        slot: None,
        generic: false,
    }
}
const fn generic(name: &'static str, kind: Kind) -> KeyDef {
    KeyDef {
        generic: true,
        ..key(name, kind)
    }
}
const fn tex(name: &'static str, slot: usize) -> KeyDef {
    KeyDef {
        slot: Some(slot),
        ..key(name, Kind::Texture)
    }
}
const fn ranged(name: &'static str, lo: f64, hi: f64) -> KeyDef {
    KeyDef {
        range: Some((lo, hi)),
        ..key(name, Kind::Num)
    }
}

/// Every schema key except `$schema` and `Version`, in the order of the spec's example (the order `to_text` writes).
pub const KEYS: &[KeyDef] = &[
    key("RootMaterialPath", Kind::Path),
    tex("DiffuseTexture", 0),
    tex("NormalTexture", 1),
    tex("GlowTexture", 2),
    tex("DisplacementTexture", 3),
    tex("RmaosTexture", 5),
    tex("Features0Texture", 7),
    tex("Features1Texture", 6),
    generic("TileU", Kind::Bool),
    generic("TileV", Kind::Bool),
    generic("UOffset", Kind::Num),
    generic("VOffset", Kind::Num),
    generic("UScale", Kind::Num),
    generic("VScale", Kind::Num),
    KeyDef {
        generic: true,
        ..ranged("Alpha", 0.0, 128.0)
    },
    generic("TwoSided", Kind::Bool),
    generic("Decal", Kind::Bool),
    generic("DynamicDecal", Kind::Bool),
    generic("ModelSpaceNormals", Kind::Bool),
    generic("VertexColors", Kind::Bool),
    generic("CastShadows", Kind::Bool),
    generic("ReceiveShadows", Kind::Bool),
    generic("ZBufferTest", Kind::Bool),
    generic("ZBufferWrite", Kind::Bool),
    generic("ExternalEmittance", Kind::Bool),
    generic("EmittanceColor", Kind::Color),
    KeyDef {
        generic: true,
        ..ranged("EmittanceMult", 0.0, INF)
    },
    key("PBREnabled", Kind::Bool),
    ranged("PBRDisplacementTextureScale", 0.0, INF),
    ranged("PBRSpecularLevel", 0.0, 1.0),
    ranged("PBRRoughnessScale", 0.0, INF),
    key("PBRSubsurfaceEnabled", Kind::Bool),
    key("PBRSubsurfaceColor", Kind::Color),
    ranged("PBRSubsurfaceOpacity", 0.0, 1.0),
    // shortcut: invented key (owner 2026-11-15), not in CS's zcsm.schema.json; rename or drop when CS adds its own foliage key.
    key("PBRFoliageEnabled", Kind::Bool),
    key("PBRThinSurfaceEnabled", Kind::Bool),
    ranged("PBRThinSurfaceSubsurfaceBalance", 0.0, 1.0),
    ranged("PBRThinSurfaceSpecularWeight", 0.0, 1.0),
    ranged("PBRThinSurfaceBackSpecularWeight", 0.0, 1.0),
    ranged("PBRThinSurfaceBackRoughnessScale", 0.0, 3.0),
    key("PBRTwoLayerEnabled", Kind::Bool),
    key("PBRTwoLayerColoredCoat", Kind::Bool),
    key("PBRTwoLayerInterlayerParallax", Kind::Bool),
    key("PBRTwoLayerCoatNormal", Kind::Bool),
    key("PBRTwoLayerCoatColor", Kind::Color),
    ranged("PBRTwoLayerCoatStrength", 0.0, 1.0),
    ranged("PBRTwoLayerCoatRoughness", 0.0, 1.0),
    ranged("PBRTwoLayerCoatSpecularLevel", 0.0, 1.0),
    key("PBRFuzzEnabled", Kind::Bool),
    key("PBRFuzzColor", Kind::Color),
    ranged("PBRFuzzWeight", 0.0, 1.0),
    ranged("PBRFuzzRoughness", -INF, 1.0),
    key("PBRFuzzRoughnessTexture", Kind::Bool),
    key("PBRGlintEnabled", Kind::Bool),
    ranged("PBRGlintScreenSpaceScale", 1.0, INF),
    ranged("PBRGlintLogMicrofacetDensity", 1.0, 40.0),
    ranged("PBRGlintMicrofacetRoughness", 0.005, 0.3),
    ranged("PBRGlintDensityRandomization", 0.0, 5.0),
    key("PBRHairMarschner", Kind::Bool),
];

/// A key's value. Texture paths and `RootMaterialPath` are `Text`.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Num(f64),
    Color([u8; 3]),
    Text(String),
}

/// One `.csm` file: the keys it sets, by their `KEYS` name.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Csm {
    pub version: u32,
    pub values: BTreeMap<&'static str, Value>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CsmError {
    #[error("not valid JSON: {0}")]
    Json(String),
    #[error("version {0} is not supported (only {VERSION})")]
    Version(u64),
    #[error("unknown key {0}")]
    UnknownKey(String),
    #[error("{key} has the wrong type")]
    Type { key: String },
    #[error("{key} is {value}, outside its range")]
    Range { key: String, value: f64 },
    #[error("{key} does not have the required form")]
    Pattern { key: String },
    #[error("{0}")]
    Rule(&'static str),
    #[error("root material {path}: {reason}")]
    Root { path: String, reason: Box<CsmError> },
    #[error("root material chain loops at {0}")]
    Loop(String),
    #[error("root material chain is too deep")]
    Depth,
    #[error("missing key {0}")]
    Missing(String),
    #[error("material file {0} not found")]
    NotFound(String),
    #[error("{0} is not a .csm file under materials")]
    NotMaterial(String),
}

/// The most files one material chain holds: the file asked for and its roots.
pub const MAX_CHAIN: usize = 16;

/// Reads the `.csm` at `path` and its `RootMaterialPath` chain through `read`, which gets canonical paths
/// (`path::canon_material`). The chain merges root first, so each child's keys win; the allOf rules are checked on
/// the merge, and the result holds no `RootMaterialPath`. An error in a root is `Root` naming that root; the file
/// asked for reports its own errors bare, as `Loop`, `Depth` and `Rule` are (Note IE2).
pub fn resolve(path: &str, read: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<Csm, CsmError> {
    let mut next = Some(
        crate::path::canon_material(path).ok_or_else(|| CsmError::NotMaterial(path.to_string()))?,
    );
    let mut seen: Vec<String> = Vec::new();
    let mut files = Vec::new();
    while let Some(at) = next {
        if seen.contains(&at) {
            return Err(CsmError::Loop(at));
        }
        if seen.len() == MAX_CHAIN {
            return Err(CsmError::Depth);
        }
        let (file, root) = match load(&at, read) {
            Ok(loaded) => loaded,
            Err(e) if seen.is_empty() => return Err(e),
            Err(e) => {
                return Err(CsmError::Root {
                    path: at,
                    reason: Box::new(e),
                })
            }
        };
        files.push(file);
        seen.push(at);
        next = root;
    }
    let mut merged = Csm {
        version: VERSION,
        values: BTreeMap::new(),
    };
    for file in files.into_iter().rev() {
        merged.values.extend(file.values);
    }
    match rule_errors(&merged).first().copied() {
        Some(rule) => Err(CsmError::Rule(rule)),
        None => Ok(merged),
    }
}

/// One file of a chain, parsed, without its `RootMaterialPath`, and that root's canonical path.
fn load(
    path: &str,
    read: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> Result<(Csm, Option<String>), CsmError> {
    let bytes = read(path).ok_or_else(|| CsmError::NotFound(path.to_string()))?;
    let text =
        std::str::from_utf8(&bytes).map_err(|e| CsmError::Json(format!("not UTF-8: {e}")))?;
    let mut c = parse(text)?;
    let root = match c.values.remove("RootMaterialPath") {
        Some(Value::Text(r)) if !r.is_empty() => Some(crate::path::canon_material(&r).ok_or_else(
            || CsmError::Pattern {
                key: "RootMaterialPath".into(),
            },
        )?),
        _ => None,
    };
    Ok((c, root))
}

fn ends_with_ci(s: &str, suffix: &str) -> bool {
    let (s, suffix) = (s.as_bytes(), suffix.as_bytes());
    s.len() >= suffix.len() && s[s.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
}

fn parse_color(s: &str) -> Option<[u8; 3]> {
    let digits = s.strip_prefix('#')?;
    if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

fn value_of(def: &KeyDef, v: &serde_json::Value) -> Result<Value, CsmError> {
    let ty = || CsmError::Type {
        key: def.name.to_string(),
    };
    let pattern = || CsmError::Pattern {
        key: def.name.to_string(),
    };
    match def.kind {
        Kind::Bool => v.as_bool().map(Value::Bool).ok_or_else(ty),
        Kind::Num => {
            let n = v.as_f64().ok_or_else(ty)?;
            match def.range {
                Some((lo, hi)) if !(lo..=hi).contains(&n) => Err(CsmError::Range {
                    key: def.name.to_string(),
                    value: n,
                }),
                _ => Ok(Value::Num(n)),
            }
        }
        Kind::Color => parse_color(v.as_str().ok_or_else(ty)?)
            .map(Value::Color)
            .ok_or_else(pattern),
        Kind::Texture | Kind::Path => {
            let s = v.as_str().ok_or_else(ty)?;
            let ext = if def.kind == Kind::Texture {
                ".dds"
            } else {
                ".csm"
            };
            if s.is_empty() || ends_with_ci(s, ext) {
                Ok(Value::Text(s.to_string()))
            } else {
                Err(pattern())
            }
        }
    }
}

/// Reads one `.csm` file with the schema's per-file checks: valid JSON (a leading UTF-8 BOM is accepted), `Version`
/// present and 1, no unknown key, and each key's type, range and pattern. The allOf rules are `rule_errors`.
pub fn parse(text: &str) -> Result<Csm, CsmError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let json: serde_json::Value =
        serde_json::from_str(text).map_err(|e| CsmError::Json(e.to_string()))?;
    let serde_json::Value::Object(map) = json else {
        return Err(CsmError::Json("the file is not a JSON object".into()));
    };
    let version = map
        .get("Version")
        .ok_or_else(|| CsmError::Missing("Version".into()))?;
    // Draft-07 counts a whole float (`1.0`) as an integer.
    let whole = version
        .as_f64()
        .filter(|v| *v >= 0.0 && v.fract() == 0.0)
        .map(|v| v as u64);
    let version = version.as_u64().or(whole).ok_or_else(|| CsmError::Type {
        key: "Version".into(),
    })?;
    if version != u64::from(VERSION) {
        return Err(CsmError::Version(version));
    }
    let mut c = Csm {
        version: VERSION,
        values: BTreeMap::new(),
    };
    for (name, v) in &map {
        match name.as_str() {
            "Version" => {}
            "$schema" if v.is_string() => {}
            "$schema" => return Err(CsmError::Type { key: name.clone() }),
            _ => {
                let def = KEYS
                    .iter()
                    .find(|k| k.name == name)
                    .ok_or_else(|| CsmError::UnknownKey(name.clone()))?;
                c.values.insert(def.name, value_of(def, v)?);
            }
        }
    }
    Ok(c)
}

/// The messages of `rule_errors`, one per allOf rule of the schema (with `PBRFoliageEnabled` added to the first two).
pub mod rule {
    pub const HAIR_ALONE: &str =
        "PBRHairMarschner cannot be on with subsurface, thin surface, foliage, two-layer (or its coat options), fuzz or glint";
    pub const TWO_LAYER_EXCLUDES: &str =
        "PBRTwoLayerEnabled cannot be on with subsurface, thin surface, foliage, fuzz or glint";
    pub const FUZZ_GLINT: &str = "PBRFuzzEnabled cannot be on with PBRGlintEnabled";
    pub const COLORED_COAT_NEEDS_TWO_LAYER: &str =
        "PBRTwoLayerColoredCoat needs PBRTwoLayerEnabled";
    pub const PARALLAX_NEEDS_TWO_LAYER: &str =
        "PBRTwoLayerInterlayerParallax needs PBRTwoLayerEnabled";
    pub const COAT_NORMAL_NEEDS_TWO_LAYER: &str = "PBRTwoLayerCoatNormal needs PBRTwoLayerEnabled";
    pub const PBR_NEEDS_TEXTURES: &str =
        "PBREnabled needs DiffuseTexture, NormalTexture and RmaosTexture";
    pub const PARALLAX_NEEDS_DISPLACEMENT: &str =
        "PBRTwoLayerInterlayerParallax needs DisplacementTexture";
    pub const COAT_NORMAL_NEEDS_FEATURES1: &str = "PBRTwoLayerCoatNormal needs Features1Texture";
}

/// The schema's allOf rules that `c` breaks, in the schema's order. As in the schema, a rule checks only keys the
/// file holds: a switch that is absent, or a texture that is absent rather than empty, breaks nothing.
pub fn rule_errors(c: &Csm) -> Vec<&'static str> {
    use rule::*;
    let is = |k: &str, b: bool| c.values.get(k) == Some(&Value::Bool(b));
    let on = |k: &str| is(k, true);
    let empty = |k: &str| matches!(c.values.get(k), Some(Value::Text(s)) if s.is_empty());
    let any_on = |keys: &[&str]| keys.iter().any(|k| on(k));
    let rules = [
        (
            on("PBRHairMarschner")
                && any_on(&[
                    "PBRSubsurfaceEnabled",
                    "PBRThinSurfaceEnabled",
                    "PBRTwoLayerEnabled",
                    "PBRTwoLayerColoredCoat",
                    "PBRTwoLayerInterlayerParallax",
                    "PBRTwoLayerCoatNormal",
                    "PBRFuzzEnabled",
                    "PBRGlintEnabled",
                    "PBRFoliageEnabled",
                ]),
            HAIR_ALONE,
        ),
        (
            on("PBRTwoLayerEnabled")
                && any_on(&[
                    "PBRSubsurfaceEnabled",
                    "PBRThinSurfaceEnabled",
                    "PBRFuzzEnabled",
                    "PBRGlintEnabled",
                    "PBRFoliageEnabled",
                ]),
            TWO_LAYER_EXCLUDES,
        ),
        (on("PBRFuzzEnabled") && on("PBRGlintEnabled"), FUZZ_GLINT),
        (
            on("PBRTwoLayerColoredCoat") && is("PBRTwoLayerEnabled", false),
            COLORED_COAT_NEEDS_TWO_LAYER,
        ),
        (
            on("PBRTwoLayerInterlayerParallax") && is("PBRTwoLayerEnabled", false),
            PARALLAX_NEEDS_TWO_LAYER,
        ),
        (
            on("PBRTwoLayerCoatNormal") && is("PBRTwoLayerEnabled", false),
            COAT_NORMAL_NEEDS_TWO_LAYER,
        ),
        (
            on("PBREnabled")
                && ["DiffuseTexture", "NormalTexture", "RmaosTexture"]
                    .iter()
                    .any(|k| empty(k)),
            PBR_NEEDS_TEXTURES,
        ),
        (
            on("PBRTwoLayerInterlayerParallax") && empty("DisplacementTexture"),
            PARALLAX_NEEDS_DISPLACEMENT,
        ),
        (
            on("PBRTwoLayerCoatNormal") && empty("Features1Texture"),
            COAT_NORMAL_NEEDS_FEATURES1,
        ),
    ];
    rules
        .into_iter()
        .filter_map(|(broken, text)| broken.then_some(text))
        .collect()
}

/// Writes `c` as a `.csm` file: `Version` 1 first, then the keys `c` holds in `KEYS` order, 2-space indent,
/// colours as lowercase `#rrggbb`, numbers in `serde_json`'s float form, no trailing newline.
pub fn to_text(c: &Csm) -> String {
    let mut lines = Vec::new();
    if !SCHEMA_URL.is_empty() {
        lines.push(format!(
            "  \"$schema\": {}",
            serde_json::Value::from(SCHEMA_URL)
        ));
    }
    lines.push(format!("  \"Version\": {VERSION}"));
    for def in KEYS {
        let Some(v) = c.values.get(def.name) else {
            continue;
        };
        let v = match v {
            Value::Bool(b) => b.to_string(),
            // shortcut: a non-finite number writes `null`, which `parse` refuses; callers write finite values.
            Value::Num(n) => serde_json::Value::from(*n).to_string(),
            Value::Color([r, g, b]) => format!("\"#{r:02x}{g:02x}{b:02x}\""),
            Value::Text(s) => serde_json::Value::from(s.as_str()).to_string(),
        };
        lines.push(format!("  \"{}\": {v}", def.name));
    }
    format!("{{\n{}\n}}", lines.join(",\n"))
}

/// The `f64` that `v`'s shortest decimal form names, so 0.6f32 is written `0.6`.
pub fn num(v: f32) -> f64 {
    format!("{v}").parse().unwrap_or(f64::from(v))
}

/// A colour to bytes: `round(clamp(v, 0, 1) * 255)` per channel, worked in `f64` on `num(v)`, no gamma.
pub fn hex(rgb: [f32; 3]) -> [u8; 3] {
    rgb.map(|v| (num(v).clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// Bytes to a colour: `/ 255`, no gamma.
pub fn rgb(c: [u8; 3]) -> [f32; 3] {
    c.map(|b| f32::from(b) / 255.0)
}
