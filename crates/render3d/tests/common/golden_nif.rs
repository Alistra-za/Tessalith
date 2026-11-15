//! sdc's own load of a golden subject's NIF (v2 Phase D, D6c), shared by `golden_material.rs` and `golden_defines.rs`:
//! the owner's game files (`SDC_ACHERUS`, read-only), the shape → NIF table, the CS 2.0 subjects pointed at a `.csm`
//! built here (Note IE7), and the load that finds the captured shape. Needs SDC_GOLDEN and SDC_ACHERUS.
#![allow(dead_code)]

use nif::material::MaterialState;
use render3d::material::{drawn, own_pbr, Pbr};
use render3d::textures::register_for_slot;
use render3d::{parse, ParsedNif, Renderer, Replay, ShaderSource, ShapeId};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Shape name → game path. Sources in the D6c report (Skyrim.esm STAT MODL; the s6 copy for ShaderSpheres02).
pub const NIFS: &[(&str, &str)] = &[
    ("Coffin", r"meshes\clutter\common\commoncoffinopen02.nif"),
    ("Object174", r"meshes\furniture\noble\nobletablelow02.nif"),
    ("dibella_L.004", r"meshes\clutter\statues\statuedibella.nif"),
    (
        "BlackreachCrystalL01",
        r"meshes\clutter\blackreach\blackreachcrystall01.nif",
    ),
    (
        "NorRubblePiece01",
        r"meshes\dungeons\nordic\rubble\norrubblepiece01.nif",
    ),
    (
        "ShaderSpheres02",
        r"meshes\architecture\sovengarde\testcollision01.nif",
    ),
    (
        "SOVStairsPiece01",
        r"meshes\architecture\sovengarde\sovstairspiece01.nif",
    ),
    // The type-21 subjects (Note ID51; the MO2 loose winner is the s6 copy while it is installed).
    (
        "HelgenStoneBlock01",
        r"meshes\clutter\helgen\helgenstoneblock01.nif",
    ),
    ("Corpse04", r"meshes\clutter\burntcorpses\corpse04.nif"),
    (
        "AzuraSmallCrystal05",
        r"meshes\dungeons\azurasstar\azurasmallcrystal05.nif",
    ),
    ("Rug01Rolled", r"meshes\clutter\common\rug01rolled.nif"),
    (
        "LoadScreenMRaltar01",
        r"meshes\loadscreenart\loadscreenmraltar01.nif",
    ),
    ("CookingStone01", r"meshes\clutter\cookingstone01.nif"),
    ("HandCartCanopy", r"meshes\clutter\carts\handcartcanopy.nif"),
    (
        "CaveWormtubeLarge",
        r"meshes\dlc01\plants\cavewormtube\cavewormtubelarge.nif",
    ),
];

/// The feature bits of a CS 2.0 subject: v1's (`pbr2::v1_values` of the mesh's own flags), replaced, or added to.
#[derive(Clone, Copy)]
pub enum Features {
    V1,
    Replace(u32),
    Add(u32),
}

/// One CS 2.0 subject of the T1, T2 and T2L captures (phase 1's manifest `pbr2test1-subjects.tsv`, Note ID51): the capture's
/// shape name, the manifest label, its features, PBRFuzz (fuzz roughness, texture switch) and PBRThinSurface
/// (p21-p24). No subject overrides a parameter.
pub struct Pbr2Subject {
    pub shape: &'static str,
    pub label: &'static str,
    pub features: Features,
    pub fuzz: Option<[f32; 2]>,
    pub thin: Option<[f32; 4]>,
}

/// The CS 2.0 subjects (Note IE7). The captures drew phase 1's type-21 NIFs; those are gone, so each subject is its
/// own mesh pointed at a `.csm` that carries the same values. Source: phase 1's manifest `pbr2test1-subjects.tsv`
/// (outside the repo, beside the captures), copied here row by row: shape, label, feature bits (replace or `+` add),
/// PBRFuzz and PBRThinSurface; it overrides no parameter.
pub const PBR2: &[Pbr2Subject] = &[
    s2("HelgenStoneBlock01", "base", Features::V1, None, None),
    s2("Corpse04", "subsurface", Features::V1, None, None),
    s2("AzuraSmallCrystal05", "two-layer", Features::V1, None, None),
    s2("Rug01Rolled", "fuzz", Features::V1, Some([0.6, 0.0]), None),
    s2("LoadScreenMRaltar01", "glint", Features::V1, None, None),
    s2(
        "CookingStone01",
        "hair",
        Features::Replace(0x40),
        None,
        None,
    ),
    s2(
        "HandCartCanopy",
        "thin-surface",
        Features::Add(0x200),
        None,
        Some([0.3, 0.7, 0.2, 1.5]),
    ),
    s2(
        "CaveWormtubeLarge",
        "foliage",
        Features::Add(0x100),
        None,
        None,
    ),
];

const fn s2(
    shape: &'static str,
    label: &'static str,
    features: Features,
    fuzz: Option<[f32; 2]>,
    thin: Option<[f32; 4]>,
) -> Pbr2Subject {
    Pbr2Subject {
        shape,
        label,
        features,
        fuzz,
        thin,
    }
}

/// Points a v1 PBR shape (type 0 or 11 with SLSF2 bit 23) of subject `s` at a `.csm` holding the values phase 1's
/// type-21 subject NIF carried: `pbr2::v1_values` of the shape's flags and trailer, `s`'s features, a PBRFuzz as the
/// fuzz roughness (and its texture switch when 1), a PBRThinSurface as p21-p24; neither on two-layer or hair, as CS 2.0
/// read them. Returns the `.csm` path and the text the resolver serves; None for any other shape.
fn point_at_csm(s: &Pbr2Subject, p: &mut render3d::ParsedShape) -> Option<(String, Vec<u8>)> {
    use nif::pbr2::feat;
    let m = &mut p.material;
    if !matches!(m.shader_type, 0 | 11) || !m.is_pbr() {
        return None;
    }
    let (mut v, _) = nif::pbr2::v1_values(m.shader_type, m.slsf2, &m.params);
    v.features = match s.features {
        Features::V1 => v.features,
        Features::Replace(f) => f,
        Features::Add(a) => v.features | a,
    };
    let plain = v.features & (feat::TWO_LAYER | feat::HAIR) == 0;
    if let Some([r, tex]) = s.fuzz.filter(|_| plain) {
        v.fuzz_roughness = r;
        if tex == 1.0 {
            v.features |= feat::FUZZ_ROUGHNESS_TEXTURE;
        }
    }
    if let Some(t) = s.thin.filter(|_| plain) {
        v.params[21..=24].copy_from_slice(&t);
    }
    let path = format!("materials/subjects/{}-{}.csm", s.label, p.id.0);
    m.csm_path = Some(path.clone());
    let text = nif::csm::to_text(&nif::pbr2::csm_from_values(&v));
    Some((path, text.into_bytes()))
}

/// The CS 2.0 subject of a subject folder (`ev<event>_<shape>_<index>`).
pub fn subject_of(folder: &str) -> Option<&'static Pbr2Subject> {
    let f = folder.to_ascii_lowercase();
    PBR2.iter()
        .find(|p| f.contains(&format!("_{}", p.shape.to_ascii_lowercase())))
}

/// Material defines compared (VC is VS-only in sdc and the geometry is the capture's).
pub const M: &[&str] = &[
    "ENVMAP",
    "PARALLAX",
    "FACEGEN",
    "FACEGEN_RGB_TINT",
    "HAIR",
    "MULTI_LAYER_PARALLAX",
    "EYE",
    "SKINNED",
    "MODELSPACENORMALS",
    "PROJECTED_UV",
    "TRUE_PBR",
    "DO_ALPHA_TEST",
    "DEPTH_WRITE_DECALS",
    "GLINT",
    "ANISO_LIGHTING",
    "SPECULAR",
    "SOFT_LIGHTING",
    "RIM_LIGHTING",
    "BACK_LIGHTING",
];

/// The owner's game files, read-only: MO2 loose winner first, else the highest-ranked active BSA. No Store, no
/// scan.db.
pub struct Game {
    origins: Vec<modscan::mo2::LooseOrigin>,
    bsas: Vec<vfs::bsa::Bsa>,
    /// Canonical path → (bsa index, entry index), the highest rank winning.
    index: HashMap<String, (usize, usize)>,
}

impl Game {
    fn load(acherus: &Path) -> Game {
        let inst = modscan::mo2::Instance::load(acherus, None).unwrap();
        let (mut origins, _) = modscan::mo2::loose_origins(&inst).unwrap();
        origins.sort_by_key(|o| std::cmp::Reverse(o.priority));
        let (active, _) = modscan::archives::active_bsas(&inst, &origins);
        let (mut bsas, mut index) = (Vec::new(), HashMap::new());
        // active_bsas is lowest rank first: later inserts win.
        for a in &active {
            let Ok(b) = vfs::bsa::Bsa::open(&a.path) else {
                continue;
            };
            for (i, e) in b.entries.iter().enumerate() {
                index.insert(e.norm.clone(), (bsas.len(), i));
            }
            bsas.push(b);
        }
        Game {
            origins,
            bsas,
            index,
        }
    }

    /// (origin, bytes) of a game path (`meshes\…` or canonical `textures/…`).
    pub fn file(&self, rel: &str) -> Option<(String, Vec<u8>)> {
        let norm = nif::path::canon_mesh(rel);
        let win = norm.replace('/', "\\");
        if let Some(o) = self.origins.iter().find(|o| o.root.join(&win).is_file()) {
            return Some((
                format!("loose {}", o.name),
                std::fs::read(o.root.join(&win)).ok()?,
            ));
        }
        let (b, e) = *self.index.get(&norm)?;
        let bsa = &self.bsas[b];
        Some((
            format!("bsa {}", bsa.path.file_name()?.to_string_lossy()),
            bsa.read(&bsa.entries[e]).ok()?,
        ))
    }
}

pub struct Env {
    pub root: PathBuf,
    pub game: Game,
    pub cache: tempfile::TempDir,
}

pub fn env() -> Option<Env> {
    let (Ok(root), Ok(acherus)) = (std::env::var("SDC_GOLDEN"), std::env::var("SDC_ACHERUS"))
    else {
        println!("skipped: set SDC_GOLDEN and SDC_ACHERUS");
        return None;
    };
    let root = PathBuf::from(root);
    Some(Env {
        root,
        game: Game::load(Path::new(&acherus)),
        cache: tempfile::tempdir().unwrap(),
    })
}

/// A renderer on the bundled shaders.
pub fn renderer(env: &Env) -> Renderer {
    Renderer::with_source(ShaderSource::Bundled, env.cache.path().to_path_buf()).unwrap()
}

pub fn key(d: &str) -> &str {
    d.split('=').next().unwrap_or(d)
}

pub fn json(p: &Path) -> Value {
    serde_json::from_str(&render3d::replay::nonfinite_to_null(
        &std::fs::read_to_string(p).unwrap(),
    ))
    .unwrap()
}

/// The subject's captured shape name (`state.json` identity.shapeName) when [`NIFS`] maps it.
pub fn mapped_shape(dir: &Path) -> Option<String> {
    let name = json(&dir.join("state.json"))["identity"]["shapeName"]
        .as_str()?
        .to_string();
    NIFS.iter()
        .any(|(n, _)| n.eq_ignore_ascii_case(&name))
        .then_some(name)
}

/// sdc's load of the subject's NIF: the shape, how it was found, and the shape's slot textures by register.
pub struct SdcShape {
    pub id: ShapeId,
    pub rule: String,
    pub nif_origin: String,
    /// (register, canonical path, bytes or None when the resolver found nothing).
    pub slots: Vec<(u32, String, Option<Vec<u8>>)>,
    /// LoadReport errors + compile errors.
    pub errors: Vec<String>,
    /// The loaded shape's `.csm` name, whether it took PBR values from it and its `csm_error`: a counted subject
    /// counts only when its `.csm` resolved (Note IE7).
    pub csm: (Option<String>, bool, Option<String>),
    /// The PS b1 fields whose x, y and z are a colour of the shape's `.csm` (empty without one): `PBRParams2` with
    /// subsurface or two-layer, `MultiLayerParallaxData` with fuzz (Note IE7).
    pub colours: Vec<&'static str>,
}

/// Loads the subject's NIF into `ren` (features = the capture's non-material defines, so ENABLE_LL and the CS
/// features match) and finds the shape by identity.shapeName, then ShapeId(shapeIndex) when several share it.
pub fn load_sdc(ren: &mut Renderer, env: &Env, dir: &Path, r: &Replay) -> Result<SdcShape, String> {
    let ident = &json(&dir.join("state.json"))["identity"];
    let name = ident["shapeName"].as_str().ok_or("no identity.shapeName")?;
    let index = ident["shapeIndex"].as_u64();
    let path = NIFS
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .ok_or_else(|| format!("{name}: not in NIFS"))?
        .1;
    let (nif_origin, bytes) = env
        .game
        .file(path)
        .ok_or_else(|| format!("{path}: not found"))?;
    let mut parsed: ParsedNif = parse(&bytes);
    let subject = PBR2.iter().find(|s| s.shape.eq_ignore_ascii_case(name));
    let csm: HashMap<String, Vec<u8>> = match subject {
        Some(s) => parsed
            .shapes
            .iter_mut()
            .filter_map(|p| point_at_csm(s, p))
            .collect(),
        None => HashMap::new(),
    };
    let with = |n: &str| -> Vec<&render3d::ParsedShape> {
        parsed
            .shapes
            .iter()
            .filter(|s| s.name.eq_ignore_ascii_case(n))
            .collect()
    };
    // The capture splits a "name:N" NIF shape name into shapeName and shapeIndex.
    let suffixed = index.map(|i| format!("{name}:{i}"));
    let by_index = index.and_then(|i| parsed.shapes.iter().find(|s| s.id == ShapeId(i as u32)));
    let (shape, how) = match (with(name).as_slice(), suffixed.as_deref().map(with)) {
        ([one], _) => (*one, "shapeName unique".to_string()),
        (_, Some(v)) if v.len() == 1 => (v[0], format!("NIF name {:?}", v[0].name)),
        (several, _)
            if several.len() > 1 && by_index.is_some_and(|b| b.name.eq_ignore_ascii_case(name)) =>
        {
            (
                by_index.unwrap(),
                format!("shapeName x{}, ShapeId(shapeIndex)", several.len()),
            )
        }
        _ => {
            return Err(format!(
                "{name} (index {index:?}): no unique shape in {path}; shapes {:?}",
                parsed
                    .shapes
                    .iter()
                    .map(|s| (s.id.0, &s.name))
                    .collect::<Vec<_>>()
            ))
        }
    };
    let rule = format!(
        "{how} -> block {} {:?} type {}; ShapeId(shapeIndex {index:?}) {}",
        shape.id.0,
        shape.name,
        shape.material.shader_type,
        match by_index {
            Some(b) if b.id == shape.id => "is the same block".to_string(),
            Some(b) => format!("is block {} {:?}", b.id.0, b.name),
            None => "absent".to_string(),
        }
    );
    let id = shape.id;
    // CS features as captured: everything but the material defines, DEFERRED, VC and K=V values.
    let features: Vec<String> = r.defines[0]
        .iter()
        .filter(|d| {
            !d.contains('=') && !M.contains(&d.as_str()) && !matches!(d.as_str(), "DEFERRED" | "VC")
        })
        .cloned()
        .collect();
    ren.set_features(features);
    let rep = ren.load_parsed_sync(&parsed, &|p| {
        csm.get(p)
            .cloned()
            .or_else(|| env.game.file(p).map(|f| f.1))
    });
    // The registers of the material the shaders draw (Note ID33), as `bind_textures` maps them; a `.csm` applied.
    let m = ren
        .shapes()
        .into_iter()
        .find(|s| s.id == id)
        .ok_or("the shape was not loaded")?
        .material;
    let d = drawn(&m);
    let (pbr, msn) = (own_pbr(&d) != Pbr::None, d.slsf1 & (1 << 12) != 0);
    let slots = d
        .slots
        .iter()
        .enumerate()
        .filter(|(_, p)| !p.norm.is_empty())
        .filter_map(|(slot, p)| {
            let reg = register_for_slot(d.shader_type, d.slsf2, pbr, msn, slot)?;
            Some((reg, p.norm.clone(), env.game.file(&p.norm).map(|f| f.1)))
        })
        .collect();
    Ok(SdcShape {
        id,
        rule,
        nif_origin,
        slots,
        errors: rep.errors.into_iter().chain(rep.compile_errors).collect(),
        csm: (m.csm_path.clone(), m.pbr.is_some(), m.csm_error.clone()),
        colours: colour_fields(&m),
    })
}

/// The PS b1 fields that carry a colour of `m`'s CS 2.0 PBR values, as `write_pbr2` packs them.
fn colour_fields(m: &MaterialState) -> Vec<&'static str> {
    use nif::pbr2::feat;
    let Some(v) = &m.pbr else { return vec![] };
    let has = |b: u32| v.features & b != 0;
    let mut out = vec![];
    if has(feat::TWO_LAYER | feat::SUBSURFACE) {
        out.push("PBRParams2");
    }
    if has(feat::FUZZ) && !has(feat::TWO_LAYER) {
        out.push("MultiLayerParallaxData");
    }
    out
}
